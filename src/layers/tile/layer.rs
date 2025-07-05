//! Core TileLayer implementation

use super::{TileLayerOptions, TileLevel, TileCache, TileLoader, TilePriority, TileLoaderConfig, TileSource, OpenStreetMapSource};
use crate::{
    core::{
        geo::{Point, TileCoord},
        viewport::Viewport,
    },
    layers::{
        base::{LayerProperties, LayerTrait, LayerType},
        animation::AnimationManager,
    },
    rendering::context::RenderContext,
    traits::{PointMath, GeometryOps, RetryLogic},
    Result,
};

pub struct TileLayer {
    pub(crate) properties: LayerProperties,
    pub(crate) options: TileLayerOptions,
    pub(crate) tile_source: Box<dyn TileSource>,
    pub(crate) tile_loader: TileLoader,
    pub(crate) tile_cache: TileCache,
    pub(crate) levels: HashMap<u8, TileLevel>,
    pub(crate) tile_zoom: Option<u8>,
    pub(crate) loading: bool,
    pub(crate) test_mode: bool,
    
    pub(crate) keep_buffer: u32,
    
    pub(crate) animation_manager: Option<crate::layers::animation::AnimationManager>,
    
    pub(crate) render_bounds: Option<crate::core::geo::LatLngBounds>,
    pub(crate) boundary_buffer: f64,
    
    pub(crate) tiles_loading_count: usize,
    pub(crate) loading_state_changed: bool,
}
use crate::prelude::{HashMap, Arc};

#[cfg(feature = "debug")]
use log;

impl TileLayer {
    /// Create a new tile layer with Leaflet-style configuration
    pub fn new(
        id: String,
        tile_source: Box<dyn TileSource>,
        options: TileLayerOptions,
    ) -> Result<Self> {
        let loader_config = TileLoaderConfig {
            max_concurrent: 128, // Much more aggressive concurrency
            max_retries: 2,
            retry_delay: std::time::Duration::from_millis(25), // Faster retries
        };
        Self::new_with_config(id, tile_source, options, loader_config)
    }

    pub fn new_with_config(
        id: String,
        tile_source: Box<dyn TileSource>,
        options: TileLayerOptions,
        loader_config: TileLoaderConfig,
    ) -> Result<Self> {
        let properties = LayerProperties {
            id,
            name: "Tile Layer".to_string(),
            layer_type: LayerType::Tile,
            visible: true,
            opacity: options.opacity,
            z_index: options.z_index,
            interactive: false,
            options: serde_json::Value::Null,
        };

        let tile_loader = TileLoader::new(loader_config);
        let tile_cache = TileCache::new(1024);

        Ok(Self {
            properties,
            tile_source,
            tile_loader,
            tile_cache,
            levels: HashMap::default(),
            tile_zoom: None,
            loading: false,
            test_mode: false,
            keep_buffer: options.keep_buffer,
            animation_manager: Some(AnimationManager::new()),
            render_bounds: options.bounds.clone(),
            boundary_buffer: 0.1, // Default buffer in degrees
            tiles_loading_count: 0,
            loading_state_changed: false,
            options,
        })
    }

    /// Create with animation manager
    pub fn with_animation_manager(mut self, animation_manager: AnimationManager) -> Self {
        self.animation_manager = Some(animation_manager);
        self
    }

    /// Enable test mode for reduced functionality
    pub fn with_test_mode(mut self, test_mode: bool) -> Self {
        self.test_mode = test_mode;
        self
    }

    /// Set custom boundary buffer
    pub fn with_boundary_buffer(mut self, buffer: f64) -> Self {
        self.boundary_buffer = buffer;
        self
    }

