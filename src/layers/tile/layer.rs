//! Core TileLayer implementation

use super::{TileLayerOptions, TileLevel, TileCache, TileLoader, TilePriority, TileLoaderConfig, TileSource, OpenStreetMapSource};
use crate::{
    core::{
        geo::{LatLng, LatLngBounds, Point, TileCoord},
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
    pub(crate) last_update_time: std::time::Instant,
    pub(crate) update_interval_ms: u64,
    pub(crate) pending_update: bool,
    
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
            last_update_time: std::time::Instant::now(),
            update_interval_ms: options.update_interval_ms,
            pending_update: false,
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

        let bounds = (
            Point::new(screen_x, screen_y),
            Point::new(screen_x + tile_size, screen_y + tile_size),
        );

        bounds
    }

    pub fn for_testing(id: String, name: String) -> Self {
        println!("🧪 [DEBUG] TileLayer::for_testing() - Creating test tile layer '{}' ({})", name, id);
        let mut layer = Self::new(id, Box::new(OpenStreetMapSource::new()), TileLayerOptions::default()).unwrap();
        layer.test_mode = true;
        layer.update_interval_ms = 0;
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



    fn tile_bounds(&self, coord: &TileCoord) -> LatLngBounds {
        let tile_size = self.options.tile_size as f64;
        let scale = 256.0 * 2_f64.powf(coord.z as f64);

        let nw_x = coord.x as f64 * tile_size;
        let nw_y = coord.y as f64 * tile_size;
        let se_x = nw_x + tile_size;
        let se_y = nw_y + tile_size;

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

    pub fn mark_needs_update(&mut self) {
        self.pending_update = true;
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
}
