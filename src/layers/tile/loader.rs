use crossbeam_channel::{unbounded, Receiver, Sender};

use super::source::TileSource;
use crate::core::geo::TileCoord;
use crate::core::viewport::Viewport;
use crate::prelude::{Arc, BinaryHeap, Duration, HashSet, Instant, Mutex, Ordering, VecDeque};
use crate::traits::GeometryOps;
use crate::Result;
use once_cell::sync::Lazy;

#[cfg(feature = "debug")]
use log;

/// Shared async HTTP client optimized for tile fetching
pub(crate) static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("maplet/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .connection_verbose(true)
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .pool_max_idle_per_host(16)
        .build()
        .expect("failed to build reqwest async client")
});

/// Priority for tile loading (higher number = higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TilePriority {
    /// Background/low priority
    Background = 1,
    /// Prefetch tiles for predicted movement
    Prefetch = 10,
    /// One ring around visible area
    Adjacent = 50,
    /// Currently visible tiles (highest priority)
    Visible = 100,
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
        match self.priority.cmp(&other.priority) {
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

/// Configuration for the tile loader - MUCH more aggressive defaults
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
            max_concurrent: 128,
            max_retries: 2,
            retry_delay: std::time::Duration::from_millis(25),
        }
    }
}

/// Unified configuration presets for TileLoaderConfig
impl TileLoaderConfig {
    pub fn low_resource() -> Self {
        Self {
            max_concurrent: 16,
            max_retries: 1,
            retry_delay: std::time::Duration::from_millis(50),
        }
    }

    pub fn high_performance() -> Self {
        Self {
            max_concurrent: 256,
            max_retries: 3,
            retry_delay: std::time::Duration::from_millis(10),
        }
    }

