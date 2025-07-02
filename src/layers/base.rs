use crate::{core::viewport::Viewport, input::events::InputEvent, Result};

use crate::rendering::context::RenderContext;

pub trait LayerTrait: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    fn layer_type(&self) -> LayerType;

    fn z_index(&self) -> i32;

    fn set_z_index(&mut self, z_index: i32);

    fn opacity(&self) -> f32;

    fn set_opacity(&mut self, opacity: f32);

    fn visible(&self) -> bool;

    fn set_visible(&mut self, visible: bool);

    fn on_add(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }

    fn on_remove(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()>;

    fn handle_input(&mut self, _input: &InputEvent) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }

    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        None
    }

    fn intersects_bounds(&self, bounds: &crate::core::geo::LatLngBounds) -> bool {
        if let Some(layer_bounds) = self.bounds() {
            layer_bounds.intersects(bounds)
        } else {
            true
        }
    }

    fn options(&self) -> serde_json::Value;

    fn set_options(&mut self, options: serde_json::Value) -> Result<()>;

    fn as_any(&self) -> &dyn std::any::Any;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    Tile,
    Vector,
    Marker,
    Image,
    Canvas,
    Custom,
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerType::Tile => write!(f, "tile"),
            LayerType::Vector => write!(f, "vector"),
            LayerType::Marker => write!(f, "marker"),
            LayerType::Image => write!(f, "image"),
            LayerType::Canvas => write!(f, "canvas"),
            LayerType::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerProperties {
    pub id: String,
    pub name: String,
    pub layer_type: LayerType,
    pub z_index: i32,
    pub opacity: f32,
    pub visible: bool,
    pub interactive: bool,
    pub options: serde_json::Value,
}

impl LayerProperties {
    pub fn new(id: String, name: String, layer_type: LayerType) -> Self {
        Self {
            id,
            name,
            layer_type,
            z_index: 0,
            opacity: 1.0,
            visible: true,
            interactive: true,
            options: serde_json::Value::Null,
        }
    }
}

impl Default for LayerProperties {
    fn default() -> Self {
        Self::new(
            "default".to_string(),
            "Default Layer".to_string(),
            LayerType::Custom,
        )
    }
}

pub struct BaseLayer {
    pub properties: LayerProperties,
}

impl BaseLayer {
    pub fn new(properties: LayerProperties) -> Self {
        Self { properties }
    }
}

impl LayerTrait for BaseLayer {
    fn id(&self) -> &str {
        &self.properties.id
    }

    fn name(&self) -> &str {
        &self.properties.name
    }

    fn layer_type(&self) -> LayerType {
        self.properties.layer_type
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

    fn render(&mut self, _context: &mut RenderContext, _viewport: &Viewport) -> Result<()> {
        Ok(())
    }

    fn options(&self) -> serde_json::Value {
        self.properties.options.clone()
    }

    fn set_options(&mut self, options: serde_json::Value) -> Result<()> {
        self.properties.options = options;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any
    where
        Self: 'static,
    {
        self as &dyn std::any::Any
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any
    where
        Self: 'static,
    {
        self as &mut dyn std::any::Any
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_properties() {
        let props = LayerProperties::new(
            "test".to_string(),
            "Test Layer".to_string(),
            LayerType::Vector,
        );

        assert_eq!(props.id, "test");
        assert_eq!(props.name, "Test Layer");
        assert_eq!(props.layer_type, LayerType::Vector);
        assert_eq!(props.z_index, 0);
        assert_eq!(props.opacity, 1.0);
        assert!(props.visible);
    }

    #[test]
    fn test_base_layer() {
        let props = LayerProperties::new(
            "base".to_string(),
            "Base Layer".to_string(),
            LayerType::Custom,
        );
        let mut layer = BaseLayer::new(props);

        assert_eq!(layer.id(), "base");
        assert_eq!(layer.opacity(), 1.0);

        layer.set_opacity(0.5);
        assert_eq!(layer.opacity(), 0.5);

        layer.set_visible(false);
        assert!(!layer.visible());
    }

    #[test]
    fn test_layer_type_display() {
        assert_eq!(LayerType::Tile.to_string(), "tile");
        assert_eq!(LayerType::Vector.to_string(), "vector");
        assert_eq!(LayerType::Marker.to_string(), "marker");
    }
}
