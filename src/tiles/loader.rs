use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::core::geo::TileCoord;
use crate::core::viewport::Viewport;
use super::source::TileSource;
use crate::Result;
use once_cell::sync::Lazy;

#[cfg(feature = "debug")]
use log;

/// Shared async HTTP client optimized for tile fetching
pub(crate) static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("map-rs/0.1 (+https://github.com/example/map-rs)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build reqwest async client")
});

/// Priority for tile loading (higher number = higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePriority {
    /// Currently visible tiles (highest priority)
    Visible = 100,
    /// One ring around visible area
    Adjacent = 50,
    /// Prefetch tiles for predicted movement
    Prefetch = 10,
    /// Background/low priority
    Background = 1,
}

/// A tile loading task with priority
#[derive(Debug, Clone)]
pub struct TileTask {
    pub coord: TileCoord,
    pub url: String,
    pub priority: TilePriority,
    /// Sequence number for tie-breaking (lower = earlier)
    pub sequence: u64,
}

impl PartialEq for TileTask {
    fn eq(&self, other: &Self) -> bool {
        self.coord == other.coord
    }
}

impl Eq for TileTask {}

impl PartialOrd for TileTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TileTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier sequence number
        match (self.priority as u8).cmp(&(other.priority as u8)) {
            Ordering::Equal => other.sequence.cmp(&self.sequence),
            other => other,
        }
    }
}

/// Result of a tile loading operation
#[derive(Debug)]
pub struct TileResult {
    pub coord: TileCoord,
    pub data: Result<Vec<u8>>,
}

/// Configuration for the tile loader
#[derive(Debug, Clone)]
pub struct TileLoaderConfig {
    /// Maximum concurrent tile downloads
    pub max_concurrent: usize,
    /// Maximum number of retry attempts per tile
    pub max_retries: usize,
    /// Delay between retry attempts
    pub retry_delay: std::time::Duration,
}

impl Default for TileLoaderConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 32,
            max_retries: 2,
            retry_delay: std::time::Duration::from_millis(100),
        }
    }
}

/// Movement pattern tracking for predictive prefetching
#[derive(Debug, Clone)]
pub struct MovementPattern {
    /// Recent viewport centers (for direction prediction)
    recent_centers: VecDeque<(crate::core::geo::LatLng, Instant)>,
    /// Recent zoom changes (for zoom prediction)
    recent_zooms: VecDeque<(f64, Instant)>,
    /// Average movement velocity (pixels per second)
    velocity: Option<crate::core::geo::Point>,
    /// Predicted next viewport center
    predicted_center: Option<crate::core::geo::LatLng>,
    /// Confidence in prediction (0.0 to 1.0)
    prediction_confidence: f64,
}

impl Default for MovementPattern {
    fn default() -> Self {
        Self {
            recent_centers: VecDeque::with_capacity(10),
            recent_zooms: VecDeque::with_capacity(5),
            velocity: None,
            predicted_center: None,
            prediction_confidence: 0.0,
        }
    }
}

impl MovementPattern {
    /// Update movement pattern with new viewport
    pub fn update(&mut self, viewport: &Viewport) {
        let now = Instant::now();
        
        // Clean old entries (older than 5 seconds)
        let cutoff = now - Duration::from_secs(5);
        self.recent_centers.retain(|(_, time)| *time > cutoff);
        self.recent_zooms.retain(|(_, time)| *time > cutoff);
        
        // Add new entries
        self.recent_centers.push_back((viewport.center, now));
        self.recent_zooms.push_back((viewport.zoom, now));
        
        // Keep reasonable limits
        if self.recent_centers.len() > 10 {
            self.recent_centers.pop_front();
        }
        if self.recent_zooms.len() > 5 {
            self.recent_zooms.pop_front();
        }
        
        // Calculate velocity and prediction
        self.calculate_velocity();
        self.predict_next_position();
    }
    