    /// Main rendering method that integrates all systems
    /// This consolidates the old duplicated render logic
    pub fn render_tiles(&self, ctx: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        let zoom = viewport.zoom.floor() as u8;
        
        // Skip rendering if zoom is out of bounds
        if zoom < self.options.min_zoom || zoom > self.options.max_zoom {
            return Ok(());
        }

        // Calculate what tiles should be visible
        let tiled_pixel_bounds = self.get_tiled_pixel_bounds(viewport, zoom);
        let tile_range = self.pixel_bounds_to_tile_range(&tiled_pixel_bounds, zoom);
        let visible_tiles = self.tile_range_to_coords(&tile_range, zoom);
        
        let mut tiles_to_queue = Vec::new();

        // Render each visible tile with boundary checking and animation support
        for coord in &visible_tiles {
            // Enhanced boundary checking - skip tiles outside render bounds
            if !self.is_tile_within_boundary(coord) {
                continue;
            }

            // Calculate initial tile screen bounds
            let mut bounds = self.calculate_tile_screen_bounds(*coord, viewport);
            
            // Apply animation transforms if active (Leaflet-style CSS transforms)
            if let Some(level) = self.levels.get(&coord.z) {
                if level.animating {
                    bounds = level.transform_bounds(bounds);
                }
            }

            let mut tile_rendered = false;
            
            // Try to render from level tiles first
            if let Some(level) = self.levels.get(&coord.z) {
                if let Some(tile_state) = level.tiles.get(coord) {
                    if let Some(tile_data) = tile_state.get_display_data() {
                        if ctx.render_tile(tile_data, bounds, self.opacity()).is_ok() {
                            tile_rendered = true;
                        }
                    }
                }
            }

            // Fallback to cache if not in levels
            if !tile_rendered {
                if let Some(tile_data) = self.tile_cache.get(coord) {
                    if ctx.render_tile(&tile_data, bounds, self.opacity()).is_ok() {
                        tile_rendered = true;
                    }
                }
            }

            // Queue for loading if not rendered
            if !tile_rendered {
                tiles_to_queue.push(*coord);
                // Render placeholder (empty tile)
                let _ = ctx.render_tile(&Vec::new(), bounds, self.opacity());
            }
        }

        // Queue tiles that need loading
        if !tiles_to_queue.is_empty() {
            let _ = self.tile_loader.queue_tiles_batch(
                self.tile_source.as_ref(),
                tiles_to_queue,
                TilePriority::Visible,
            );
        }

        Ok(())
    }

    /// Calculate screen bounds for a tile coordinate
    fn calculate_tile_screen_bounds(
        &self,
        coord: TileCoord,
        viewport: &Viewport,
    ) -> (Point, Point) {
        let tile_size = self.options.tile_size as f64;

        // Convert tile coordinate to world pixel coordinates
        let tile_world_x = coord.x as f64 * tile_size;
        let tile_world_y = coord.y as f64 * tile_size;

        // Project viewport center to same zoom level
        let center_world = viewport.project(&viewport.center, Some(coord.z as f64));
        
        // Calculate screen position relative to viewport center
        let screen_x = tile_world_x - center_world.x + viewport.size.x / 2.0;
        let screen_y = tile_world_y - center_world.y + viewport.size.y / 2.0;
 
        (
            Point::new(screen_x, screen_y),
            Point::new(screen_x + tile_size, screen_y + tile_size),
        )
    }

    pub fn for_testing(id: String, name: String) -> Self {
        println!("🧪 [DEBUG] TileLayer::for_testing() - Creating test tile layer '{}' ({})", name, id);
        let mut layer = Self::new_with_config(
            id, 
            Box::new(OpenStreetMapSource::new()), 
            TileLayerOptions::default(),
            TileLoaderConfig::for_testing()
        ).unwrap();
        layer.test_mode = true;
        layer
    }

    pub fn openstreetmap(id: String, name: String) -> Self {
        println!("🌍 [DEBUG] TileLayer::openstreetmap() - Creating OSM tile layer '{}' ({})", name, id);
        let options = TileLayerOptions::default();
        Self::new(id, Box::new(OpenStreetMapSource::new()), options).unwrap_or_else(|e| {
            println!("❌ [DEBUG] TileLayer::openstreetmap() - Failed to create layer: {:?}", e);
            panic!("Failed to create OpenStreetMap tile layer: {:?}", e);
        })
    }

    pub fn satellite(id: String, _name: String) -> Self {
        let options = TileLayerOptions {
            attribution: Some("© Esri, Maxar, GeoEye, Earthstar Geographics, CNES/Airbus DS, USDA, USGS, AeroGRID, IGN, and the GIS User Community".to_string()),
            subdomains: vec![],
            ..Default::default()
        };
        Self::new(id, Box::new(OpenStreetMapSource::new()), options).unwrap()
    }

    /// Create a high-performance tile layer with background task manager
    pub fn new_with_high_performance(
        id: String,
        name: String,
        bg_task_manager: Arc<crate::background::BackgroundTaskManager>,
    ) -> Self {
        println!("🚀 [DEBUG] TileLayer::new_with_high_performance() - Creating high-performance tile layer '{}' ({})", name, id);
        
        let options = TileLayerOptions {
            keep_buffer: 8, // Much more aggressive buffering
            ..Default::default()
        };
        
        // Create tile loader with high-performance preset and background task manager
        let tile_loader = TileLoader::with_high_performance_preset(bg_task_manager);
        
        let properties = LayerProperties {
            id,
            name,
            layer_type: LayerType::Tile,
            visible: true,
            opacity: options.opacity,
            z_index: options.z_index,
            interactive: false,
            options: serde_json::Value::Null,
        };

        let tile_cache = TileCache::new(4096); // Large cache for high performance

        Self {
            properties,
            tile_source: Box::new(OpenStreetMapSource::new()),
            tile_loader,
            tile_cache,
            levels: HashMap::default(),
            tile_zoom: None,
            loading: false,
            test_mode: false,
            keep_buffer: options.keep_buffer,
            animation_manager: Some(AnimationManager::new()),
            render_bounds: options.bounds.clone(),
            boundary_buffer: 0.1,
            tiles_loading_count: 0,
            loading_state_changed: false,
            options,
        }
    }



