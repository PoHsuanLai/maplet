use crate::{
    core::{
        geo::{LatLng, LatLngBounds, TileCoord},
        viewport::Viewport,
    },
    layers::base::{LayerProperties, LayerTrait, LayerType},
    rendering::context::RenderContext,
    tiles::{loader::TileLoader, source::TileSource},
    Result,
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{mpsc::Receiver, mpsc::Sender, Arc, Mutex},
};

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
        }
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
    tile_loader: Arc<TileLoader>,
    /// Receiver for completed tile downloads
    tile_rx: Mutex<Receiver<(TileCoord, Vec<u8>)>>,
    /// Currently visible tiles
    visible_tiles: HashMap<TileCoord, Arc<Vec<u8>>>,
    /// Loading tiles (to avoid duplicate requests)
    loading_tiles: HashMap<TileCoord, bool>,
    /// Error tiles (failed to load)
    error_tiles: HashMap<TileCoord, String>,
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

        // Channel for tile completion messages
        let (tx, rx): (Sender<(TileCoord, Vec<u8>)>, Receiver<(TileCoord, Vec<u8>)>) = std::sync::mpsc::channel();

        Self {
            properties,
            tile_source,
            tile_loader: Arc::new(TileLoader::new(tx)),
            tile_rx: Mutex::new(rx),
            options,
            visible_tiles: HashMap::new(),
            loading_tiles: HashMap::new(),
            error_tiles: HashMap::new(),
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
        // ------------------------------------------------------------------
        // 1. Determine zoom level within layer limits
        // ------------------------------------------------------------------
        let mut zoom = viewport.zoom.floor() as u8;
        zoom = zoom.clamp(self.options.min_zoom, self.options.max_zoom);

        let tiles_per_axis = 1u32 << zoom;

        // ------------------------------------------------------------------
        // 2. Convert viewport bounds to tile-space indices
        // ------------------------------------------------------------------
        let bounds = viewport.bounds();

        // Helper closure to convert lat/lng → fractional tile indices
        let ll_to_tile = |lat: f64, lng: f64| -> (f64, f64) {
            let lat_clamped = LatLng::clamp_lat(lat);
            let lat_rad = lat_clamped.to_radians();
            let x = (lng + 180.0) / 360.0 * tiles_per_axis as f64;
            let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)
                / 2.0
                * tiles_per_axis as f64;
            (x, y)
        };

        let north_west_lat = bounds.north_east.lat;
        let north_west_lng = bounds.south_west.lng;
        let south_east_lat = bounds.south_west.lat;
        let south_east_lng = bounds.north_east.lng;

        let (min_x_f, max_y_f) = ll_to_tile(north_west_lat, north_west_lng);
        let (max_x_f, min_y_f) = ll_to_tile(south_east_lat, south_east_lng);

        let margin: i32 = 1; // one-tile buffer around view

        let min_x = (min_x_f.floor() as i32 - margin).max(0) as u32;
        let max_x = (max_x_f.ceil() as i32 + margin).min(tiles_per_axis as i32 - 1) as u32;
        let min_y = (min_y_f.floor() as i32 - margin).max(0) as u32;
        let max_y = (max_y_f.ceil() as i32 + margin).min(tiles_per_axis as i32 - 1) as u32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                tiles.push(TileCoord { x, y, z: zoom });
            }
        }

        tiles
    }

    /// Load a tile asynchronously
    fn load_tile(&mut self, coord: TileCoord) -> Result<()> {
        if self.loading_tiles.contains_key(&coord) || self.visible_tiles.contains_key(&coord) {
            return Ok(());
        }

        self.loading_tiles.insert(coord, true);

        let tile_source = &*self.tile_source;
        self.tile_loader.start_download(tile_source, coord);

        Ok(())
    }

    /// Update visible tiles based on current viewport
    pub async fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        let visible_coords = self.get_visible_tiles(viewport);

        // Drain any completed downloads
        {
            use std::sync::mpsc::TryRecvError;
            let rx_lock = self.tile_rx.lock().unwrap();
            loop {
                match rx_lock.try_recv() {
                    Ok((coord, data)) => {
                        log::info!("Tile {:?} downloaded ({} bytes)", coord, data.len());
                        log::info!("tile ready {:?} ({} bytes)", coord, data.len());
                        self.visible_tiles.insert(coord, Arc::new(data));
                        self.loading_tiles.remove(&coord);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }

        // Load at most N tiles per frame to avoid long blocking stalls
        const MAX_LOAD_PER_CALL: usize = 4;
        let mut loaded_this_call = 0;

        // Load any missing tiles (bounded)
        for coord in &visible_coords {
            if loaded_this_call >= MAX_LOAD_PER_CALL {
                break; // defer remaining tiles to next frame
            }

            if !self.visible_tiles.contains_key(coord) && !self.error_tiles.contains_key(coord) {
                log::debug!("Loading tile {:?}", coord);
                self.load_tile(*coord)?;
                loaded_this_call += 1;
            }
        }

        // Defer pruning of obsolete tiles: keep them around while they can
        // still be useful for smooth zooming. Only purge when they are two
        // or more zoom levels away from the current integer zoom.
        {
            let current_z = viewport.zoom.floor() as i32;
            let mut to_remove = Vec::new();
            for coord in self.visible_tiles.keys() {
                let dz = (coord.z as i32 - current_z).abs();
                if dz >= 2 {
                    to_remove.push(*coord);
                }
            }

            for coord in to_remove {
                self.visible_tiles.remove(&coord);
                self.loading_tiles.remove(&coord);
            }
        }

        Ok(())
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
        self.visible_tiles.clear();
        self.loading_tiles.clear();
        self.error_tiles.clear();
    }

    /// Returns true if there are any tiles currently being downloaded.
    pub fn is_loading(&self) -> bool {
        !self.loading_tiles.is_empty()
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

    async fn render(&self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.visible() {
            return Ok(());
        }

        log::debug!("rendering tile layer: {} tiles ready", self.visible_tiles.len());

        // Get visible tile coordinates for the *target* zoom level
        let visible_coords = self.get_visible_tiles(viewport);

        // Helper to locate the best (highest-resolution available) tile for a coordinate.
        let find_best_tile = |coord: TileCoord, store: &HashMap<TileCoord, Arc<Vec<u8>>>| -> Option<(TileCoord, Arc<Vec<u8>>)> {
            // Try exact match first
            if let Some(data) = store.get(&coord) {
                return Some((coord, data.clone()));
            }

            // Walk up the pyramid until we find a parent tile that we already have.
            let mut current = coord;
            while current.z > 0 {
                current = TileCoord { x: current.x / 2, y: current.y / 2, z: current.z - 1 };
                if let Some(data) = store.get(&current) {
                    return Some((current, data.clone()));
                }
            }
            None
        };

        // Render each visible area, falling back to lower-zoom tiles when needed.
        for coord in visible_coords {
            if let Some((tile_coord, tile_data)) = find_best_tile(coord, &self.visible_tiles) {
                // Use the existing helper that returns accurate geographic bounds for the tile.
                // This avoids duplicating the inverse Web-Mercator math and eliminates the previous
                // off-by-half-height bug that caused tiles to render as thin horizontal strips.
                let tile_bounds = tile_coord.bounds();

                let mut screen_min = viewport.lat_lng_to_pixel(&tile_bounds.south_west);
                let mut screen_max = viewport.lat_lng_to_pixel(&tile_bounds.north_east);

                if screen_min.x > screen_max.x {
                    std::mem::swap(&mut screen_min.x, &mut screen_max.x);
                }
                if screen_min.y > screen_max.y {
                    std::mem::swap(&mut screen_min.y, &mut screen_max.y);
                }

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

                context.render_tile(tile_data.as_slice(), (screen_min, screen_max), self.opacity())?;
            }
        }

        // Note: Do **not** mutate internal state here; `render` only has an
        // immutable reference. Tile pruning is handled inside `update_tiles`.

        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        // Clean up any error tiles after some time
        if self.error_tiles.len() > 100 {
            self.error_tiles.clear();
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
