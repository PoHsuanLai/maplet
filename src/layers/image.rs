use crate::{
    core::{geo::LatLngBounds, viewport::Viewport},
    layers::base::{LayerProperties, LayerTrait, LayerType},
    rendering::context::RenderContext,
    Result,
};

pub struct ImageLayer {
    properties: LayerProperties,
    url: String,
    bounds: LatLngBounds,
}

impl ImageLayer {
    pub fn new(id: String, url: String, bounds: LatLngBounds) -> Self {
        let properties = LayerProperties::new(id, "Image Layer".to_string(), LayerType::Image);
        Self {
            properties,
            url,
            bounds,
        }
    }
}

impl LayerTrait for ImageLayer {
    crate::impl_layer_trait!(ImageLayer, properties);

    fn bounds(&self) -> Option<LatLngBounds> {
        Some(self.bounds.clone())
    }

    fn render(
        &mut self,
        _context: &mut RenderContext,
        _viewport: &Viewport,
    ) -> Result<()> {
        // TODO: Implement image rendering
        Ok(())
    }

    fn options(&self) -> serde_json::Value {
        serde_json::json!({
            "url": self.url,
            "bounds": {
                "south": self.bounds.south_west.lat,
                "west": self.bounds.south_west.lng,
                "north": self.bounds.north_east.lat,
                "east": self.bounds.north_east.lng
            }
        })
    }

    fn set_options(&mut self, _options: serde_json::Value) -> Result<()> {
        // TODO: Implement option setting
        Ok(())
    }
}
