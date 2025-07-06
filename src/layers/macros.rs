//! Macros to reduce boilerplate in layer implementations
//!
//! This module provides macros that generate common LayerTrait implementations
//! to avoid code duplication across different layer types.

/// Macro to implement the standard LayerTrait boilerplate methods
///
/// This generates implementations for:
/// - id(), name(), layer_type()
/// - z_index(), set_z_index()
/// - opacity(), set_opacity()
/// - visible(), set_visible()
/// - as_any(), as_any_mut()
///
/// Usage:
/// ```rust
/// impl_layer_trait_boilerplate!(MyLayer, properties);
/// ```
#[macro_export]
macro_rules! impl_layer_trait {
    ($layer_type:ty, $properties_field:ident) => {
        fn id(&self) -> &str {
            &self.$properties_field.id
        }

        fn name(&self) -> &str {
            &self.$properties_field.name
        }

        fn layer_type(&self) -> LayerType {
            self.$properties_field.layer_type
        }

        fn z_index(&self) -> i32 {
            self.$properties_field.z_index
        }

        fn set_z_index(&mut self, z_index: i32) {
            self.$properties_field.z_index = z_index;
        }

        fn opacity(&self) -> f32 {
            self.$properties_field.opacity
        }

        fn set_opacity(&mut self, opacity: f32) {
            self.$properties_field.opacity = opacity.clamp(0.0, 1.0);
        }

        fn is_visible(&self) -> bool {
            self.$properties_field.visible
        }

        fn set_visible(&mut self, visible: bool) {
            self.$properties_field.visible = visible;
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}

/// Macro to implement basic options handling for layers that store options in properties
#[macro_export]
macro_rules! impl_basic_options {
    ($properties_field:ident) => {
        fn options(&self) -> serde_json::Value {
            self.$properties_field.options.clone()
        }

        fn set_options(&mut self, options: serde_json::Value) -> $crate::Result<()> {
            self.$properties_field.options = options;
            Ok(())
        }
    };
}

/// Macro to create a layer constructor with LayerProperties
#[macro_export]
macro_rules! impl_layer_constructor {
    ($layer_type:ty, $layer_enum:expr) => {
        pub fn new(id: String, name: String) -> Self {
            let properties = $crate::layers::base::LayerProperties::new(id, name, $layer_enum);
            Self { properties }
        }
    };
}

/// Macro to implement default options serialization for layers with just properties
#[macro_export]
macro_rules! impl_default_options_serialization {
    ($properties_field:ident) => {
        fn options(&self) -> serde_json::Value {
            serde_json::json!({
                "id": self.$properties_field.id,
                "name": self.$properties_field.name,
                "layer_type": self.$properties_field.layer_type.to_string(),
                "z_index": self.$properties_field.z_index,
                "opacity": self.$properties_field.opacity,
                "visible": self.$properties_field.visible,
                "interactive": self.$properties_field.interactive
            })
        }
    };
}

/// Macro to implement standard layer TODO option setting
#[macro_export]
macro_rules! impl_todo_options_setting {
    () => {
        fn set_options(&mut self, _options: serde_json::Value) -> $crate::Result<()> {
            // TODO: Implement option setting
            Ok(())
        }
    };
}

/// Macro to implement standard layer render TODO
#[macro_export]
macro_rules! impl_todo_render {
    () => {
        fn render(
            &mut self,
            _context: &mut $crate::rendering::context::RenderContext,
            _viewport: &$crate::core::viewport::Viewport,
        ) -> $crate::Result<()> {
            // TODO: Implement rendering
            Ok(())
        }
    };
}
