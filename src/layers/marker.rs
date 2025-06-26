use crate::{
    core::geo::LatLng,
    layers::base::{LayerProperties, LayerTrait, LayerType},
    Result,
};

use once_cell::sync::Lazy;

#[cfg(feature = "render")]
use image;

pub struct Marker {
    properties: LayerProperties,
    pub position: LatLng,
}

impl Marker {
    pub fn new(id: String, position: LatLng) -> Self {
        let properties = LayerProperties::new(id, "Marker".to_string(), LayerType::Marker);
        Self {
            properties,
            position,
        }
    }
}

static MARKER_BYTES: &[u8] = include_bytes!("../../assets/images/marker-icon.png");

// Decode once to RGBA
static MARKER_RGBA: Lazy<Vec<u8>> = Lazy::new(|| {
    let img = image::load_from_memory(MARKER_BYTES).expect("embedded marker icon should decode");
    img.to_rgba8().into_raw()
});

static MARKER_SIZE: (u32, u32) = (25, 41); // standard leaflet icon

impl LayerTrait for Marker {
    fn id(&self) -> &str {
        &self.properties.id
    }
    fn name(&self) -> &str {
        &self.properties.name
    }
    fn layer_type(&self) -> LayerType {
        LayerType::Marker
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
    fn options(&self) -> serde_json::Value {
        self.properties.options.clone()
    }
    fn set_options(&mut self, options: serde_json::Value) -> Result<()> {
        self.properties.options = options;
        Ok(())
    }

    fn render(
        &self,
        #[cfg(feature = "render")] context: &mut crate::rendering::context::RenderContext,
        #[cfg(not(feature = "render"))] context: &mut (),
        viewport: &crate::core::viewport::Viewport,
    ) -> Result<()> {
        // Convert position to pixel coords
        let pixel = viewport.lat_lng_to_pixel(&self.position);

        let half_w = MARKER_SIZE.0 as f64 / 2.0;
        let h = MARKER_SIZE.1 as f64;

        let min = crate::core::geo::Point::new(pixel.x - half_w, pixel.y - h);
        let max = crate::core::geo::Point::new(pixel.x + half_w, pixel.y);

        context.render_tile(&MARKER_RGBA, (min, max), self.opacity())
    }

    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        Some(crate::core::geo::LatLngBounds::new(
            self.position,
            self.position,
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
