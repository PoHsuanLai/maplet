use crate::{
    core::{
        config::TileLoadingConfig,
        geo::{LatLng, LatLngBounds, Point, TileCoord},
        viewport::Viewport,
    },
    layers::base::{LayerProperties, LayerTrait, LayerType},
    tiles::{
        cache::TileCache,
        loader::{TileLoader, TilePriority},
        source::TileSource,
    },
    Result,
};
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};

#[cfg(feature = "render")]
use crate::rendering::context::RenderContext;

#[cfg(feature = "debug")]
use log;

/// Configuration for a tile layer
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TileLayerOptions {
    /// URL template for tiles (e.g., "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png")
    pub url_template: String,
    /// Available subdomains for load balancing
    pub subdomains: Vec<String>,
    /// Attribution text
    pub attribution: String,
    /// Tile size in pixels
    pub tile_size: u32,
    /// Maximum zoom level for this tile source
    pub max_zoom: u8,
    /// Minimum zoom level for this tile source
    pub min_zoom: u8,
    /// Whether to show tiles outside zoom range
    pub bounds: Option<LatLngBounds>,
    /// HTTP headers to send with tile requests
    pub headers: HashMap<String, String>,
    /// Cross-origin policy
    pub crossorigin: Option<String>,
    /// Whether to use HTTPS
    pub force_https: bool,
    /// Whether to enable fade animations
    pub fade_animation: bool,
    /// Update tiles when zooming (smooth zoom)
    pub update_when_zooming: bool,
}

impl Default for TileLayerOptions {
    fn default() -> Self {
        Self {
            url_template: "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            subdomains: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            attribution: "© OpenStreetMap contributors".to_string(),
            tile_size: 256,
            max_zoom: 18,
            min_zoom: 0,
            bounds: None,
            headers: HashMap::new(),
            crossorigin: None,
            force_https: true,
            fade_animation: true,
            update_when_zooming: true,
        }
    }
}

/// Represents a zoom level with its tiles
#[derive(Debug, Clone)]
pub struct TileLevel {
    /// Zoom level
    pub zoom: u8,
    /// Tiles at this level
    pub tiles: HashMap<TileCoord, TileState>,
    /// Transform scale for smooth zoom animations
    pub scale: f64,
    /// Transform translation for positioning
    pub translation: Point,
    /// Whether this level is active
    pub active: bool,
}

impl TileLevel {
    pub fn new(zoom: u8) -> Self {
        Self {
            zoom,
            tiles: HashMap::new(),
            scale: 1.0,
            translation: Point::new(0.0, 0.0),
            active: false,
        }
    }
}

/// State of a tile in the tile system
#[derive(Debug, Clone)]
pub struct TileState {
    /// Tile coordinate
    pub coord: TileCoord,
    /// Tile data if loaded
    pub data: Option<Arc<Vec<u8>>>,
    /// Loading state
    pub loading: bool,
    /// Load error if any
    pub error: Option<String>,
    /// Whether tile is current (visible in viewport)
    pub current: bool,
    /// Whether tile should be retained
    pub retain: bool,
    /// Opacity for fade animations (0.0 to 1.0)
    pub opacity: f32,
    /// Time when tile finished loading (for fade animation)
    pub loaded_time: Option<std::time::Instant>,
    /// Number of retry attempts made
    pub retry_count: u32,
    /// Last retry time for exponential backoff
    pub last_retry_time: Option<std::time::Instant>,
    /// Parent tile data to show while loading (for smooth zoom)
    pub parent_data: Option<Arc<Vec<u8>>>,
    /// Whether this tile should show parent data
    pub show_parent: bool,
}

