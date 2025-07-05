use crate::{
    core::{geo::LatLng, viewport::Viewport},
    layers::base::{LayerProperties, LayerTrait, LayerType},
    rendering::context::RenderContext,
    Result,
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

    fn set_options(&mut self, _options: serde_json::Value) -> Result<()> {
        // TODO: Implement option setting
        Ok(())
    }

    fn render(
        &mut self,
        _context: &mut RenderContext,
        _viewport: &Viewport,
    ) -> Result<()> {
        // TODO: Implement marker rendering
        Ok(())
    }

    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        Some(crate::core::geo::LatLngBounds::new(
            self.position,
            self.position,
        ))
    }
}
