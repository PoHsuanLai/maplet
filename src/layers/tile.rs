use crate::{
    core::{
        geo::{LatLng, LatLngBounds, Point, TileCoord},
        viewport::Viewport,
    },
    layers::base::{LayerProperties, LayerTrait, LayerType},

    tiles::{
        loader::{TileLoader, TilePriority},
        source::TileSource,
        cache::TileCache,
    },
    Result,
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::Arc,
};

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
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn mark_loaded(&mut self, data: Arc<Vec<u8>>) {
        self.data = Some(data);
        self.loading = false;
        self.loaded_time = Some(std::time::Instant::now());
    }

    pub fn mark_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
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
        let mut options = TileLayerOptions::default();
        options.url_template = "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png".to_string();
        options.attribution = "© OpenStreetMap contributors".to_string();
        Self::with_options(id, name, options)
    }

    /// Create a tile layer for satellite imagery
    pub fn satellite(id: String, name: String) -> Self {
        let mut options = TileLayerOptions::default();
        options.url_template = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}".to_string();
        options.subdomains = vec![]; // ArcGIS doesn't use subdomains
        options.attribution = "© Esri, Maxar, GeoEye, Earthstar Geographics, CNES/Airbus DS, USDA, USGS, AeroGRID, IGN, and the GIS User Community".to_string();
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
        let half_size = Point::new(viewport.size.x / (scale * 2.0), viewport.size.y / (scale * 2.0));
        
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
        let nw_lat_rad = std::f64::consts::FRAC_PI_2 - 2.0 * ((0.5 - nw_y / scale) * 2.0 * std::f64::consts::PI).exp().atan();
        let nw_lat = nw_lat_rad * d;
        
        let se_lng = se_x / scale * 360.0 - 180.0;
        let se_lat_rad = std::f64::consts::FRAC_PI_2 - 2.0 * ((0.5 - se_y / scale) * 2.0 * std::f64::consts::PI).exp().atan();
        let se_lat = se_lat_rad * d;
        
        LatLngBounds::new(
            LatLng::new(se_lat, nw_lng), // south-west
            LatLng::new(nw_lat, se_lng), // north-east
        )
    }

    /// Update tiles for the current viewport (main tile management method)
    pub fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        let now = std::time::Instant::now();
        
        // Only update frequently if there are new results or the viewport changed significantly
        let should_update = now.duration_since(self.last_update) > std::time::Duration::from_millis(16) // ~60fps
            || self.tile_loader.has_pending_results();
            
        if !should_update {
            return Ok(());
        }
        
        self.last_update = now;

        // Process any completed tile downloads first
        self.process_tile_results()?;

        // Determine target zoom based on viewport
        let target_zoom = viewport.zoom.round() as u8;
        let target_zoom = target_zoom.max(self.options.min_zoom).min(self.options.max_zoom);

        // Update tile opacity for fade animations
        self.update_tile_opacity();

        // Get currently visible tiles
        let visible_coords = self.get_visible_tiles(viewport);
        
        // Only queue new tiles if we don't have too many pending
        let pending_count = self.levels.values()
            .flat_map(|level| level.tiles.values())
            .filter(|tile| tile.loading)
            .count();
            
        if pending_count < 20 {  // Limit pending downloads to prevent overload
            // Update levels and queue missing tiles
            self.update_levels(viewport, target_zoom)?;

            // Queue visible tiles for loading if not already loaded or loading
            let mut tiles_to_load = Vec::new();
            for coord in visible_coords {
                if let Some(level) = self.levels.get(&coord.z) {
                    if let Some(tile) = level.tiles.get(&coord) {
                        if !tile.is_loaded() && !tile.loading {
                            tiles_to_load.push(coord);
                        }
                    } else {
                        tiles_to_load.push(coord);
                    }
                }
            }

            if !tiles_to_load.is_empty() {
                // Mark tiles as loading before queuing
                for coord in &tiles_to_load {
                    if let Some(level) = self.levels.get_mut(&coord.z) {
                        level.tiles.entry(*coord)
                            .and_modify(|tile| tile.loading = true)
                            .or_insert_with(|| {
                                let mut tile = TileState::new(*coord);
                                tile.loading = true;
                                tile.current = true;
                                tile
                            });
                    }
                }

                #[cfg(feature = "debug")]
                log::debug!("Queueing {} tiles for loading", tiles_to_load.len());

                // Queue tiles for download
                if let Err(e) = self.tile_loader.queue_tiles_batch(
                    self.tile_source.as_ref(),
                    tiles_to_load,
                    crate::tiles::loader::TilePriority::Visible,
                ) {
                    log::warn!("Failed to queue tiles: {}", e);
                }
                
                self.loading = true;
            }
        }

        // Prune old tiles periodically
        if now.duration_since(self.last_update) > std::time::Duration::from_secs(5) {
            self.prune_tiles();
        }

        // Update zoom transforms for smooth zooming
        self.update_zoom_transforms(viewport);

        Ok(())
    }

    /// Update tile levels (similar to Leaflet's _updateLevels)
    fn update_levels(&mut self, viewport: &Viewport, target_zoom: u8) -> Result<()> {
        // Remove old levels that are no longer needed
        let levels_to_remove: Vec<u8> = self.levels.keys()
            .filter(|&&z| (z as i16 - target_zoom as i16).abs() > 2)
            .copied()
            .collect();
            
        for zoom in levels_to_remove {
            self.levels.remove(&zoom);
        }

        // Ensure current level exists
        if !self.levels.contains_key(&target_zoom) {
            self.levels.insert(target_zoom, TileLevel::new(target_zoom));
        }

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
                    let fade_progress = (elapsed.as_millis() as f32 / fade_duration.as_millis() as f32).clamp(0.0, 1.0);
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
    pub fn tile_source(&self) -> &Box<dyn TileSource> {
        &self.tile_source
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
        log::debug!("rendering tile layer: {} tiles ready", self.levels.values().map(|level| level.tiles.len()).sum::<usize>());

        // Get visible tile coordinates for the *target* zoom level
        let visible_coords = self.get_visible_tiles(viewport);

        // Helper to locate the best (highest-resolution available) tile for a coordinate.
        let find_best_tile = |coord: TileCoord, store: &HashMap<TileCoord, TileState>| -> Option<(TileCoord, TileState)> {
            // Try exact match first
            if let Some(tile) = store.get(&coord) {
                return Some((coord, tile.clone()));
            }

            // Walk up the pyramid until we find a parent tile that we already have.
            let mut current = coord;
            while current.z > 0 {
                current = TileCoord { x: current.x / 2, y: current.y / 2, z: current.z - 1 };
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
                let center_world_y = (1.0 - (center_lat_rad.tan() + 1.0 / center_lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * world_scale;

                // Tile's top-left corner in world-pixel space.
                let tile_px_x = tile_coord.x as f64 * 256.0;
                let tile_px_y = tile_coord.y as f64 * 256.0;

                // Convert to screen-pixel coordinates by offsetting from the viewport center.
                let screen_min = crate::core::geo::Point::new(
                    (tile_px_x - center_world_x) + viewport.size.x / 2.0,
                    (tile_px_y - center_world_y) + viewport.size.y / 2.0,
                );

                let screen_max = crate::core::geo::Point::new(screen_min.x + 256.0, screen_min.y + 256.0);
                
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
        if self.levels.values().flat_map(|level| level.tiles.values()).filter(|tile| tile.error.is_some()).count() > 100 {
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
