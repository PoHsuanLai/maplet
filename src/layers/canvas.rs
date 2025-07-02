use crate::{
    core::viewport::Viewport,
    layers::base::{LayerProperties, LayerTrait, LayerType},
    rendering::context::RenderContext,
    Result,
};

pub struct CanvasLayer {
    properties: LayerProperties,
}

impl CanvasLayer {
    pub fn new(id: String, name: String) -> Self {
        let properties = LayerProperties::new(id, name, LayerType::Canvas);
        Self { properties }
    }
}

impl LayerTrait for CanvasLayer {
    crate::impl_layer_trait!(CanvasLayer, properties);
    crate::impl_basic_options!(properties);

    fn render(
        &mut self,
        _context: &mut RenderContext,
        _viewport: &Viewport,
    ) -> Result<()> {
        Ok(())
    }
}
