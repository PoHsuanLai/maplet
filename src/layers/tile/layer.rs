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
    pub(crate) last_loading_count: usize,
    pub(crate) loading_state_changed: bool,
}
use std::{collections::HashMap, sync::Arc};

#[cfg(feature = "debug")]
use log;

impl TileLayer {
    /// Create a new tile layer with Leaflet-style configuration
    pub fn new(
        id: String,
        tile_source: Box<dyn TileSource>,
        options: TileLayerOptions,
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

        let loader_config = TileLoaderConfig {
            max_concurrent: 6,
            max_retries: 3,
            retry_delay: std::time::Duration::from_millis(500),
        };

        let tile_loader = TileLoader::new(loader_config);
        let tile_cache = TileCache::new(1024);

        Ok(Self {
            properties,
            tile_source,
            tile_loader,
            tile_cache,
            levels: HashMap::new(),
            tile_zoom: None,
            loading: false,
            test_mode: false,
            keep_buffer: options.keep_buffer,
            animation_manager: Some(AnimationManager::new()),
            render_bounds: options.bounds.clone(),
            boundary_buffer: 0.1, // Default buffer in degrees
            tiles_loading_count: 0,
            last_loading_count: 0,
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
        let mut layer = Self::new(id, Box::new(OpenStreetMapSource::new()), TileLayerOptions::default()).unwrap();
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
        self.tiles_loading_count > 0 && (self.loading_state_changed || self.tiles_loading_count < 5)
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
            buffered_bounds.intersects(&tile_bounds)
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
        let n = 2.0_f64.powi(coord.z as i32);
        
        // Northwest corner
        let nw_lng = coord.x as f64 / n * 360.0 - 180.0;
        let nw_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n);
        let nw_lat = nw_lat_rad.sinh().atan().to_degrees();
        
        // Southeast corner  
        let se_lng = (coord.x + 1) as f64 / n * 360.0 - 180.0;
        let se_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * (coord.y + 1) as f64 / n);
        let se_lat = se_lat_rad.sinh().atan().to_degrees();
        
        crate::core::geo::LatLngBounds::new(
            crate::core::geo::LatLng::new(se_lat, nw_lng), // south-west
            crate::core::geo::LatLng::new(nw_lat, se_lng), // north-east
        )
    }

    /// Main update method that consolidates all tile operations
    /// This is the unified entry point for all tile loading, similar to Leaflet's _update method
    pub fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        let target_zoom = viewport.zoom.floor() as u8;
        let clamped_zoom = target_zoom.clamp(self.options.min_zoom, self.options.max_zoom);

        // Test mode - minimal tile setup
        if self.test_mode {
            self.levels
                .entry(clamped_zoom)
                .or_insert_with(|| TileLevel::new(clamped_zoom));
            return Ok(());
        }

        // Process any completed tile results first
        self.process_tile_results()?;

        // Handle zoom animation updates (CSS-style transforms)
        if let Some(ref mut animation_manager) = self.animation_manager {
            if let Some(_animation_state) = animation_manager.update() {
                // Animation transforms are handled by the central orchestrator now
            }
        }

        // Calculate current view requirements
        let tiled_pixel_bounds = self.get_tiled_pixel_bounds(viewport, clamped_zoom);
        let tile_range = self.pixel_bounds_to_tile_range(&tiled_pixel_bounds, clamped_zoom);
        let visible_tiles = self.tile_range_to_coords(&tile_range, clamped_zoom);

        // Expanded range for prefetching (Leaflet's keepBuffer)
        let buffered_range = self.expand_tile_range(&tile_range, self.keep_buffer as i32);
        let prefetch_tiles = self.tile_range_to_coords(&buffered_range, clamped_zoom);

        // Ensure level exists for current zoom
        self.levels
            .entry(clamped_zoom)
            .or_insert_with(|| TileLevel::new(clamped_zoom));

        // Mark tiles for retention within buffer area (Leaflet's noPruneRange)
        self.mark_tiles_for_retention(&buffered_range, clamped_zoom);

        // Load visible tiles with highest priority
        self.load_tiles_batch(&visible_tiles, TilePriority::Visible, clamped_zoom)?;

        // Load prefetch tiles (excluding already visible ones)
        let prefetch_only: Vec<_> = prefetch_tiles
            .into_iter()
            .filter(|coord| !visible_tiles.contains(coord))
            .collect();
        self.load_tiles_batch(&prefetch_only, TilePriority::Prefetch, clamped_zoom)?;

        // Clean up old tiles
        self.prune_tiles(clamped_zoom);

        // Update loading state
        self.tile_zoom = Some(clamped_zoom);
        let current_loading_count = self.count_loading_tiles();
        self.loading_state_changed = current_loading_count != self.last_loading_count;
        self.last_loading_count = current_loading_count;
        self.tiles_loading_count = current_loading_count;
        self.loading = current_loading_count > 0;

        Ok(())
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
        
        // Prune tiles within each level
        for level in self.levels.values_mut() {
            level.tiles.retain(|coord, tile| {
                // Always keep current tiles
                if tile.current {
                    return true;
                }
                
                // Keep tiles marked for retention
                if tile.retain {
                    tile.retain = false; // Reset for next frame
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
                    
                    if !buffered_bounds.intersects(&tile_bounds) {
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

    /// Count tiles currently loading across all levels
    fn count_loading_tiles(&self) -> usize {
        self.levels
            .values()
            .map(|level| {
                level
                    .tiles
                    .values()
                    .filter(|tile| tile.loading)
                    .count()
            })
            .sum()
    }

    /// Static version of calculate_tile_bounds to avoid borrow conflicts
    fn calculate_tile_bounds_static_fn(coord: &TileCoord) -> crate::core::geo::LatLngBounds {
        let n = 2.0_f64.powi(coord.z as i32);
        
        // Northwest corner
        let nw_lng = coord.x as f64 / n * 360.0 - 180.0;
        let nw_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n);
        let nw_lat = nw_lat_rad.sinh().atan().to_degrees();
        
        // Southeast corner  
        let se_lng = (coord.x + 1) as f64 / n * 360.0 - 180.0;
        let se_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * (coord.y + 1) as f64 / n);
        let se_lat = se_lat_rad.sinh().atan().to_degrees();
        
        crate::core::geo::LatLngBounds::new(
            crate::core::geo::LatLng::new(se_lat, nw_lng), // south-west
            crate::core::geo::LatLng::new(nw_lat, se_lng), // north-east
        )
    }
}