    // Removed unused tile_bounds method - functionality exists in TileCoord::bounds()



    pub(crate) fn process_tile_results(&mut self) -> Result<()> {
        let results = self.tile_loader.try_recv_results();

        for result in results {
            match result.data {
                Ok(data) => {
                    let data_arc = Arc::new(data);
                    self.tile_cache.put(result.coord, data_arc.clone());

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

        if self.loading && self.all_tiles_loaded() {
            self.loading = false;
            log::debug!("Tile loading completed");
        }

        Ok(())
    }

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

    pub fn tile_source(&self) -> &dyn TileSource {
        self.tile_source.as_ref()
    }

    pub fn options(&self) -> &TileLayerOptions {
        &self.options
    }

    pub fn set_tile_options(&mut self, options: TileLayerOptions) {
        self.options = options;

        self.levels.clear();
        self.tile_zoom = None;
        self.loading = false;
    }

    pub fn is_loading(&self) -> bool {
        self.tiles_loading_count > 0 && (self.loading_state_changed || self.tiles_loading_count < 10)
    }



    pub fn needs_repaint(&self) -> bool {
        // Only repaint when loading state actually changes
        if self.loading_state_changed {
            return true;
        }
        
        // Only repaint when we have very few tiles loading (almost done)
        if self.tiles_loading_count > 0 && self.tiles_loading_count <= 1 {
            return true;
        }
        
        // Check if we have active animations that need repainting
        if let Some(ref animation_manager) = self.animation_manager {
            if animation_manager.is_animating() {
                return true;
            }
        }
        
        false
    }

    pub(crate) fn handle_tile_retries(&mut self) -> Result<()> {
        let config = self.tile_loader.config().clone();

        for level in self.levels.values_mut() {
            let mut tiles_to_retry = Vec::new();

            for (coord, tile_state) in level.tiles.iter() {
                if tile_state.error.is_some()
                    && !tile_state.loading
                    && tile_state.should_retry(
                        config.max_retries as u32,
                        config.retry_delay.as_millis() as u64,
                        false, // exponential_backoff not in config
                    )
                {
                    tiles_to_retry.push(*coord);
                }
            }

            for coord in tiles_to_retry {
                if let Some(tile_state) = level.tiles.get_mut(&coord) {
                    tile_state.loading = true;
                    tile_state.error = None;

                    let priority = TilePriority::Background;

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

    pub fn has_tiles_at_zoom(&self, zoom: u8) -> bool {
        self.levels
            .get(&zoom)
            .is_some_and(|level| !level.tiles.is_empty())
    }

    pub fn tile_loader(&self) -> &TileLoader {
        &self.tile_loader
    }

    pub fn tile_loader_mut(&mut self) -> &mut TileLoader {
        &mut self.tile_loader
    }

    /// Get tiled pixel bounds for a specific zoom level
    /// This matches Leaflet's _getTiledPixelBounds method
    pub fn get_tiled_pixel_bounds(&self, viewport: &Viewport, zoom: u8) -> (Point, Point) {
        let map_zoom = viewport.zoom;
        let scale = 2_f64.powf(map_zoom - zoom as f64);
        let pixel_center = viewport.project(&viewport.center, Some(zoom as f64));
        let half_size = Point::new(viewport.size.x / (scale * 2.0), viewport.size.y / (scale * 2.0));
        
        (
            pixel_center.subtract(&half_size),
            pixel_center.add(&half_size),
        )
    }

    /// Convert pixel bounds to tile coordinate range
    /// This matches Leaflet's _pxBoundsToTileRange method
    pub fn pixel_bounds_to_tile_range(&self, bounds: &(Point, Point), _zoom: u8) -> (Point, Point) {
        let tile_size = self.options.tile_size as f64;
        let min = Point::new(
            (bounds.0.x / tile_size).floor(),
            (bounds.0.y / tile_size).floor(),
        );
        let max = Point::new(
            (bounds.1.x / tile_size).ceil(),
            (bounds.1.y / tile_size).ceil(),
        );
        (min, max)
    }

    /// Convert tile range to coordinate list with boundary checking
    /// Enhanced with improved boundary validation
    pub fn tile_range_to_coords(&self, range: &(Point, Point), zoom: u8) -> Vec<TileCoord> {
        let mut coords = Vec::new();
        let max_coord = (1u32 << zoom) as i32;

        for y in (range.0.y as i32)..=(range.1.y as i32) {
            for x in (range.0.x as i32)..=(range.1.x as i32) {
                // Validate Y coordinate
                if y < 0 || y >= max_coord {
                    continue;
                }
                
                // Wrap X coordinate for spherical mercator
                let wrapped_x = ((x % max_coord) + max_coord) % max_coord;
                
                let coord = TileCoord {
                    x: wrapped_x as u32,
                    y: y as u32,
                    z: zoom,
                };

                // Enhanced boundary checking
                if self.is_valid_tile(&coord) && self.is_tile_within_boundary(&coord) {
                    coords.push(coord);
                }
            }
        }

        coords
    }

    /// Enhanced boundary checking with configurable buffer
    /// This provides the "border" functionality requested by the user
    pub fn is_tile_within_boundary(&self, coord: &TileCoord) -> bool {
        if let Some(bounds) = &self.render_bounds {
            let tile_bounds = self.calculate_tile_bounds_static(coord);
            
            // Create buffered bounds for more flexible boundary checking
            let buffered_bounds = crate::core::geo::LatLngBounds::new(
                crate::core::geo::LatLng::new(
                    bounds.south_west.lat - self.boundary_buffer,
                    bounds.south_west.lng - self.boundary_buffer,
                ),
                crate::core::geo::LatLng::new(
                    bounds.north_east.lat + self.boundary_buffer,
                    bounds.north_east.lng + self.boundary_buffer,
                ),
            );
            
            // Check if tile intersects with buffered bounds
            buffered_bounds.intersects_bounds(&tile_bounds)
        } else {
            true // No boundary restrictions
        }
    }

    /// Check if a tile coordinate is valid for the current configuration
    fn is_valid_tile(&self, coord: &TileCoord) -> bool {
        let max_coord = 1u32 << coord.z;
        coord.x < max_coord && coord.y < max_coord
            && coord.z >= self.options.min_zoom
            && coord.z <= self.options.max_zoom
    }

    /// Static version of calculate_tile_bounds to avoid borrow conflicts
    fn calculate_tile_bounds_static(&self, coord: &TileCoord) -> crate::core::geo::LatLngBounds {
        // Use unified unprojection instead of duplicate Web Mercator calculations
        let tile_size = 256.0;
        
        // Calculate pixel bounds for this tile
        let nw_pixel = crate::core::geo::Point::new(
            coord.x as f64 * tile_size, 
            coord.y as f64 * tile_size
        );
        let se_pixel = crate::core::geo::Point::new(
            (coord.x + 1) as f64 * tile_size, 
            (coord.y + 1) as f64 * tile_size
        );
        
        // Use unified unprojection
        let temp_viewport = crate::core::viewport::Viewport::default();
        let nw_latlng = temp_viewport.unproject(&nw_pixel, Some(coord.z as f64));
        let se_latlng = temp_viewport.unproject(&se_pixel, Some(coord.z as f64));
        
        crate::core::geo::LatLngBounds::new(
            crate::core::geo::LatLng::new(se_latlng.lat, nw_latlng.lng), // south-west
            crate::core::geo::LatLng::new(nw_latlng.lat, se_latlng.lng), // north-east
        )
    }

    /// Main update method that consolidates all tile operations
    /// This is the unified entry point for all tile loading, similar to Leaflet's _update method
    pub fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        // Trigger aggressive prefetching by updating the tile loader with viewport changes
        self.tile_loader.update_viewport(viewport);
        let zoom = viewport.zoom.floor() as u8;
        
        // Check for zoom changes and trigger Leaflet-style animations
        let zoom_changed = if let Some(previous_zoom) = self.tile_zoom {
            zoom != previous_zoom
        } else {
            true // First time setup
        };
        
        if zoom_changed && self.tile_zoom.is_some() {
            // Start zoom animation between old and new zoom levels
            if let Some(ref mut animation_manager) = self.animation_manager {
                let old_zoom = self.tile_zoom.unwrap() as f64;
                let new_zoom = zoom as f64;
                
    // Use smooth zoom animation
                animation_manager.start_smooth_zoom(
                    viewport.center,
                    viewport.center,
                    old_zoom,
                    new_zoom,
                    None, // Focus point (None for center zoom)
                );
            }
            
            // Animate the transition between levels
            self.animate_zoom_transition(
                self.tile_zoom.unwrap() as f64,
                zoom as f64,
                viewport.center,
                viewport,
            );
        }
        
        // Update levels like Leaflet (manage zoom level containers)
        self.update_levels(zoom, self.options.max_zoom);
        
        // Set zoom transforms for all levels during animations
        self.set_zoom_transforms(viewport.center, viewport.zoom, viewport);
        
        // Skip if zoom is out of bounds
        if zoom < self.options.min_zoom || zoom > self.options.max_zoom {
            return Ok(());
        }

        // Calculate visible tile bounds (like Leaflet's _getTiledPixelBounds)
        let tiled_pixel_bounds = self.get_tiled_pixel_bounds(viewport, zoom);
        let tile_range = self.pixel_bounds_to_tile_range(&tiled_pixel_bounds, zoom);
        
        // Expand tile range by keepBuffer (like Leaflet's noPruneRange)
        let buffer_range = self.expand_tile_range(&tile_range, self.keep_buffer as i32);
        
        // Mark tiles for retention first (like Leaflet's tile retention)
        self.mark_tiles_for_retention(&buffer_range, zoom);
        
        // Get all tiles in the buffered range
        let all_coords = self.tile_range_to_coords(&buffer_range, zoom);
        
        // Calculate tile center for distance-based sorting (like Leaflet)
        let tile_center_x = (tile_range.0.x + tile_range.1.x) / 2.0;
        let tile_center_y = (tile_range.0.y + tile_range.1.y) / 2.0;
        let tile_center = Point::new(tile_center_x, tile_center_y);
        
        // Separate tiles into priority groups like Leaflet
        let mut visible_tiles = Vec::new();
        let mut buffer_tiles = Vec::new();
        
        for coord in all_coords {
            // Skip invalid tiles
            if !self.is_valid_tile(&coord) {
                continue;
            }
            
            // Check if tile is in visible range (not just buffer)
            let visible_tile_range = self.tile_range_to_coords(&tile_range, zoom);
            if visible_tile_range.contains(&coord) {
                visible_tiles.push(coord);
            } else {
                buffer_tiles.push(coord);
            }
        }
        
        // Sort visible tiles by distance from center (like Leaflet)
        visible_tiles.sort_by(|a, b| {
            let dist_a = ((a.x as f64 - tile_center.x).powi(2) + (a.y as f64 - tile_center.y).powi(2)).sqrt();
            let dist_b = ((b.x as f64 - tile_center.x).powi(2) + (b.y as f64 - tile_center.y).powi(2)).sqrt();
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Sort buffer tiles by distance from center
        buffer_tiles.sort_by(|a, b| {
            let dist_a = ((a.x as f64 - tile_center.x).powi(2) + (a.y as f64 - tile_center.y).powi(2)).sqrt();
            let dist_b = ((b.x as f64 - tile_center.x).powi(2) + (b.y as f64 - tile_center.y).powi(2)).sqrt();
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Load visible tiles first with high priority
        if !visible_tiles.is_empty() {
            self.load_tiles_batch(&visible_tiles, super::TilePriority::Visible, zoom)?;
        }
        
        // Load buffer tiles with lower priority (Leaflet-style prefetch)
        if !buffer_tiles.is_empty() {
            self.load_tiles_batch(&buffer_tiles, super::TilePriority::Adjacent, zoom)?;
        }
        
        // Load parent tiles for better zoom-out experience (like Leaflet's _retainParent)
        if zoom > 0 {
            let parent_zoom = zoom - 1;
            let parent_coords = self.get_parent_tiles(&tile_range, parent_zoom);
            if !parent_coords.is_empty() {
                self.load_tiles_batch(&parent_coords, super::TilePriority::Background, parent_zoom)?;
            }
        }
        
        // Load child tiles for better zoom-in experience (like Leaflet's _retainChildren)
        if zoom < 18 {
            let child_zoom = zoom + 1;
            let child_coords = self.get_child_tiles(&tile_range, child_zoom);
            if !child_coords.is_empty() && child_coords.len() <= 16 { // Limit child tiles
                self.load_tiles_batch(&child_coords, super::TilePriority::Background, child_zoom)?;
            }
        }
        
        // Prune old tiles that are no longer needed
        self.prune_tiles(zoom);
        
        Ok(())
    }
    
    /// Get parent tiles for the given tile range (like Leaflet's parent retention)
    fn get_parent_tiles(&self, tile_range: &(Point, Point), parent_zoom: u8) -> Vec<TileCoord> {
        let mut parent_tiles = Vec::new();
        
        // Calculate parent tile coordinates (each parent contains 4 children)
        let min_parent_x = (tile_range.0.x as u32) / 2;
        let max_parent_x = (tile_range.1.x as u32) / 2;
        let min_parent_y = (tile_range.0.y as u32) / 2;
        let max_parent_y = (tile_range.1.y as u32) / 2;
        
        for x in min_parent_x..=max_parent_x {
            for y in min_parent_y..=max_parent_y {
                parent_tiles.push(TileCoord { x, y, z: parent_zoom });
            }
        }
        
        parent_tiles
    }
    
    /// Get child tiles for the given tile range (like Leaflet's child retention)
    fn get_child_tiles(&self, tile_range: &(Point, Point), child_zoom: u8) -> Vec<TileCoord> {
        let mut child_tiles = Vec::new();
        
        // Calculate child tile coordinates (each parent has 4 children)
        let min_child_x = (tile_range.0.x as u32) * 2;
        let max_child_x = (tile_range.1.x as u32) * 2 + 1;
        let min_child_y = (tile_range.0.y as u32) * 2;
        let max_child_y = (tile_range.1.y as u32) * 2 + 1;
        
        for x in min_child_x..=max_child_x {
            for y in min_child_y..=max_child_y {
                child_tiles.push(TileCoord { x, y, z: child_zoom });
            }
        }
        
        child_tiles
    }

    /// Expand tile range by buffer amount (Leaflet's keepBuffer)
    fn expand_tile_range(&self, range: &(Point, Point), buffer: i32) -> (Point, Point) {
        (
            Point::new(range.0.x - buffer as f64, range.0.y - buffer as f64),
            Point::new(range.1.x + buffer as f64, range.1.y + buffer as f64),
        )
    }

    /// Mark tiles for retention within buffer area
    /// This prevents tiles from being pruned during panning
    fn mark_tiles_for_retention(&mut self, buffer_range: &(Point, Point), zoom: u8) {
        if let Some(level) = self.levels.get_mut(&zoom) {
            for (coord, tile) in &mut level.tiles {
                let in_buffer = coord.x as f64 >= buffer_range.0.x
                    && coord.x as f64 <= buffer_range.1.x
                    && coord.y as f64 >= buffer_range.0.y
                    && coord.y as f64 <= buffer_range.1.y;
                
                tile.current = in_buffer;
                if in_buffer {
                    tile.retain = true;
                }
            }
        }
    }

    /// Load a batch of tiles with specified priority
    fn load_tiles_batch(&self, coords: &[TileCoord], priority: TilePriority, _zoom: u8) -> Result<()> {
        if coords.is_empty() {
            return Ok(());
        }

        // Queue tiles for loading
        for &coord in coords {
            if let Err(e) = self.tile_loader.queue_tile(self.tile_source.as_ref(), coord, priority) {
                #[cfg(feature = "debug")]
                log::warn!("Failed to queue tile {:?}: {}", coord, e);
            }
        }

        Ok(())
    }

    /// Prune old tiles (Leaflet's _pruneTiles logic)
    /// Enhanced with better retention logic and boundary consideration
    fn prune_tiles(&mut self, current_zoom: u8) {
        // Extract boundary checking info to avoid borrow conflicts
        let render_bounds = self.render_bounds.clone();
        let boundary_buffer = self.boundary_buffer;
        
        // First, mark all tiles for retention (like Leaflet's _pruneTiles)
        for level in self.levels.values_mut() {
            for tile in level.tiles.values_mut() {
                tile.retain = tile.current;
            }
        }
        
        // Collect coordinates that need parent/child retention (like Leaflet's _retainParent and _retainChildren)
        let mut coords_needing_retention = Vec::new();
        for level in self.levels.values() {
            for (coord, tile) in level.tiles.iter() {
                if tile.current && !tile.is_loaded() {
                    coords_needing_retention.push(*coord);
                }
            }
        }
        
        // Apply retention logic without borrowing conflicts
        for coord in coords_needing_retention {
            // Try to retain parent tiles going back 5 zoom levels
            if !self.retain_parent_tiles(coord.x, coord.y, coord.z, coord.z.saturating_sub(5)) {
                // If no parent found, retain children going forward 2 zoom levels
                self.retain_child_tiles(coord.x, coord.y, coord.z, (coord.z + 2).min(18));
            }
        }
        
        // Now prune tiles that are not marked for retention
        for level in self.levels.values_mut() {
            level.tiles.retain(|coord, tile| {
                // Always keep tiles marked for retention
                if tile.retain {
                    return true;
                }
                
                // Keep recently loaded tiles for a short time
                if tile.is_loaded() {
                    if let Some(loaded_time) = tile.loaded_time {
                        if loaded_time.elapsed().as_secs() < 30 {
                            return true;
                        }
                    }
                }
                
                // Check boundary constraints (inline to avoid borrow conflict)
                if let Some(ref bounds) = render_bounds {
                    let tile_bounds = Self::calculate_tile_bounds_static_fn(coord);
                    
                    let buffered_bounds = crate::core::geo::LatLngBounds::new(
                        crate::core::geo::LatLng::new(
                            bounds.south_west.lat - boundary_buffer,
                            bounds.south_west.lng - boundary_buffer,
                        ),
                        crate::core::geo::LatLng::new(
                            bounds.north_east.lat + boundary_buffer,
                            bounds.north_east.lng + boundary_buffer,
                        ),
                    );
                    
                    if !buffered_bounds.intersects_bounds(&tile_bounds) {
                        return false;
                    }
                }
                
                false
            });
        }

        // Remove levels that are too far from current zoom
        let levels_to_remove: Vec<_> = self
            .levels
            .keys()
            .filter(|&&zoom| (zoom as i16 - current_zoom as i16).abs() > 2)
            .cloned()
            .collect();
        
        for zoom in levels_to_remove {
            #[cfg(feature = "debug")]
            log::debug!("Pruning level {} (too far from current zoom {})", zoom, current_zoom);
            self.levels.remove(&zoom);
        }
    }

    /// Static version of calculate_tile_bounds to avoid borrow conflicts
    fn calculate_tile_bounds_static_fn(coord: &TileCoord) -> crate::core::geo::LatLngBounds {
        // Use unified unprojection instead of duplicate Web Mercator calculations
        let tile_size = 256.0;
        
        // Calculate pixel bounds for this tile
        let nw_pixel = crate::core::geo::Point::new(
            coord.x as f64 * tile_size, 
            coord.y as f64 * tile_size
        );
        let se_pixel = crate::core::geo::Point::new(
            (coord.x + 1) as f64 * tile_size, 
            (coord.y + 1) as f64 * tile_size
        );
        
        // Use unified unprojection
        let temp_viewport = crate::core::viewport::Viewport::default();
        let nw_latlng = temp_viewport.unproject(&nw_pixel, Some(coord.z as f64));
        let se_latlng = temp_viewport.unproject(&se_pixel, Some(coord.z as f64));
        
        crate::core::geo::LatLngBounds::new(
            crate::core::geo::LatLng::new(se_latlng.lat, nw_latlng.lng), // south-west
            crate::core::geo::LatLng::new(nw_latlng.lat, se_latlng.lng), // north-east
        )
    }

    /// Retain parent tiles for smooth zoom-out (like Leaflet's _retainParent)
    fn retain_parent_tiles(&mut self, x: u32, y: u32, z: u8, min_zoom: u8) -> bool {
        if z <= min_zoom {
            return false;
        }

        let parent_x = x / 2;
        let parent_y = y / 2;
        let parent_z = z - 1;
        let parent_coord = TileCoord { x: parent_x, y: parent_y, z: parent_z };

        if let Some(parent_level) = self.levels.get_mut(&parent_z) {
            if let Some(parent_tile) = parent_level.tiles.get_mut(&parent_coord) {
                if parent_tile.is_loaded() {
                    parent_tile.retain = true;
                    return true;
                } else if parent_tile.loading {
                    parent_tile.retain = true;
                }
            }
        }

        // Recursively check higher zoom levels
        if parent_z > min_zoom {
            return self.retain_parent_tiles(parent_x, parent_y, parent_z, min_zoom);
        }

        false
    }

    /// Retain child tiles for smooth zoom-in (like Leaflet's _retainChildren)
    fn retain_child_tiles(&mut self, x: u32, y: u32, z: u8, max_zoom: u8) {
        if z >= max_zoom {
            return;
        }

        let child_z = z + 1;
        
        // Each parent tile has 4 children
        for child_x in (x * 2)..(x * 2 + 2) {
            for child_y in (y * 2)..(y * 2 + 2) {
                let child_coord = TileCoord { x: child_x, y: child_y, z: child_z };
                
                if let Some(child_level) = self.levels.get_mut(&child_z) {
                    if let Some(child_tile) = child_level.tiles.get_mut(&child_coord) {
                        if child_tile.is_loaded() {
                            child_tile.retain = true;
                            continue;
                        } else if child_tile.loading {
                            child_tile.retain = true;
                        }
                    }
                }

                // Recursively check higher zoom levels
                if child_z < max_zoom {
                    self.retain_child_tiles(child_x, child_y, child_z, max_zoom);
                }
            }
        }
    }
    
    /// Leaflet-style level management methods
    
    /// Update levels like Leaflet's _updateLevels method
    /// This manages the CSS-style zoom level containers
    pub fn update_levels(&mut self, current_zoom: u8, max_zoom: u8) {
        // Remove levels that are too far from current zoom or have no tiles
        let levels_to_remove: Vec<_> = self.levels.keys()
            .filter(|&&zoom| {
                let level = self.levels.get(&zoom).unwrap();
                let zoom_diff = (zoom as i16 - current_zoom as i16).abs();
                
                // Remove if too far from current zoom OR has no tiles and is not current zoom
                zoom_diff > 2 || (level.tiles.is_empty() && zoom != current_zoom)
            })
            .cloned()
            .collect();
        
        for zoom in levels_to_remove {
            #[cfg(feature = "debug")]
            log::debug!("Removing level {} (too far from current zoom {} or empty)", zoom, current_zoom);
            self.levels.remove(&zoom);
        }
        
        // Update z-index for existing levels (like Leaflet)
        for (zoom, level) in self.levels.iter_mut() {
            level.set_z_index(max_zoom as i32 - (*zoom as i32 - current_zoom as i32).abs());
        }
        
        // Ensure current level exists
        if !self.levels.contains_key(&current_zoom) {
            let mut level = TileLevel::new(current_zoom);
            level.set_active(true);
            level.set_z_index(max_zoom as i32);
            self.levels.insert(current_zoom, level);
            
            #[cfg(feature = "debug")]
            log::debug!("Created new level {} with z-index {}", current_zoom, max_zoom);
        }
    }
    
    /// Set zoom transforms for all levels (like Leaflet's _setZoomTransforms)
    pub fn set_zoom_transforms(&mut self, center: crate::core::geo::LatLng, zoom: f64, viewport: &crate::core::viewport::Viewport) {
        for level in self.levels.values_mut() {
            level.set_zoom_transform(center, zoom, center, zoom, viewport);
        }
    }
    
    /// Animate zoom transition between levels (like Leaflet's _animateZoom)
    pub fn animate_zoom_transition(&mut self, from_zoom: f64, to_zoom: f64, center: crate::core::geo::LatLng, viewport: &crate::core::viewport::Viewport) {
        let from_level = from_zoom.floor() as u8;
        let to_level = to_zoom.floor() as u8;
        
        // Ensure both levels exist
        if !self.levels.contains_key(&from_level) {
            self.levels.insert(from_level, TileLevel::new(from_level));
        }
        if !self.levels.contains_key(&to_level) {
            self.levels.insert(to_level, TileLevel::new(to_level));
        }
        
        // Retain parent and child tiles for smooth transitions (like Leaflet)
        self.retain_tiles_for_zoom_transition(from_level, to_level);
        
        // Set transforms for smooth animation
        if let Some(level) = self.levels.get_mut(&from_level) {
            level.set_zoom_transform(center, from_zoom, center, to_zoom, viewport);
            level.set_retain(true);
            level.set_opacity(1.0);
        }
        
        if let Some(level) = self.levels.get_mut(&to_level) {
            level.set_zoom_transform(center, to_zoom, center, to_zoom, viewport);
            level.set_active(true);
            level.set_opacity(1.0);
        }
    }
    
    /// Retain parent and child tiles during zoom transitions (like Leaflet's parent/child retention)
    fn retain_tiles_for_zoom_transition(&mut self, from_zoom: u8, to_zoom: u8) {
        if from_zoom == to_zoom {
            return;
        }
        
        // Collect coordinates that need parent/child retention
        let mut tiles_needing_parents = Vec::new();
        let mut tiles_needing_children = Vec::new();
        
        // If zooming out, we need parent tiles from the higher zoom level
        if to_zoom < from_zoom {
            if let Some(from_level) = self.levels.get(&from_zoom) {
                for coord in from_level.tiles.keys() {
                    tiles_needing_parents.push(*coord);
                }
            }
        }
        
        // If zooming in, we need child tiles from the lower zoom level  
        if to_zoom > from_zoom {
            if let Some(from_level) = self.levels.get(&from_zoom) {
                for coord in from_level.tiles.keys() {
                    tiles_needing_children.push(*coord);
                }
            }
        }
        
        // Retain parent tiles for smooth zoom-out
        for coord in tiles_needing_parents {
            self.retain_parent_tiles(coord.x, coord.y, coord.z, to_zoom);
        }
        
        // Retain child tiles for smooth zoom-in
        for coord in tiles_needing_children {
            self.retain_child_tiles(coord.x, coord.y, coord.z, to_zoom);
        }
    }
}