impl TileState {
    pub fn new(coord: TileCoord) -> Self {
        Self {
            coord,
            data: None,
            loading: false,
            error: None,
            current: false,
            retain: false,
            opacity: 0.0,
            loaded_time: None,
            retry_count: 0,
            last_retry_time: None,
            parent_data: None,
            show_parent: false,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn has_display_data(&self) -> bool {
        self.data.is_some() || (self.show_parent && self.parent_data.is_some())
    }

    pub fn get_display_data(&self) -> Option<&Arc<Vec<u8>>> {
        if let Some(ref data) = self.data {
            Some(data)
        } else if self.show_parent {
            self.parent_data.as_ref()
        } else {
            None
        }
    }

    pub fn mark_loaded(&mut self, data: Arc<Vec<u8>>) {
        self.data = Some(data);
        self.loading = false;
        self.error = None;
        self.retry_count = 0;
        self.loaded_time = Some(std::time::Instant::now());
    }

    pub fn mark_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.retry_count += 1;
        self.last_retry_time = Some(std::time::Instant::now());
    }

    pub fn should_retry(
        &self,
        max_retries: u32,
        retry_delay_ms: u64,
        exponential_backoff: bool,
    ) -> bool {
        if self.retry_count >= max_retries {
            return false;
        }

        if let Some(last_retry) = self.last_retry_time {
            let delay_multiplier = if exponential_backoff {
                2_u64.pow(self.retry_count)
            } else {
                1
            };
            let required_delay = retry_delay_ms * delay_multiplier;
            last_retry.elapsed().as_millis() >= required_delay as u128
        } else {
            true
        }
    }

    pub fn set_parent_data(&mut self, parent_data: Option<Arc<Vec<u8>>>) {
        self.show_parent = parent_data.is_some() && self.data.is_none();
        self.parent_data = parent_data;
    }
}

/// A tile-based layer that displays map tiles from a tile server
pub struct TileLayer {
    /// Base layer properties
    properties: LayerProperties,
    /// Tile layer specific options
    options: TileLayerOptions,
    /// Tile source for fetching tiles
    tile_source: Box<dyn TileSource>,
    /// Tile loader for async tile fetching
    tile_loader: TileLoader,
    /// Tile cache for storing downloaded tiles
    tile_cache: TileCache,
    /// Tile levels (different zoom levels)
    levels: HashMap<u8, TileLevel>,
    /// Current tile zoom level
    tile_zoom: Option<u8>,
    /// Whether currently loading
    loading: bool,
    /// Last update time for animation control
    last_update: std::time::Instant,
}

impl TileLayer {
    /// Create a new tile layer with default OpenStreetMap tiles
    pub fn new(id: String, name: String) -> Self {
        Self::with_options(id, name, TileLayerOptions::default())
    }

    /// Create a new tile layer with custom options
    pub fn with_options(id: String, name: String, options: TileLayerOptions) -> Self {
        let properties = LayerProperties::new(id.clone(), name, LayerType::Tile);
        let tile_source: Box<dyn TileSource> =
            Box::new(crate::tiles::source::OpenStreetMapSource::default());

        Self {
            properties,
            tile_source,
            tile_loader: TileLoader::with_default_config(),
            tile_cache: TileCache::with_default_capacity(),
            options,
            levels: HashMap::new(),
            tile_zoom: None,
            loading: false,
            last_update: std::time::Instant::now(),
        }
    }

    /// Create a tile layer for OpenStreetMap
    pub fn openstreetmap(id: String, name: String) -> Self {
        let options = TileLayerOptions {
            url_template: "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            attribution: "© OpenStreetMap contributors".to_string(),
            ..Default::default()
        };
        Self::with_options(id, name, options)
    }

    /// Create a tile layer for satellite imagery
    pub fn satellite(id: String, name: String) -> Self {
        let options = TileLayerOptions {
            url_template: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}".to_string(),
            subdomains: vec![], // ArcGIS doesn't use subdomains
            attribution: "© Esri, Maxar, GeoEye, Earthstar Geographics, CNES/Airbus DS, USDA, USGS, AeroGRID, IGN, and the GIS User Community".to_string(),
            ..Default::default()
        };
        Self::with_options(id, name, options)
    }