    fn calculate_velocity(&mut self) {
        if self.recent_centers.len() < 2 {
            self.velocity = None;
            return;
        }
        
        let recent: Vec<_> = self.recent_centers.iter().collect();
        let mut total_velocity = crate::core::geo::Point::new(0.0, 0.0);
        let mut count = 0;
        
        for i in 1..recent.len() {
            let (prev_pos, prev_time) = recent[i - 1];
            let (curr_pos, curr_time) = recent[i];
            
            let time_diff = curr_time.duration_since(*prev_time).as_secs_f64();
            if time_diff > 0.0 {
                let lat_diff = curr_pos.lat - prev_pos.lat;
                let lng_diff = curr_pos.lng - prev_pos.lng;
                
                // Convert to approximate pixel velocity (rough approximation)
                let pixel_per_degree = 111_000.0; // meters per degree at equator
                let velocity_x = lng_diff * pixel_per_degree / time_diff;
                let velocity_y = lat_diff * pixel_per_degree / time_diff;
                
                total_velocity.x += velocity_x;
                total_velocity.y += velocity_y;
                count += 1;
            }
        }
        
        if count > 0 {
            self.velocity = Some(crate::core::geo::Point::new(
                total_velocity.x / count as f64,
                total_velocity.y / count as f64,
            ));
        }
    }
    
    fn predict_next_position(&mut self) {
        if let Some(velocity) = self.velocity {
            if let Some((last_center, _last_time)) = self.recent_centers.back() {
                // Predict position 1 second ahead
                let prediction_time = 1.0;
                let pixel_per_degree = 111_000.0;
                
                let predicted_lat = last_center.lat + (velocity.y * prediction_time) / pixel_per_degree;
                let predicted_lng = last_center.lng + (velocity.x * prediction_time) / pixel_per_degree;
                
                self.predicted_center = Some(crate::core::geo::LatLng::new(predicted_lat, predicted_lng));
                
                // Calculate confidence based on velocity consistency
                let speed = (velocity.x.powi(2) + velocity.y.powi(2)).sqrt();
                self.prediction_confidence = if speed > 10.0 { // Moving fast enough to predict
                    (speed / 1000.0).min(1.0) // Higher speed = higher confidence, capped at 1.0
                } else {
                    0.0
                };
            }
        }
    }
    
    /// Get tiles to prefetch based on movement prediction
    pub fn get_prefetch_tiles(&self, current_viewport: &Viewport) -> Vec<TileCoord> {
        let mut prefetch_tiles = Vec::new();
        
        if let Some(predicted_center) = self.predicted_center {
            if self.prediction_confidence > 0.3 {
                // Get tiles for predicted viewport
                let zoom = current_viewport.zoom.floor() as u8;
                let tiles_per_axis = 1u32 << zoom;
                
                // Convert predicted center to tile coordinates
                let lat_rad = predicted_center.lat.to_radians();
                let x = (predicted_center.lng + 180.0) / 360.0 * tiles_per_axis as f64;
                let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)
                    / 2.0 * tiles_per_axis as f64;
                
                let center_x = x as u32;
                let center_y = y as u32;
                
                // Add surrounding tiles
                let radius = 2; // Prefetch 2 tiles in each direction
                for dx in -(radius as i32)..=(radius as i32) {
                    for dy in -(radius as i32)..=(radius as i32) {
                        let tile_x = ((center_x as i32 + dx) as u32) % tiles_per_axis;
                        let tile_y = ((center_y as i32 + dy).max(0) as u32).min(tiles_per_axis - 1);
                        
                        prefetch_tiles.push(TileCoord {
                            x: tile_x,
                            y: tile_y,
                            z: zoom,
                        });
                    }
                }
            }
        }
        
        prefetch_tiles
    }
}

/// Network performance tracking
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    /// Recent download times (in milliseconds)
    recent_download_times: VecDeque<(Duration, Instant)>,
    /// Average download time
    average_download_time: Duration,
    /// Network condition (Good, Fair, Poor)
    condition: NetworkCondition,
    /// Failed requests in recent window
    recent_failures: VecDeque<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkCondition {
    Good,    // < 500ms average
    Fair,    // 500ms - 2s average
    Poor,    // > 2s average
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            recent_download_times: VecDeque::with_capacity(20),
            average_download_time: Duration::from_millis(500),
            condition: NetworkCondition::Good,
            recent_failures: VecDeque::with_capacity(10),
        }
    }
}

impl NetworkMetrics {
    /// Record a successful download
    pub fn record_success(&mut self, duration: Duration) {
        let now = Instant::now();
        
        // Clean old entries (older than 30 seconds)
        let cutoff = now - Duration::from_secs(30);
        self.recent_download_times.retain(|(_, time)| *time > cutoff);
        
        // Add new entry
        self.recent_download_times.push_back((duration, now));
        
        // Keep reasonable limit
        if self.recent_download_times.len() > 20 {
            self.recent_download_times.pop_front();
        }
        
        // Update average
        self.update_average();
        self.update_condition();
    }
    
