use crate::{core::viewport::Viewport, input::events::InputEvent, Result};

#[cfg(feature = "render")]
use crate::rendering::context::RenderContext;

/// Base trait that all layers must implement
pub trait LayerTrait: Send + Sync {
    /// Gets the unique identifier for this layer
    fn id(&self) -> &str;

    /// Gets the display name of the layer
    fn name(&self) -> &str;

    /// Gets the layer type
    fn layer_type(&self) -> LayerType;

    /// Gets the z-index (drawing order) of the layer
    fn z_index(&self) -> i32;

    /// Sets the z-index of the layer
    fn set_z_index(&mut self, z_index: i32);

    /// Gets the opacity of the layer (0.0 to 1.0)
    fn opacity(&self) -> f32;

    /// Sets the opacity of the layer
    fn set_opacity(&mut self, opacity: f32);

    /// Gets whether the layer is visible
    fn visible(&self) -> bool;

    /// Sets whether the layer is visible
    fn set_visible(&mut self, visible: bool);

    /// Called when the layer is added to a map
    fn on_add(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }

    /// Called when the layer is removed from a map
    fn on_remove(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }

    /// Renders the layer (when render feature is enabled)
    #[cfg(feature = "render")]
    fn render(&self, context: &mut RenderContext, viewport: &Viewport) -> Result<()>;

    /// No-op render method when render feature is disabled
    #[cfg(not(feature = "render"))]
    fn render(&self, _context: &mut (), _viewport: &Viewport) -> Result<()> {
        Ok(()) // Default to no-op when rendering is disabled
    }

    /// Handles input events
    fn handle_input(&mut self, _input: &InputEvent) -> Result<()> {
        Ok(())
    }

    /// Updates the layer state (called each frame)
    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }

    /// Gets the bounds of the layer content (if applicable)
    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        None
    }

    /// Checks if the layer intersects with the given bounds
    fn intersects_bounds(&self, bounds: &crate::core::geo::LatLngBounds) -> bool {
        if let Some(layer_bounds) = self.bounds() {
            layer_bounds.intersects(bounds)
        } else {
            true // Assume it might be visible
        }
    }

    /// Gets layer-specific options as JSON
    fn options(&self) -> serde_json::Value;

    /// Sets layer-specific options from JSON
    fn set_options(&mut self, options: serde_json::Value) -> Result<()>;

    /// Returns self as `Any` for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns mutable self as `Any` for downcasting
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Types of layers supported by the mapping library
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    /// Tile-based layer (e.g., OSM, satellite imagery)
    Tile,
    /// Vector layer (points, lines, polygons)
    Vector,
    /// Individual markers/points of interest
    Marker,
    /// Raster image overlays
    Image,
    /// Custom canvas-drawn content
    Canvas,
    /// Plugin-defined custom layer
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

/// Common layer properties that most layers will need
#[derive(Debug, Clone)]
pub struct LayerProperties {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Layer type
    pub layer_type: LayerType,
    /// Drawing order (higher numbers drawn on top)
    pub z_index: i32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Whether the layer is visible
    pub visible: bool,
    /// Whether the layer is interactive (responds to mouse/touch events)
    pub interactive: bool,
    /// Custom options specific to this layer
    pub options: serde_json::Value,
}

impl LayerProperties {
    /// Creates new layer properties with default values
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

/// A simple base layer implementation that other layers can extend
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

    #[cfg(feature = "render")]
    fn render(&self, _context: &mut RenderContext, _viewport: &Viewport) -> Result<()> {
        // Base implementation does nothing
        Ok(())
    }

    #[cfg(not(feature = "render"))]
    fn render(&self, _context: &mut (), _viewport: &Viewport) -> Result<()> {
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