    /// Get tiles that should be visible in the current viewport
    fn get_visible_tiles(&self, viewport: &Viewport) -> Vec<TileCoord> {
        // Determine the appropriate zoom level within layer limits
        let zoom = viewport.zoom.floor() as u8;
        let clamped_zoom = zoom.clamp(self.options.min_zoom, self.options.max_zoom);

        // Get tiled pixel bounds (matches Leaflet's _getTiledPixelBounds)
        let center = viewport.center;
        let pixel_center = viewport.project(&center, Some(clamped_zoom as f64)).floor();
        let scale = 2_f64.powf(viewport.zoom - clamped_zoom as f64);
        let half_size = Point::new(
            viewport.size.x / (scale * 2.0),
            viewport.size.y / (scale * 2.0),
        );

        let pixel_bounds_min = pixel_center.subtract(&half_size);
        let pixel_bounds_max = pixel_center.add(&half_size);

        // Convert pixel bounds to tile range (matches Leaflet's _pxBoundsToTileRange)
        let tile_size = self.options.tile_size as f64;
        let min_tile_x = (pixel_bounds_min.x / tile_size).floor() as i32;
        let max_tile_x = (pixel_bounds_max.x / tile_size).floor() as i32;
        let min_tile_y = (pixel_bounds_min.y / tile_size).floor() as i32;
        let max_tile_y = (pixel_bounds_max.y / tile_size).floor() as i32;

        // Add buffer margin for smooth scrolling
        let margin = 1;
        let min_x = min_tile_x - margin;
        let max_x = max_tile_x + margin;
        let min_y = min_tile_y - margin;
        let max_y = max_tile_y + margin;

        // Calculate the tile range for this zoom level
        let tiles_per_axis = 1u32 << clamped_zoom;
        let max_tile_index = tiles_per_axis as i32 - 1;

        let mut tiles = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                // Clamp Y coordinates (no wrapping for Y)
                if y < 0 || y > max_tile_index {
                    continue;
                }

                // Wrap X coordinates around the world (matches Leaflet's _wrapCoords)
                let mut wrapped_x = x;
                while wrapped_x < 0 {
                    wrapped_x += tiles_per_axis as i32;
                }
                while wrapped_x > max_tile_index {
                    wrapped_x -= tiles_per_axis as i32;
                }

                let tile_coord = TileCoord {
                    x: wrapped_x as u32,
                    y: y as u32,
                    z: clamped_zoom,
                };

                // Only add valid tiles
                if self.is_valid_tile(&tile_coord) {
                    tiles.push(tile_coord);
                }
            }
        }

        tiles
    }

    /// Check if a tile coordinate is valid for this layer
    fn is_valid_tile(&self, coord: &TileCoord) -> bool {
        // Check if within layer bounds if specified
        if let Some(bounds) = &self.options.bounds {
            let tile_bounds = self.tile_bounds(coord);
            if !bounds.intersects(&tile_bounds) {
                return false;
            }
        }

        // Check zoom level
        if coord.z < self.options.min_zoom || coord.z > self.options.max_zoom {
            return false;
        }

        // Check if tile coordinates are within valid range for this zoom level
        let tiles_per_axis = 1u32 << coord.z;
        coord.x < tiles_per_axis && coord.y < tiles_per_axis
    }

    /// Get the geographical bounds of a tile
    fn tile_bounds(&self, coord: &TileCoord) -> LatLngBounds {
        let tile_size = self.options.tile_size as f64;
        let scale = 256.0 * 2_f64.powf(coord.z as f64);

        // Calculate pixel coordinates of tile corners
        let nw_x = coord.x as f64 * tile_size;
        let nw_y = coord.y as f64 * tile_size;
        let se_x = nw_x + tile_size;
        let se_y = nw_y + tile_size;

        // Convert to geographical coordinates using spherical mercator
        let d = 180.0 / std::f64::consts::PI;

        let nw_lng = nw_x / scale * 360.0 - 180.0;
        let nw_lat_rad = std::f64::consts::FRAC_PI_2
            - 2.0
                * ((0.5 - nw_y / scale) * 2.0 * std::f64::consts::PI)
                    .exp()
                    .atan();
        let nw_lat = nw_lat_rad * d;

        let se_lng = se_x / scale * 360.0 - 180.0;
        let se_lat_rad = std::f64::consts::FRAC_PI_2
            - 2.0
                * ((0.5 - se_y / scale) * 2.0 * std::f64::consts::PI)
                    .exp()
                    .atan();
        let se_lat = se_lat_rad * d;

        LatLngBounds::new(
            LatLng::new(se_lat, nw_lng), // south-west
            LatLng::new(nw_lat, se_lng), // north-east
        )
    }

    /// Update tiles for the current viewport (main tile management method)
    pub fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        let target_zoom = viewport.zoom.floor() as u8;

        // Update levels first
        self.update_levels(viewport, target_zoom)?;
        self.update_zoom_transforms(viewport);

        // Process any completed tile loads/errors
        self.process_tile_results()?;

        // Handle retries for failed tiles
        self.handle_tile_retries()?;

        // Update parent tile data for smooth zoom
        if let Some(config) = self.get_tile_config() {
            if config.show_parent_tiles {
                self.update_parent_tiles(target_zoom)?;
            }
        }

        // Get visible tiles with prefetch buffer
        let visible_tiles = self.get_tiles_with_prefetch(viewport, target_zoom);

        // Mark all tiles as not current first
        for level in self.levels.values_mut() {
            for tile_state in level.tiles.values_mut() {
                tile_state.current = false;
            }
        }

        // Filter valid tiles first to avoid borrow checker issues
        let valid_tiles: Vec<_> = visible_tiles
            .into_iter()
            .filter(|coord| self.is_valid_tile(coord))
            .collect();

        // Update current tiles and request missing ones
        if let Some(level) = self.levels.get_mut(&target_zoom) {
            for coord in valid_tiles {
                let tile_state = level
                    .tiles
                    .entry(coord)
                    .or_insert_with(|| TileState::new(coord));
                tile_state.current = true;

                // If tile is not loaded and not currently loading, request it
                if !tile_state.is_loaded() && !tile_state.loading {
                    tile_state.loading = true;
                    let priority = if coord.z == target_zoom {
                        TilePriority::Visible
                    } else {
                        TilePriority::Adjacent
                    };

                    if let Err(e) =
                        self.tile_loader
                            .queue_tile(self.tile_source.as_ref(), coord, priority)
                    {
                        #[cfg(feature = "debug")]
                        log::warn!("Failed to request tile {:?}: {}", coord, e);
                        tile_state.loading = false;
                        tile_state.mark_error(e.to_string());
                    }
                }
            }
        }

        // Preload tiles for next zoom level if enabled
        if let Some(config) = self.get_tile_config() {
            if config.preload_zoom_tiles && viewport.zoom.fract() > 0.7 {
                self.preload_next_zoom_level(viewport, target_zoom + 1)?;
            }
        }

        // Update tile opacity for smooth fade-in
        self.update_tile_opacity();

        // Clean up old tiles
        self.prune_tiles();

        // Update loading state
        self.loading = !self.all_tiles_loaded();
        self.last_update = std::time::Instant::now();

        Ok(())
    }

    /// Update tile levels (similar to Leaflet's _updateLevels)
    fn update_levels(&mut self, viewport: &Viewport, target_zoom: u8) -> Result<()> {
        // Remove old levels that are no longer needed
        let levels_to_remove: Vec<u8> = self
            .levels
            .keys()
            .filter(|&&z| (z as i16 - target_zoom as i16).abs() > 2)
            .copied()
            .collect();

        for zoom in levels_to_remove {
            self.levels.remove(&zoom);
        }

        // Ensure current level exists
        self.levels.entry(target_zoom).or_insert_with(|| TileLevel::new(target_zoom));

        // Update zoom transforms for smooth animations
        self.update_zoom_transforms(viewport);

        Ok(())
    }

    /// Update zoom transforms (similar to Leaflet's _setZoomTransforms)
    fn update_zoom_transforms(&mut self, viewport: &Viewport) {
        let current_zoom = viewport.zoom;

        for (zoom, level) in &mut self.levels {
            let scale = 2_f64.powf(current_zoom - *zoom as f64);
            level.scale = scale;

            // Calculate translation to keep map centered
            // This enables smooth zoom without re-rendering
            level.translation = Point::new(0.0, 0.0); // Simplified for now
            level.active = (*zoom as f64 - current_zoom).abs() < 2.0;
        }
    }

    /// Process completed tile loading results
    fn process_tile_results(&mut self) -> Result<()> {
        let results = self.tile_loader.try_recv_results();

        for result in results {
            match result.data {
                Ok(data) => {
                    let data_arc = Arc::new(data);
                    // Cache the tile
                    self.tile_cache.put(result.coord, data_arc.clone());

                    // Update tile state
                    if let Some(level) = self.levels.get_mut(&result.coord.z) {
                        if let Some(tile) = level.tiles.get_mut(&result.coord) {
                            tile.mark_loaded(data_arc);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load tile {:?}: {}", result.coord, e);
                    if let Some(level) = self.levels.get_mut(&result.coord.z) {
                        if let Some(tile) = level.tiles.get_mut(&result.coord) {
                            tile.mark_error(e.to_string());
                        }
                    }
                }
            }
        }

        // Check if loading is complete
        if self.loading && self.all_tiles_loaded() {
            self.loading = false;
            log::debug!("Tile loading completed");
        }

        Ok(())
    }

    /// Check if all current tiles are loaded
    fn all_tiles_loaded(&self) -> bool {
        for level in self.levels.values() {
            for tile in level.tiles.values() {
                if tile.current && tile.loading {
                    return false;
                }
            }
        }
        true
    }

    /// Update tile opacity for fade animations (similar to Leaflet's _updateOpacity)
    fn update_tile_opacity(&mut self) {
        let now = std::time::Instant::now();
        let fade_duration = std::time::Duration::from_millis(200);

        for level in self.levels.values_mut() {
            for tile in level.tiles.values_mut() {
                if let Some(loaded_time) = tile.loaded_time {
                    let elapsed = now.duration_since(loaded_time);
                    let fade_progress = (elapsed.as_millis() as f32
                        / fade_duration.as_millis() as f32)
                        .clamp(0.0, 1.0);
                    tile.opacity = fade_progress;
                } else if tile.loading {
                    tile.opacity = 0.0;
                }
            }
        }
    }

    /// Prune tiles that are no longer needed (similar to Leaflet's _pruneTiles)
    fn prune_tiles(&mut self) {
        for level in self.levels.values_mut() {
            // Mark tiles for retention
            for tile in level.tiles.values_mut() {
                tile.retain = tile.current;
            }

            // Retain parent/child tiles for smooth transitions
            let tiles_to_check: Vec<_> = level.tiles.keys().copied().collect();
            for coord in tiles_to_check {
                if let Some(tile) = level.tiles.get(&coord) {
                    if tile.current && tile.is_loaded() {
                        // Could retain parent/child tiles here for smoother transitions
                        // Similar to Leaflet's _retainParent and _retainChildren
                    }
                }
            }

            // Remove tiles that should not be retained
            level.tiles.retain(|_, tile| tile.retain);
        }
    }

    /// Get the tile source
    pub fn tile_source(&self) -> &dyn TileSource {
        self.tile_source.as_ref()
    }

    /// Get the tile options
    pub fn options(&self) -> &TileLayerOptions {
        &self.options
    }

    /// Set new tile options
    pub fn set_tile_options(&mut self, options: TileLayerOptions) {
        self.options = options;

        // Clear cache when options change
        self.levels.clear();
        self.tile_zoom = None;
        self.loading = false;
    }

    /// Returns true if there are any tiles currently being downloaded or processed.
    pub fn is_loading(&self) -> bool {
        self.loading || self.tile_loader.has_pending_results()
    }

    /// Handle retry logic for failed tiles
    fn handle_tile_retries(&mut self) -> Result<()> {
        let config = self.get_tile_config().cloned().unwrap_or_default();

        for level in self.levels.values_mut() {
            let mut tiles_to_retry = Vec::new();

            for (coord, tile_state) in level.tiles.iter() {
                if tile_state.error.is_some()
                    && !tile_state.loading
                    && tile_state.should_retry(
                        config.max_retries,
                        config.retry_delay_ms,
                        config.exponential_backoff,
                    )
                {
                    tiles_to_retry.push(*coord);
                }
            }

            for coord in tiles_to_retry {
                if let Some(tile_state) = level.tiles.get_mut(&coord) {
                    tile_state.loading = true;
                    tile_state.error = None;

                    let priority = TilePriority::Background; // Retries get lower priority

                    if let Err(e) =
                        self.tile_loader
                            .queue_tile(self.tile_source.as_ref(), coord, priority)
                    {
                        #[cfg(feature = "debug")]
                        log::warn!("Failed to retry tile {:?}: {}", coord, e);
                        tile_state.loading = false;
                        tile_state.mark_error(e.to_string());
                    }
                }
            }
        }

        Ok(())
    }

    /// Update parent tile data for smooth zoom animations
    fn update_parent_tiles(&mut self, target_zoom: u8) -> Result<()> {
        if target_zoom == 0 {
            return Ok(());
        }

        let parent_zoom = target_zoom - 1;

        // Get parent tile data
        let mut parent_tiles = Vec::new();
        if let Some(parent_level) = self.levels.get(&parent_zoom) {
            for (coord, tile_state) in &parent_level.tiles {
                if tile_state.is_loaded() {
                    parent_tiles.push((*coord, tile_state.data.clone()));
                }
            }
        }

        // Update current level tiles with parent data
        if let Some(current_level) = self.levels.get_mut(&target_zoom) {
            for tile_state in current_level.tiles.values_mut() {
                if !tile_state.is_loaded() {
                    // Find parent tile
                    let parent_coord = TileCoord {
                        x: tile_state.coord.x / 2,
                        y: tile_state.coord.y / 2,
                        z: parent_zoom,
                    };

                    if let Some((_, parent_data)) = parent_tiles
                        .iter()
                        .find(|(coord, _)| *coord == parent_coord)
                    {
                        tile_state.set_parent_data(parent_data.clone());
                    }
                }
            }
        }

        Ok(())
    }

    /// Get tiles with prefetch buffer around visible area
    fn get_tiles_with_prefetch(&self, viewport: &Viewport, zoom: u8) -> Vec<TileCoord> {
        let base_tiles = self.get_visible_tiles(viewport);
        let default_config = TileLoadingConfig::default();
        let config = self.get_tile_config().unwrap_or(&default_config);

        if config.prefetch_buffer == 0 {
            return base_tiles;
        }

        let mut all_tiles = Vec::new();
        let buffer = config.prefetch_buffer;

        // Get bounds of visible tiles
        let mut min_x = u32::MAX;
        let mut max_x = u32::MIN;
        let mut min_y = u32::MAX;
        let mut max_y = u32::MIN;

        for coord in &base_tiles {
            min_x = min_x.min(coord.x);
            max_x = max_x.max(coord.x);
            min_y = min_y.min(coord.y);
            max_y = max_y.max(coord.y);
        }

        // Add buffer tiles (handle underflow safely)
        let start_x = min_x.saturating_sub(buffer);
        let end_x = max_x.saturating_add(buffer);
        let start_y = min_y.saturating_sub(buffer);
        let end_y = max_y.saturating_add(buffer);

        for x in start_x..=end_x {
            for y in start_y..=end_y {
                let coord = TileCoord { x, y, z: zoom };
                if self.is_valid_tile(&coord) {
                    all_tiles.push(coord);
                }
            }
        }

        all_tiles
    }

    /// Preload tiles for next zoom level
    fn preload_next_zoom_level(&mut self, viewport: &Viewport, next_zoom: u8) -> Result<()> {
        if next_zoom > self.options.max_zoom {
            return Ok(());
        }

        // Create level if it doesn't exist
        self.levels
            .entry(next_zoom)
            .or_insert_with(|| TileLevel::new(next_zoom));

        // Get visible tiles at next zoom level
        let mut next_viewport = viewport.clone();
        next_viewport.set_zoom(next_zoom as f64);
        let next_tiles = self.get_visible_tiles(&next_viewport);

        // Filter valid tiles and limit count first
        let valid_tiles: Vec<_> = next_tiles
            .into_iter()
            .take(6) // Limit preload to avoid overload
            .filter(|coord| self.is_valid_tile(coord))
            .collect();

        // Request tiles that aren't already loading or loaded
        if let Some(level) = self.levels.get_mut(&next_zoom) {
            for coord in valid_tiles {
                let tile_state = level
                    .tiles
                    .entry(coord)
                    .or_insert_with(|| TileState::new(coord));

                if !tile_state.is_loaded() && !tile_state.loading {
                    tile_state.loading = true;

                    if let Err(e) = self.tile_loader.queue_tile(
                        self.tile_source.as_ref(),
                        coord,
                        TilePriority::Prefetch,
                    ) {
                        #[cfg(feature = "debug")]
                        log::debug!("Failed to preload tile {:?}: {}", coord, e);
                        tile_state.loading = false;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get tile loading configuration
    fn get_tile_config(&self) -> Option<&TileLoadingConfig> {
        // The tile loader uses TileLoaderConfig, but we need to return TileLoadingConfig
        // For now, return None as this is likely used for performance configuration
        // This should be connected to the map's performance configuration
        None
    }
}

#[async_trait]
impl LayerTrait for TileLayer {
    fn id(&self) -> &str {
        &self.properties.id
    }

    fn name(&self) -> &str {
        &self.properties.name
    }

    fn layer_type(&self) -> LayerType {
        LayerType::Tile
    }

    fn z_index(&self) -> i32 {
        self.properties.z_index
    }

    fn set_z_index(&mut self, z_index: i32) {
        self.properties.z_index = z_index;
    }

    fn opacity(&self) -> f32 {
        self.properties.opacity
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.properties.opacity = opacity.clamp(0.0, 1.0);
    }

    fn visible(&self) -> bool {
        self.properties.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.properties.visible = visible;
    }

    fn bounds(&self) -> Option<LatLngBounds> {
        self.options.bounds.clone()
    }

    fn render(&self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.visible() {
            return Ok(());
        }

        #[cfg(feature = "debug")]
        log::debug!(
            "rendering tile layer: {} tiles ready",
            self.levels
                .values()
                .map(|level| level.tiles.len())
                .sum::<usize>()
        );

        // Get visible tile coordinates for the *target* zoom level
        let visible_coords = self.get_visible_tiles(viewport);

        // Helper to locate the best (highest-resolution available) tile for a coordinate.
        let find_best_tile = |coord: TileCoord,
                              store: &HashMap<TileCoord, TileState>|
         -> Option<(TileCoord, TileState)> {
            // Try exact match first
            if let Some(tile) = store.get(&coord) {
                return Some((coord, tile.clone()));
            }

            // Walk up the pyramid until we find a parent tile that we already have.
            let mut current = coord;
            while current.z > 0 {
                current = TileCoord {
                    x: current.x / 2,
                    y: current.y / 2,
                    z: current.z - 1,
                };
                if let Some(tile) = store.get(&current) {
                    return Some((current, tile.clone()));
                }
            }
            None
        };

        // Render each visible area, falling back to lower-zoom tiles when needed.
        for coord in visible_coords {
            // Find the best tile across levels
            let mut best_tile = None;
            if let Some(level) = self.levels.get(&coord.z) {
                if let Some((tile_coord, tile_state)) = find_best_tile(coord, &level.tiles) {
                    if tile_state.is_loaded() {
                        best_tile = Some((tile_coord, tile_state));
                    }
                }
            }

            if let Some((tile_coord, tile_state)) = best_tile {
                // Use the existing helper that returns accurate geographic bounds for the tile.
                // This avoids duplicating the inverse Web-Mercator math and eliminates the previous
                // off-by-half-height bug that caused tiles to render as thin horizontal strips.
                // --- Pixel placement through direct world-pixel math (closer to Leaflet) ---
                let world_scale = 256.0 * viewport.scale();

                // Center of the viewport in world-pixel space at the tile zoom.
                let center_world_x = (viewport.center.lng + 180.0) / 360.0 * world_scale;
                let center_lat_rad = viewport.center.lat.to_radians();
                let center_world_y = (1.0
                    - (center_lat_rad.tan() + 1.0 / center_lat_rad.cos()).ln()
                        / std::f64::consts::PI)
                    / 2.0
                    * world_scale;

                // Tile's top-left corner in world-pixel space.
                let tile_px_x = tile_coord.x as f64 * 256.0;
                let tile_px_y = tile_coord.y as f64 * 256.0;

                // Convert to screen-pixel coordinates by offsetting from the viewport center.
                let screen_min = crate::core::geo::Point::new(
                    (tile_px_x - center_world_x) + viewport.size.x / 2.0,
                    (tile_px_y - center_world_y) + viewport.size.y / 2.0,
                );

                let screen_max =
                    crate::core::geo::Point::new(screen_min.x + 256.0, screen_min.y + 256.0);

                #[cfg(feature = "debug")]
                log::debug!(
                    "draw tile {:?} (fallback from {:?}) on screen bounds=({:.1},{:.1})-({:.1},{:.1}) (zoom {})",
                    coord,
                    tile_coord,
                    screen_min.x,
                    screen_min.y,
                    screen_max.x,
                    screen_max.y,
                    viewport.zoom
                );

                if let Some(tile_data) = &tile_state.data {
                    let opacity = self.opacity() * tile_state.opacity;
                    context.render_tile(tile_data.as_slice(), (screen_min, screen_max), opacity)?;
                }
            }
        }

        // Note: Do **not** mutate internal state here; `render` only has an
        // immutable reference. Tile pruning is handled inside `update_tiles`.

        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        // Clean up any error tiles after some time
        if self
            .levels
            .values()
            .flat_map(|level| level.tiles.values())
            .filter(|tile| tile.error.is_some())
            .count()
            > 100
        {
            for level in self.levels.values_mut() {
                level.tiles.retain(|_, tile| tile.error.is_none());
            }
        }

        Ok(())
    }

    fn options(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn set_options(&mut self, options: serde_json::Value) -> Result<()> {
        if let Ok(tile_options) = serde_json::from_value::<TileLayerOptions>(options) {
            self.set_tile_options(tile_options);
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_layer_creation() {
        let layer = TileLayer::new("test".to_string(), "Test Layer".to_string());
        assert_eq!(layer.id(), "test");
        assert_eq!(layer.name(), "Test Layer");
        assert_eq!(layer.layer_type(), LayerType::Tile);
    }

    #[test]
    fn test_openstreetmap_layer() {
        let layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
        assert!(layer.options().url_template.contains("openstreetmap.org"));
        assert!(layer.options().attribution.contains("OpenStreetMap"));
    }

    #[test]
    fn test_satellite_layer() {
        let layer = TileLayer::satellite("sat".to_string(), "Satellite".to_string());
        assert!(layer.options().url_template.contains("arcgisonline.com"));
        assert!(layer.options().attribution.contains("Esri"));
    }
}