    pub fn for_testing() -> Self {
        Self {
            max_concurrent: 4,
            max_retries: 0,
            retry_delay: std::time::Duration::from_millis(500),
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
            recent_centers: VecDeque::with_capacity(20),
            recent_zooms: VecDeque::with_capacity(10),
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

        // OPTIMIZATION: Clean old entries more efficiently by draining from front
        let cutoff = now - Duration::from_secs(3);

        // Use efficient front-draining instead of retain for better performance
        while let Some(&(_, time)) = self.recent_centers.front() {
            if time <= cutoff {
                self.recent_centers.pop_front();
            } else {
                break;
            }
        }

        while let Some(&(_, time)) = self.recent_zooms.front() {
            if time <= cutoff {
                self.recent_zooms.pop_front();
            } else {
                break;
            }
        }

        // Add new entries
        self.recent_centers.push_back((viewport.center, now));
        self.recent_zooms.push_back((viewport.zoom, now));

        // Calculate velocity and prediction
        self.calculate_velocity();
        self.predict_next_position();
    }

    fn calculate_velocity(&mut self) {
        if self.recent_centers.len() < 2 {
            self.velocity = None;
            return;
        }

        // OPTIMIZATION: Use window iterator for pairwise comparison
        let mut total_velocity = crate::core::geo::Point::new(0.0, 0.0);
        let mut count = 0;

        let centers: Vec<_> = self.recent_centers.iter().collect();
        for window in centers.windows(2) {
            let (prev_pos, prev_time) = window[0];
            let (curr_pos, curr_time) = window[1];
            let time_diff = curr_time.duration_since(*prev_time).as_secs_f64();

            if time_diff > 0.0 {
                let lat_diff = curr_pos.lat - prev_pos.lat;
                let lng_diff = curr_pos.lng - prev_pos.lng;

                // Convert to rough pixel velocity (this is approximate)
                let velocity_x = lng_diff / time_diff;
                let velocity_y = lat_diff / time_diff;

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
        } else {
            self.velocity = None;
        }
    }

    fn predict_next_position(&mut self) {
        if let Some(velocity) = self.velocity {
            if let Some(&(last_center, _last_time)) = self.recent_centers.back() {
                // OPTIMIZATION: Use simple linear prediction with confidence based on data consistency
                let prediction_time = 1.0; // Predict 1 second ahead

                let predicted_lat = last_center.lat + velocity.y * prediction_time;
                let predicted_lng = last_center.lng + velocity.x * prediction_time;

                self.predicted_center =
                    Some(crate::core::geo::LatLng::new(predicted_lat, predicted_lng));

                // OPTIMIZATION: Calculate confidence based on velocity consistency
                let velocity_magnitude = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
                self.prediction_confidence = (velocity_magnitude * 100.0).clamp(0.0, 1.0);
            }
        } else {
            self.predicted_center = None;
            self.prediction_confidence = 0.0;
        }
    }

    /// Get prefetch tiles with optimized tile generation
    pub fn get_prefetch_tiles(&self, current_viewport: &Viewport) -> Vec<TileCoord> {
        if self.prediction_confidence < 0.3 {
            return Vec::new();
        }

        let Some(predicted_center) = self.predicted_center else {
            return Vec::new();
        };

        // OPTIMIZATION: Pre-allocate result vector with estimated capacity
        let mut tiles = Vec::with_capacity(50);

        // Create a predicted viewport
        let predicted_viewport = Viewport::new(
            predicted_center,
            current_viewport.zoom,
            current_viewport.size,
        );

        // Get tiles for predicted viewport with a small buffer
        let buffer = 1;
        let zoom = current_viewport.zoom.floor() as u32;
        let predicted_tiles = self.get_aggressive_buffer_tiles(&predicted_viewport, zoom, buffer);

        // OPTIMIZATION: Extend instead of individual pushes
        tiles.extend(predicted_tiles);

        // OPTIMIZATION: Also prefetch at adjacent zoom levels for smooth transitions
        if zoom > 0 {
            let lower_zoom_tiles = self.get_zoom_level_tiles(&predicted_viewport, zoom - 1, buffer);
            tiles.extend(lower_zoom_tiles);
        }

        if zoom < 18 {
            let higher_zoom_tiles =
                self.get_zoom_level_tiles(&predicted_viewport, zoom + 1, buffer);
            tiles.extend(higher_zoom_tiles);
        }

        tiles
    }

    fn get_aggressive_buffer_tiles(
        &self,
        viewport: &Viewport,
        zoom: u32,
        buffer: u32,
    ) -> Vec<TileCoord> {
        let bounds = viewport.bounds();

        let nw_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.north_east.lat, bounds.south_west.lng),
            Some(zoom as f64),
        );
        let se_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.south_west.lat, bounds.north_east.lng),
            Some(zoom as f64),
        );

        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as i32 - buffer as i32;
        let max_x = (se_proj.x / tile_size).ceil() as i32 + buffer as i32;
        let min_y = (nw_proj.y / tile_size).floor() as i32 - buffer as i32;
        let max_y = (se_proj.y / tile_size).ceil() as i32 + buffer as i32;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;

        // OPTIMIZATION: Pre-calculate result size and allocate
        let width = (max_x - min_x + 1).max(0) as usize;
        let height = (max_y - min_y + 1).max(0) as usize;
        let mut tiles = Vec::with_capacity(width * height);

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if x >= 0 && y >= 0 && x < max_tile && y < max_tile {
                    tiles.push(TileCoord {
                        x: x as u32,
                        y: y as u32,
                        z: zoom as u8,
                    });
                }
            }
        }
        tiles
    }

    /// Get tiles for a specific zoom level
    fn get_zoom_level_tiles(&self, viewport: &Viewport, zoom: u32, buffer: u32) -> Vec<TileCoord> {
        self.get_aggressive_buffer_tiles(viewport, zoom, buffer)
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
    Good, // < 500ms average
    Fair, // 500ms - 2s average
    Poor, // > 2s average
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            recent_download_times: VecDeque::with_capacity(100),
            average_download_time: Duration::from_millis(500),
            condition: NetworkCondition::Good,
            recent_failures: VecDeque::with_capacity(50),
        }
    }
}

