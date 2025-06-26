use maplet::{
    core::{geo::LatLng, geo::Point},
    layers::tile::TileLayer,
    Map,
};
use wasm_bindgen::prelude::*;

// Import console.log for debugging
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

/// Main WASM entry point
#[wasm_bindgen(start)]
pub fn main() {
    // Set up panic hook for better error messages
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();
    
    console_log!("Maplet WASM demo initialized!");
}

/// Create a new map instance for WASM
#[wasm_bindgen]
pub fn create_map(lat: f64, lng: f64, zoom: f64, width: f64, height: f64) -> WasmMap {
    let center = LatLng::new(lat, lng);
    let size = Point::new(width, height);
    let map = Map::new(center, zoom, size);
    
    WasmMap { inner: map }
}

/// WASM-compatible map wrapper
#[wasm_bindgen]
pub struct WasmMap {
    inner: Map,
}

#[wasm_bindgen]
impl WasmMap {
    /// Add an OpenStreetMap tile layer
    #[wasm_bindgen]
    pub fn add_osm_layer(&mut self, id: &str, name: &str) -> Result<(), JsValue> {
        let layer = TileLayer::openstreetmap(id.to_string(), name.to_string());
        self.inner
            .add_layer(Box::new(layer))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        console_log!("Added OSM layer: {}", name);
        Ok(())
    }
    
    /// Set the map view
    #[wasm_bindgen]
    pub fn set_view(&mut self, lat: f64, lng: f64, zoom: f64) -> Result<(), JsValue> {
        let center = LatLng::new(lat, lng);
        self.inner
            .set_view(center, zoom)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        console_log!("Set view to: {}, {} at zoom {}", lat, lng, zoom);
        Ok(())
    }
    
    /// Pan the map by pixel offset
    #[wasm_bindgen]
    pub fn pan(&mut self, dx: f64, dy: f64) -> Result<(), JsValue> {
        let delta = Point::new(dx, dy);
        self.inner
            .pan(delta)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(())
    }
    
    /// Zoom to a specific level
    #[wasm_bindgen]
    pub fn zoom_to(&mut self, zoom: f64) -> Result<(), JsValue> {
        self.inner
            .zoom_to(zoom, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        console_log!("Zoomed to level: {}", zoom);
        Ok(())
    }
    
    /// Update the map (call this in animation loop)
    #[wasm_bindgen]
    pub fn update(&mut self, delta_time: f64) -> Result<(), JsValue> {
        self.inner
            .update(delta_time)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(())
    }
    
    /// Set the map size
    #[wasm_bindgen]
    pub fn set_size(&mut self, width: f64, height: f64) {
        let size = Point::new(width, height);
        self.inner.viewport_mut().set_size(size);
    }
    
    /// Get current center latitude
    #[wasm_bindgen]
    pub fn get_center_lat(&self) -> f64 {
        self.inner.viewport().center.lat
    }
    
    /// Get current center longitude
    #[wasm_bindgen]
    pub fn get_center_lng(&self) -> f64 {
        self.inner.viewport().center.lng
    }
    
    /// Get current zoom level
    #[wasm_bindgen]
    pub fn get_zoom(&self) -> f64 {
        self.inner.viewport().zoom
    }
}

/// WASM-compatible LatLng struct
#[wasm_bindgen]
pub struct WasmLatLng {
    pub lat: f64,
    pub lng: f64,
}

#[wasm_bindgen]
impl WasmLatLng {
    #[wasm_bindgen(constructor)]
    pub fn new(lat: f64, lng: f64) -> WasmLatLng {
        WasmLatLng { lat, lng }
    }
}

/// WASM-compatible Point struct
#[wasm_bindgen]
pub struct WasmPoint {
    pub x: f64,
    pub y: f64,
}

#[wasm_bindgen]
impl WasmPoint {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64) -> WasmPoint {
        WasmPoint { x, y }
    }
}

/// Example function to demonstrate map creation
#[wasm_bindgen]
pub fn create_san_francisco_map(width: f64, height: f64) -> WasmMap {
    console_log!("Creating San Francisco map...");
    
    // San Francisco coordinates
    let center = LatLng::new(37.7749, -122.4194);
    let size = Point::new(width, height);
    let mut map = Map::new(center, 12.0, size);
    
    // Add OpenStreetMap tiles
    let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
    if let Err(e) = map.add_layer(Box::new(osm_layer)) {
        console_log!("Failed to add OSM layer: {}", e);
    }
    
    console_log!("San Francisco map created successfully!");
    WasmMap { inner: map }
}

/// Utility function to test WASM functionality
#[wasm_bindgen]
pub fn test_wasm_functionality() -> String {
    console_log!("Testing WASM functionality...");
    
    let center = LatLng::new(40.7128, -74.0060); // New York
    let size = Point::new(800.0, 600.0);
    let map = Map::new(center, 10.0, size);
    
    format!(
        "Map created at ({:.4}, {:.4}) with zoom {:.1} and size {}x{}",
        map.viewport().center.lat,
        map.viewport().center.lng,
        map.viewport().zoom,
        map.viewport().size.x,
        map.viewport().size.y
    )
} 