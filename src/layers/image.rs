use crate::{
    core::geo::LatLngBounds,
    layers::base::{LayerProperties, LayerTrait, LayerType},
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

    crate::impl_todo_render!();

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

    crate::impl_todo_options_setting!();
}