    /// Record a failed download
    pub fn record_failure(&mut self) {
        let now = Instant::now();
        
        // Clean old failures (older than 30 seconds)
        let cutoff = now - Duration::from_secs(30);
        self.recent_failures.retain(|time| *time > cutoff);
        
        // Add new failure
        self.recent_failures.push_back(now);
        
        // Keep reasonable limit
        if self.recent_failures.len() > 10 {
            self.recent_failures.pop_front();
        }
        
        self.update_condition();
    }
    
    fn update_average(&mut self) {
        if !self.recent_download_times.is_empty() {
            let total: Duration = self.recent_download_times.iter().map(|(d, _)| *d).sum();
            self.average_download_time = total / self.recent_download_times.len() as u32;
        }
    }
    
    fn update_condition(&mut self) {
        let failure_rate = self.recent_failures.len() as f64 / 10.0; // Out of last 10 attempts
        
        if failure_rate > 0.3 {
            self.condition = NetworkCondition::Poor;
        } else if self.average_download_time > Duration::from_secs(2) {
            self.condition = NetworkCondition::Poor;
        } else if self.average_download_time > Duration::from_millis(500) {
            self.condition = NetworkCondition::Fair;
        } else {
            self.condition = NetworkCondition::Good;
        }
    }
    
    /// Get adjusted concurrency limit based on network condition
    pub fn get_concurrency_limit(&self, base_limit: usize) -> usize {
        match self.condition {
            NetworkCondition::Good => base_limit,
            NetworkCondition::Fair => (base_limit * 3 / 4).max(1),
            NetworkCondition::Poor => (base_limit / 2).max(1),
        }
    }
}

/// Adaptive loading configuration
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Enable adaptive loading
    pub enabled: bool,
    /// Maximum prefetch distance in tiles
    pub max_prefetch_distance: u32,
    /// Minimum confidence for prefetching
    pub min_prefetch_confidence: f64,
    /// Maximum number of prefetch tiles
    pub max_prefetch_tiles: usize,
    /// Enable zoom-based priority adjustment
    pub zoom_priority_adjustment: bool,
    /// Network condition adaptation
    pub network_adaptive: bool,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_prefetch_distance: 3,
            min_prefetch_confidence: 0.3,
            max_prefetch_tiles: 50,
            zoom_priority_adjustment: true,
            network_adaptive: true,
        }
    }
}

/// Async tile loader with bounded concurrency and priority queues
pub struct TileLoader {
    /// Channel for sending tile tasks
    task_tx: Sender<TileTask>,
    /// Channel for receiving tile results
    result_rx: Receiver<TileResult>,
    /// Configuration
    config: TileLoaderConfig,
    /// Sequence counter for task ordering
    sequence_counter: std::sync::atomic::AtomicU64,
    /// Adaptive loading configuration
    adaptive_config: Option<AdaptiveConfig>,
    /// Movement pattern tracking
    movement_pattern: Arc<Mutex<MovementPattern>>,
    /// Network performance metrics
    network_metrics: Arc<Mutex<NetworkMetrics>>,
    /// Last known viewport for comparison
    last_viewport: Arc<Mutex<Option<Viewport>>>,
    /// Currently prefetched tiles
    prefetch_tiles: Arc<Mutex<HashSet<TileCoord>>>,
}

impl TileLoader {
    /// Create a new async tile loader
    pub fn new(config: TileLoaderConfig) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (result_tx, result_rx) = unbounded();

        // Start the background worker
        let worker_config = config.clone();
        crate::runtime::spawn(async move {
            TileWorker::new(task_rx, result_tx, worker_config).run().await;
        });