impl NetworkMetrics {
    /// Record a successful download
    pub fn record_success(&mut self, duration: Duration) {
        let now = Instant::now();

        // OPTIMIZATION: Use efficient front-draining
        let cutoff = now - Duration::from_secs(30);
        while let Some(&(_, time)) = self.recent_download_times.front() {
            if time <= cutoff {
                self.recent_download_times.pop_front();
            } else {
                break;
            }
        }

        self.recent_download_times.push_back((duration, now));
        self.update_average();
        self.update_condition();
    }

    /// Record a failed download
    pub fn record_failure(&mut self) {
        let now = Instant::now();

        // OPTIMIZATION: Use efficient front-draining
        let cutoff = now - Duration::from_secs(30);
        while let Some(&time) = self.recent_failures.front() {
            if time <= cutoff {
                self.recent_failures.pop_front();
            } else {
                break;
            }
        }

        self.recent_failures.push_back(now);
        self.update_condition();
    }

    fn update_average(&mut self) {
        if self.recent_download_times.is_empty() {
            return;
        }

        // OPTIMIZATION: Use iterator instead of collecting
        let total: Duration = self
            .recent_download_times
            .iter()
            .map(|(duration, _)| *duration)
            .sum();
        self.average_download_time = total / self.recent_download_times.len() as u32;
    }

    fn update_condition(&mut self) {
        let failure_rate = self.recent_failures.len() as f64
            / (self.recent_download_times.len() + self.recent_failures.len()).max(1) as f64;

        if failure_rate > 0.5 || self.average_download_time > Duration::from_secs(2) {
            self.condition = NetworkCondition::Poor;
        } else if failure_rate > 0.2 || self.average_download_time > Duration::from_millis(500) {
            self.condition = NetworkCondition::Fair;
        } else {
            self.condition = NetworkCondition::Good;
        }
    }

