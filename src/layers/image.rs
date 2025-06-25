use crate::{
    layers::base::{LayerProperties, LayerTrait, LayerType},
    Result,
};
use async_trait::async_trait;

pub struct ImageLayer {
    properties: LayerProperties,
}

impl ImageLayer {
    pub fn new(id: String) -> Self {
        let properties = LayerProperties::new(id, "Image Layer".to_string(), LayerType::Image);
        Self { properties }
    }
}

#[async_trait]
impl LayerTrait for ImageLayer {
    fn id(&self) -> &str {
        &self.properties.id
    }
    fn name(&self) -> &str {
        &self.properties.name
    }
    fn layer_type(&self) -> LayerType {
        LayerType::Image
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

    async fn render(
        &self,
        _context: &mut crate::rendering::context::RenderContext,
        _viewport: &crate::core::viewport::Viewport,
    ) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