        Self {
            task_tx,
            result_rx,
            config,
            sequence_counter: std::sync::atomic::AtomicU64::new(0),
            adaptive_config: None,
            movement_pattern: Arc::new(Mutex::new(MovementPattern::default())),
            network_metrics: Arc::new(Mutex::new(NetworkMetrics::default())),
            last_viewport: Arc::new(Mutex::new(None)),
            prefetch_tiles: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Create a new tile loader with default configuration
    pub fn with_default_config() -> Self {
        Self::new(TileLoaderConfig::default())
    }

    /// Queue multiple tiles for loading with batch processing
    pub fn queue_tiles_batch(
        &self,
        source: &dyn TileSource,
        coords: Vec<TileCoord>,
        priority: TilePriority,
    ) -> Result<()> {
        let sequence = self.sequence_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Create tasks for all tiles in parallel
        let tasks: Result<Vec<_>> = coords.into_iter().map(|coord| {
                         let url = source.url(coord);
            Ok(TileTask {
                coord,
                url,
                priority,
                sequence,
            })
        }).collect();
        
        let tasks = tasks?;
        
        // Send all tasks in a batch - they will be processed concurrently
        for task in tasks {
            if let Err(e) = self.task_tx.send(task) {
                return Err(format!("Failed to queue tile batch: {}", e).into());
            }
        }
        
        Ok(())
    }

    /// Queue a single tile (keeping for compatibility)
    pub fn queue_tile(
        &self,
        source: &dyn TileSource,
        coord: TileCoord,
        priority: TilePriority,
    ) -> Result<()> {
        self.queue_tiles_batch(source, vec![coord], priority)
    }

    /// Try to receive completed tile results (non-blocking)
    pub fn try_recv_results(&self) -> Vec<TileResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            results.push(result);
        }
        results
    }

    /// Get the result receiver (for polling)
    pub fn result_receiver(&self) -> &Receiver<TileResult> {
        &self.result_rx
    }

    /// Get configuration
    pub fn config(&self) -> &TileLoaderConfig {
        &self.config
    }

    /// Check if there are any pending results without consuming them
    pub fn has_pending_results(&self) -> bool {
        !self.result_rx.is_empty()
    }
    
    /// Create a new tile loader with adaptive configuration
    pub fn with_adaptive_config(config: TileLoaderConfig, adaptive_config: AdaptiveConfig) -> Self {
        let mut loader = Self::new(config);
        loader.adaptive_config = Some(adaptive_config);
        loader
    }

    /// Update viewport and trigger adaptive loading
    pub fn update_viewport(&self, viewport: &Viewport) {
        if let Some(adaptive_config) = &self.adaptive_config {
            if !adaptive_config.enabled {
                return;
            }
            
            // Update movement pattern
            if let Ok(mut pattern) = self.movement_pattern.lock() {
                pattern.update(viewport);
                
                // Get prefetch tiles if confidence is high enough
                if pattern.prediction_confidence >= adaptive_config.min_prefetch_confidence {
                    let prefetch_tiles = pattern.get_prefetch_tiles(viewport);
                    
                    // Limit prefetch tiles
                    let limited_tiles: Vec<_> = prefetch_tiles
                        .into_iter()
                        .take(adaptive_config.max_prefetch_tiles)
                        .collect();
                    
                    // Update prefetch set
                    if let Ok(mut prefetch_set) = self.prefetch_tiles.lock() {
                        prefetch_set.clear();
                        prefetch_set.extend(limited_tiles.iter().cloned());
                    }
                }
            }
            
            // Store last viewport for comparison
            if let Ok(mut last) = self.last_viewport.lock() {
                *last = Some(viewport.clone());
            }
        }
    }
    
    /// Get adaptive priority for a tile based on viewport and zoom
    pub fn get_adaptive_priority(&self, coord: &TileCoord, viewport: &Viewport) -> TilePriority {
        // Base priority calculation
        let zoom_diff = (coord.z as f64 - viewport.zoom).abs();
        
        if zoom_diff > 2.0 {
            return TilePriority::Background;
        }
        
        // Calculate distance from viewport center
        let viewport_bounds = viewport.bounds();
        let tile_center = self.tile_coord_to_lat_lng(coord);
        
        let viewport_center = viewport.center;
        let distance = ((tile_center.lat - viewport_center.lat).powi(2) 
                      + (tile_center.lng - viewport_center.lng).powi(2)).sqrt();
        
        // Check if tile intersects with viewport
        if viewport_bounds.contains(&tile_center) {
            TilePriority::Visible
        } else if distance < 0.1 { // Adjacent to viewport
            TilePriority::Adjacent  
        } else if distance < 0.2 { // Near viewport
            TilePriority::Prefetch
        } else {
            TilePriority::Background
        }
    }
    
    fn tile_coord_to_lat_lng(&self, coord: &TileCoord) -> crate::core::geo::LatLng {
        let n = 2.0_f64.powi(coord.z as i32);
        let lng = coord.x as f64 / n * 360.0 - 180.0;
        let lat_rad = std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n);
        let lat = lat_rad.sinh().atan().to_degrees();
        crate::core::geo::LatLng::new(lat, lng)
    }
}

/// Background worker that processes tile loading tasks
struct TileWorker {
    task_rx: Receiver<TileTask>,
    result_tx: Sender<TileResult>,
    config: TileLoaderConfig,
    /// Simple semaphore to limit concurrent downloads (non-tokio)
    semaphore: Arc<std::sync::Mutex<usize>>,
    /// Priority queue of pending tasks
    task_queue: BinaryHeap<TileTask>,
}