    /// Get recommended concurrency limit based on network condition
    pub fn get_concurrency_limit(&self, base_limit: usize) -> usize {
        match self.condition {
            NetworkCondition::Good => base_limit,
            NetworkCondition::Fair => base_limit * 2 / 3,
            NetworkCondition::Poor => base_limit / 2,
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
            max_prefetch_distance: 4,
            min_prefetch_confidence: 0.2,
            max_prefetch_tiles: 100,
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
    /// Last known viewport for comparison
    last_viewport: Arc<Mutex<Option<Viewport>>>,
    /// Currently prefetched tiles
    prefetch_tiles: Arc<Mutex<HashSet<TileCoord>>>,
    /// Currently pending/downloading tiles to prevent duplicates
    pending_tiles: Arc<Mutex<HashSet<TileCoord>>>,
    /// Network performance metrics
    network_metrics: Arc<Mutex<NetworkMetrics>>,
    /// Background task manager for aggressive prefetching
    bg_task_manager: Option<Arc<crate::background::BackgroundTaskManager>>,
}

impl TileLoader {
    /// Create a new async tile loader
    pub fn new(config: TileLoaderConfig) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (result_tx, result_rx) = unbounded();

        // Start the background worker
        let worker_config = config.clone();
        crate::runtime::spawn(async move {
            TileWorker::new(task_rx, result_tx, worker_config)
                .run()
                .await;
        });

        Self {
            task_tx,
            result_rx,
            config,
            sequence_counter: std::sync::atomic::AtomicU64::new(0),
            adaptive_config: None,
            movement_pattern: Arc::new(Mutex::new(MovementPattern::default())),
            last_viewport: Arc::new(Mutex::new(None)),
            prefetch_tiles: Arc::new(Mutex::new(HashSet::default())),
            pending_tiles: Arc::new(Mutex::new(HashSet::default())),
            network_metrics: Arc::new(Mutex::new(NetworkMetrics::default())),
            bg_task_manager: None,
        }
    }

    /// Create a new tile loader with default configuration
    pub fn with_default_config() -> Self {
        Self::new(TileLoaderConfig::default())
    }

    /// Queue multiple tiles for loading with batch processing and deduplication
    pub fn queue_tiles_batch(
        &self,
        source: &dyn TileSource,
        coords: Vec<TileCoord>,
        priority: TilePriority,
    ) -> Result<()> {
        let sequence = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Filter out tiles that are already pending to prevent duplicates
        let filtered_coords: Vec<TileCoord> = if let Ok(mut pending) = self.pending_tiles.try_lock()
        {
            coords
                .into_iter()
                .filter(|coord| {
                    if pending.contains(coord) {
                        #[cfg(feature = "debug")]
                        log::debug!("Skipping duplicate tile request: {:?}", coord);
                        false
                    } else {
                        pending.insert(*coord);
                        true
                    }
                })
                .collect()
        } else {
            // If we can't lock, just proceed with all coords (fallback)
            coords
        };

        if filtered_coords.is_empty() {
            return Ok(());
        }

        // Create tasks for filtered tiles
        let tasks: Result<Vec<_>> = filtered_coords
            .into_iter()
            .enumerate()
            .map(|(i, coord)| {
                let url = source.url(coord);
                Ok(TileTask {
                    coord,
                    url,
                    priority,
                    // For visible tiles, use reverse sequence to process in queue order
                    sequence: if priority == TilePriority::Visible {
                        sequence.saturating_sub(i as u64)
                    } else {
                        sequence + i as u64
                    },
                })
            })
            .collect();

        let mut tasks = tasks?;

        // Sort by priority (visible tiles first) and then by sequence (like Leaflet's distance sorting)
        tasks.sort_by(|a, b| match b.priority.cmp(&a.priority) {
            std::cmp::Ordering::Equal => a.sequence.cmp(&b.sequence),
            other => other,
        });

        // Send all tasks in priority order - visible tiles get processed first
        for task in tasks {
            let coord = task.coord; // Store coord before moving task
            if let Err(e) = self.task_tx.send(task) {
                // Remove from pending if send fails
                if let Ok(mut pending) = self.pending_tiles.try_lock() {
                    pending.remove(&coord);
                }
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
            // Remove completed tile from pending set
            if let Ok(mut pending) = self.pending_tiles.try_lock() {
                pending.remove(&result.coord);
            }
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

    /// Enable background task manager for ultra-aggressive prefetching
    pub fn with_background_task_manager(
        mut self,
        bg_task_manager: Arc<crate::background::BackgroundTaskManager>,
    ) -> Self {
        self.bg_task_manager = Some(bg_task_manager);
        self
    }

    /// Create a tile loader with high-performance preset and background task manager
    pub fn with_high_performance_preset(
        bg_task_manager: Arc<crate::background::BackgroundTaskManager>,
    ) -> Self {
        let config = TileLoaderConfig::high_performance();
        let adaptive_config = AdaptiveConfig {
            enabled: true,
            max_prefetch_distance: 8,     // Very aggressive
            min_prefetch_confidence: 0.1, // Lower threshold
            max_prefetch_tiles: 3000,     // Much higher limit
            zoom_priority_adjustment: true,
            network_adaptive: true,
        };

        Self::with_adaptive_config(config, adaptive_config)
            .with_background_task_manager(bg_task_manager)
    }

    /// Update movement pattern with new viewport and trigger prefetching
    pub fn update_viewport(&self, viewport: &Viewport) {
        // Update movement pattern tracking
        if let Ok(mut pattern) = self.movement_pattern.lock() {
            pattern.update(viewport);
        }

        // Update last known viewport
        if let Ok(mut last_viewport) = self.last_viewport.lock() {
            *last_viewport = Some(viewport.clone());
        }

        // Trigger intelligent prefetching if adaptive config is enabled
        if let Some(adaptive_config) = &self.adaptive_config {
            if adaptive_config.enabled {
                self.trigger_smart_prefetch(viewport);
            }
        }
    }

    /// Trigger smart prefetching based on current viewport and movement patterns
    fn trigger_smart_prefetch(&self, viewport: &Viewport) {
        if let Some(adaptive_config) = &self.adaptive_config {
            let current_zoom = viewport.zoom.round() as u32;
            let max_tiles = adaptive_config.max_prefetch_tiles;

            // Calculate prefetch tiles based on movement pattern
            let prefetch_coords = if let Ok(pattern) = self.movement_pattern.lock() {
                pattern.get_prefetch_tiles(viewport)
            } else {
                // Fallback to basic surrounding tiles
                self.get_basic_prefetch_tiles(viewport, current_zoom)
            };

            // Limit the number of prefetch tiles
            let limited_coords: Vec<_> = prefetch_coords.into_iter().take(max_tiles).collect();

            // If background task manager is available, use it for ultra-aggressive prefetching
            if let Some(bg_task_manager) = &self.bg_task_manager {
                self.trigger_background_prefetch(bg_task_manager, &limited_coords, viewport);
            }

            // Update prefetch tracking
            if let Ok(mut prefetch_tiles) = self.prefetch_tiles.lock() {
                prefetch_tiles.clear();
                prefetch_tiles.extend(limited_coords.iter().cloned());
            }

            // Queue prefetch tiles with appropriate priority
            for coord in limited_coords {
                let _priority = self.get_adaptive_priority(&coord, viewport);
                // Note: This would need a tile source reference to actually queue
                // The actual implementation should receive a tile source parameter
            }
        }
    }

    /// Trigger intelligent background prefetching using the background task manager
    fn trigger_background_prefetch(
        &self,
        bg_task_manager: &Arc<crate::background::BackgroundTaskManager>,
        coords: &[TileCoord],
        viewport: &Viewport,
    ) {
        // Create a background task for intelligent tile prefetching
        let prefetch_task = TilePrefetchTask::new(
            format!("prefetch_{}_{}", viewport.center.lat, viewport.center.lng),
            coords.to_vec(),
            viewport.clone(),
            self.network_metrics.clone(),
        )
        .with_priority(crate::background::TaskPriority::High);

        // Submit the task with high priority
        if let Err(e) = bg_task_manager.submit_task(Arc::new(prefetch_task)) {
            #[cfg(feature = "debug")]
            log::warn!("Failed to submit background prefetch task: {}", e);
        }
    }

    /// Get basic prefetch tiles when movement pattern is not available
    fn get_basic_prefetch_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();

        // Use unified projection instead of duplicate Web Mercator calculations
        let nw_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.north_east.lat, bounds.south_west.lng),
            Some(zoom as f64),
        );
        let se_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.south_west.lat, bounds.north_east.lng),
            Some(zoom as f64),
        );

