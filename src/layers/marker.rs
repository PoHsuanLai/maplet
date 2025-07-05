use crate::{
    core::geo::LatLng,
    layers::base::{LayerProperties, LayerTrait, LayerType},
};

pub struct Marker {
    properties: LayerProperties,
    position: LatLng,
    popup_text: Option<String>,
}

impl Marker {
    pub fn new(id: String, position: LatLng) -> Self {
        let properties = LayerProperties::new(id, "Marker".to_string(), LayerType::Marker);
        Self {
            properties,
            position,
            popup_text: None,
        }
    }

    pub fn with_popup(mut self, text: String) -> Self {
        self.popup_text = Some(text);
        self
    }

    pub fn position(&self) -> LatLng {
        self.position
    }

    pub fn set_position(&mut self, position: LatLng) {
        self.position = position;
    }
}

impl LayerTrait for Marker {
    crate::impl_layer_trait!(Marker, properties);

    fn options(&self) -> serde_json::Value {
        serde_json::json!({
            "position": {
                "lat": self.position.lat,
                "lng": self.position.lng
            },
            "popup": self.popup_text
        })
    }

    crate::impl_todo_options_setting!();
    crate::impl_todo_render!();

    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        Some(crate::core::geo::LatLngBounds::new(
            self.position,
            self.position,
        ))
    }
}