impl TileWorker {
    fn new(
        task_rx: Receiver<TileTask>,
        result_tx: Sender<TileResult>,
        config: TileLoaderConfig,
    ) -> Self {
        let semaphore = Arc::new(std::sync::Mutex::new(config.max_concurrent));
        
        Self {
            task_rx,
            result_tx,
            config,
            semaphore,
            task_queue: BinaryHeap::new(),
        }
    }

    async fn run(mut self) {
        log::debug!("TileWorker starting with max_concurrent: {}", self.config.max_concurrent);
        
        loop {
            // Collect any new tasks
            while let Ok(task) = self.task_rx.try_recv() {
                self.task_queue.push(task);
            }

            // Process the highest priority task if we have capacity
            if let Some(task) = self.task_queue.pop() {
                // Check semaphore availability
                let can_start = {
                    if let Ok(mut count) = self.semaphore.lock() {
                        if *count > 0 {
                            *count -= 1;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if can_start {
                    let result_tx = self.result_tx.clone();
                    let config = self.config.clone();
                    let semaphore = self.semaphore.clone();
                    
                    #[cfg(feature = "debug")]
                    log::debug!("Starting download for tile {:?}", task.coord);
                    
                    crate::runtime::spawn(async move {
                        let result = Self::download_tile(task.clone(), config).await;
                        let tile_result = TileResult {
                            coord: task.coord,
                            data: result,
                        };
                        
                        // Send result back
                        let _ = result_tx.send(tile_result);
                        
                        // Release semaphore permit
                        if let Ok(mut count) = semaphore.lock() {
                            *count += 1;
                        }
                    });
                } else {
                    // No capacity, put task back
                    self.task_queue.push(task);
                }
            }

            // Small delay to yield control (non-blocking)
            #[cfg(feature = "tokio-runtime")]
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            
            #[cfg(not(feature = "tokio-runtime"))]
            {
                // Use a simple async delay that doesn't block
                let start = std::time::Instant::now();
                while start.elapsed() < std::time::Duration::from_millis(10) {
                    // Check for new tasks during delay
                    if let Ok(_) = self.task_rx.try_recv() {
                        break;
                    }
                    std::hint::spin_loop();
                }
            }

            // Check if we should continue (channel still open)
            if self.task_queue.is_empty() {
                // Wait for more tasks or check if channel is closed
                match self.task_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(task) => self.task_queue.push(task),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Continue processing
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        // Channel closed, finish processing remaining tasks
                        if self.task_queue.is_empty() {
                            log::debug!("TileWorker exiting - channel disconnected");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn download_tile(task: TileTask, config: TileLoaderConfig) -> Result<Vec<u8>> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < config.max_retries {
            attempts += 1;
            
            #[cfg(feature = "debug")]
            log::debug!("Downloading tile {:?} (attempt {})", task.coord, attempts);
            
            match HTTP_CLIENT.get(&task.url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.bytes().await {
                            Ok(bytes) => {
                                #[cfg(feature = "debug")]
                                log::debug!("Downloaded tile {:?} ({} bytes)", task.coord, bytes.len());
                                return Ok(bytes.to_vec());
                            }
                            Err(e) => {
                                last_error = Some(format!("Failed to read response body: {}", e));
                            }
                        }
                    } else {
                        last_error = Some(format!("HTTP {}", response.status()));
                    }
                }
                Err(e) => {
                    last_error = Some(format!("Request failed: {}", e));
                }
            }

            if attempts < config.max_retries {
                #[cfg(feature = "debug")]
                log::warn!("Tile {:?} download failed on attempt {}: {}", 
                          task.coord, attempts, last_error.as_ref().unwrap());
                
                // Use async delay
                #[cfg(feature = "tokio-runtime")]
                tokio::time::sleep(config.retry_delay).await;
                
                #[cfg(not(feature = "tokio-runtime"))]
                {
                    let start = std::time::Instant::now();
                    while start.elapsed() < config.retry_delay {
                        std::hint::spin_loop();
                    }
                }
            }
        }

        let error = last_error.unwrap_or_else(|| "Unknown error".to_string());
        #[cfg(feature = "debug")]
        log::error!("Giving up on tile {:?} after {} attempts: {}", 
                   task.coord, attempts, error);
        Err(error.into())
    }
} 