        let tile_size = 256.0;
        let padding = 1; // 1 tile padding around visible area
        let min_x = (nw_proj.x / tile_size).floor() as i32 - padding;
        let max_x = (se_proj.x / tile_size).ceil() as i32 + padding;
        let min_y = (nw_proj.y / tile_size).floor() as i32 - padding;
        let max_y = (se_proj.y / tile_size).ceil() as i32 + padding;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if x >= 0 && y >= 0 && x < max_tile && y < max_tile {
                    tiles.push(TileCoord {
                        x: x as u32,
                        y: y as u32,
                        z: zoom as u8,
                    });
                }
            }
        }

        // Add tiles for ±1 zoom levels
        let mut multi_zoom_tiles = tiles.clone();

        if zoom > 0 {
            multi_zoom_tiles.extend(self.get_zoom_level_tiles(viewport, zoom - 1));
        }
        if zoom < 18 {
            multi_zoom_tiles.extend(self.get_zoom_level_tiles(viewport, zoom + 1));
        }

        multi_zoom_tiles
    }

    /// Get tiles for a specific zoom level covering the viewport
    fn get_zoom_level_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();

        // Use unified projection instead of duplicate Web Mercator calculations
        let nw_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.north_east.lat, bounds.south_west.lng),
            Some(zoom as f64),
        );
        let se_proj = viewport.project(
            &crate::core::geo::LatLng::new(bounds.south_west.lat, bounds.north_east.lng),
            Some(zoom as f64),
        );

        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as u32;
        let max_x = (se_proj.x / tile_size).ceil() as u32;
        let min_y = (nw_proj.y / tile_size).floor() as u32;
        let max_y = (se_proj.y / tile_size).ceil() as u32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                tiles.push(TileCoord {
                    x,
                    y,
                    z: zoom as u8,
                });
            }
        }
        tiles
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
            + (tile_center.lng - viewport_center.lng).powi(2))
        .sqrt();

        // Check if tile intersects with viewport
        if viewport_bounds.contains_point(&tile_center) {
            TilePriority::Visible
        } else if distance < 0.1 {
            // Adjacent to viewport
            TilePriority::Adjacent
        } else if distance < 0.2 {
            // Near viewport
            TilePriority::Prefetch
        } else {
            TilePriority::Background
        }
    }

    fn tile_coord_to_lat_lng(&self, coord: &TileCoord) -> crate::core::geo::LatLng {
        // Use unified unprojection instead of duplicate Web Mercator calculations
        let tile_size = 256.0;
        let pixel_x = coord.x as f64 * tile_size + tile_size / 2.0;
        let pixel_y = coord.y as f64 * tile_size + tile_size / 2.0;
        let pixel_point = crate::core::geo::Point::new(pixel_x, pixel_y);

        // Use a default viewport for coordinate conversion (we only need the projection math)
        let temp_viewport = crate::core::viewport::Viewport::default();
        temp_viewport.unproject(&pixel_point, Some(coord.z as f64))
    }

    /// Get zoom trend for smart prefetching (positive = zooming in, negative = zooming out)
    pub fn get_zoom_trend(&self) -> f64 {
        if let Ok(pattern) = self.movement_pattern.lock() {
            let recent_zooms: Vec<_> = pattern.recent_zooms.iter().collect();

            if recent_zooms.len() >= 2 {
                recent_zooms
                    .windows(2)
                    .map(|window| window[1].0 - window[0].0)
                    .sum::<f64>()
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Get prediction confidence for movement
    pub fn get_prediction_confidence(&self) -> f64 {
        if let Ok(pattern) = self.movement_pattern.lock() {
            pattern.prediction_confidence
        } else {
            0.0
        }
    }

    /// Get the number of currently pending tile requests (for debugging)
    pub fn get_pending_count(&self) -> usize {
        if let Ok(pending) = self.pending_tiles.try_lock() {
            pending.len()
        } else {
            0
        }
    }

    /// Clear all pending tiles (useful for cleanup or reset)
    pub fn clear_pending(&self) {
        if let Ok(mut pending) = self.pending_tiles.try_lock() {
            pending.clear();
        }
    }

    /// Update the tile loader configuration (creates new loader with updated config)
    pub fn update_config(&self, new_config: TileLoaderConfig) -> Self {
        Self::new(new_config)
    }
}

/// Background task for intelligent tile prefetching based on movement patterns
pub struct TilePrefetchTask {
    task_id: String,
    coords: Vec<TileCoord>,
    viewport: Viewport,
    network_metrics: Arc<Mutex<NetworkMetrics>>,
    priority: crate::background::TaskPriority,
}

impl TilePrefetchTask {
    pub fn new(
        task_id: String,
        coords: Vec<TileCoord>,
        viewport: Viewport,
        network_metrics: Arc<Mutex<NetworkMetrics>>,
    ) -> Self {
        Self {
            task_id,
            coords,
            viewport,
            network_metrics,
            priority: crate::background::TaskPriority::High,
        }
    }

    pub fn with_priority(mut self, priority: crate::background::TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl crate::background::BackgroundTask for TilePrefetchTask {
    fn execute(
        &self,
    ) -> crate::prelude::Pin<
        Box<
            dyn crate::prelude::Future<Output = crate::Result<Box<dyn std::any::Any + Send>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let coords = self.coords.clone();
            let viewport = self.viewport.clone();
            let network_metrics = self.network_metrics.clone();

            // Execute prefetch logic in background
            let result =
                crate::background::tasks::AsyncExecutor::execute_blocking_boxed(move || {
                    // Simulate aggressive prefetching analysis
                    let mut prefetch_results = Vec::new();

                    // Get network condition to adjust strategy
                    let network_condition = if let Ok(metrics) = network_metrics.lock() {
                        metrics.condition.clone()
                    } else {
                        NetworkCondition::Good
                    };

                    // Adjust prefetch strategy based on network condition
                    let coords_len = coords.len();
                    let coords_to_prefetch = match network_condition {
                        NetworkCondition::Good => coords, // All tiles
                        NetworkCondition::Fair => {
                            coords.into_iter().take(coords_len * 2 / 3).collect()
                        }
                        NetworkCondition::Poor => coords.into_iter().take(coords_len / 2).collect(),
                    };

                    // Group tiles by zoom level for efficient prefetching
                    let mut zoom_groups = std::collections::HashMap::new();
                    for coord in coords_to_prefetch {
                        zoom_groups
                            .entry(coord.z)
                            .or_insert_with(Vec::new)
                            .push(coord);
                    }

                    // Prioritize current zoom level and adjacent levels
                    let current_zoom = viewport.zoom.round() as u8;
                    let mut priority_order = vec![current_zoom];

                    // Add adjacent zoom levels
                    if current_zoom > 0 {
                        priority_order.push(current_zoom - 1);
                    }
                    if current_zoom < 18 {
                        priority_order.push(current_zoom + 1);
                    }

                    // Add other zoom levels
                    for zoom in zoom_groups.keys() {
                        if !priority_order.contains(zoom) {
                            priority_order.push(*zoom);
                        }
                    }

                    // Build prefetch recommendation
                    for zoom in priority_order {
                        if let Some(tiles) = zoom_groups.get(&zoom) {
                            prefetch_results.extend(tiles.iter().cloned());
                        }
                    }

                    Ok(prefetch_results)
                })
                .await?;

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> crate::background::TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimation based on number of tiles to analyze
        let base_time = std::time::Duration::from_millis(1);
        let coord_factor = (self.coords.len() / 100).max(1) as u32;
        base_time * coord_factor.min(50) // Cap at 50ms
    }
}

/// Background worker that processes tile loading tasks
struct TileWorker {
    task_rx: Receiver<TileTask>,
    result_tx: Sender<TileResult>,
    config: TileLoaderConfig,
    /// Unified semaphore to limit concurrent downloads
    semaphore: crate::runtime::async_utils::Semaphore,
    /// Priority queue of pending tasks
    task_queue: BinaryHeap<TileTask>,
}

impl TileWorker {
    fn new(
        task_rx: Receiver<TileTask>,
        result_tx: Sender<TileResult>,
        config: TileLoaderConfig,
    ) -> Self {
        let semaphore = crate::runtime::async_utils::Semaphore::new(config.max_concurrent);

        Self {
            task_rx,
            result_tx,
            config,
            semaphore,
            task_queue: BinaryHeap::new(),
        }
    }

    async fn run(mut self) {
        log::debug!(
            "TileWorker starting with max_concurrent: {}",
            self.config.max_concurrent
        );

        loop {
            // Collect any new tasks
            while let Ok(task) = self.task_rx.try_recv() {
                self.task_queue.push(task);
            }

            // Process the highest priority task if we have capacity
            if let Some(task) = self.task_queue.pop() {
                // Check semaphore availability using unified semaphore
                let can_start = self.semaphore.try_acquire();

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

                        // Release semaphore permit using unified semaphore
                        semaphore.release();
                    });
                } else {
                    // No capacity, put task back
                    self.task_queue.push(task);
                }
            }

            // Small delay to yield control using unified async delay
            crate::runtime::async_utils::async_delay(std::time::Duration::from_millis(10)).await;

            // Check if we should continue (channel still open)
            if self.task_queue.is_empty() {
                // Wait for more tasks or check if channel is closed
                match self
                    .task_rx
                    .recv_timeout(std::time::Duration::from_millis(100))
                {
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

    async fn download_tile(task: TileTask, _config: TileLoaderConfig) -> Result<Vec<u8>> {
        // Set up timeout for the request - use a reasonable timeout for network requests
        let request_timeout = std::time::Duration::from_secs(10); // 10 seconds is reasonable for tile downloads
        let client = &*HTTP_CLIENT;
        let response = client
            .get(&task.url)
            .timeout(request_timeout)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("HTTP {} for tile {:?}", response.status(), task.coord).into());
        }

        let data = response.bytes().await.map_err(|e| e.to_string())?.to_vec();

        Ok(data)
    }
}
