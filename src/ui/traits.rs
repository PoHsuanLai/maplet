use crate::{
    core::{geo::LatLng, viewport::Viewport},
    Result,
};
use egui::{Rect, Response, Ui, Vec2};
use std::any::Any;

/// Core trait for any UI element that can be rendered
pub trait Renderable {
    fn render(&mut self, ui: &mut Ui, rect: Rect) -> Result<Response>;
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
}

/// Trait for UI elements that can be positioned
pub trait Positionable {
    type Position;

    fn get_position(&self) -> &Self::Position;
    fn set_position(&mut self, position: Self::Position);
    fn calculate_rect(&self, container_rect: Rect, size: Vec2) -> Rect;
}

/// Trait for UI elements that can be styled
pub trait Styleable {
    type Style;

    fn get_style(&self) -> &Self::Style;
    fn set_style(&mut self, style: Self::Style);
    fn apply_style(&mut self, ui: &mut Ui);
}

/// Trait for UI elements that can handle events
pub trait EventHandler {
    type Event;
    type EventResult;

    fn handle_event(&mut self, event: Self::Event) -> Result<Self::EventResult>;
    fn can_handle(&self, event: &Self::Event) -> bool;
}

/// Trait for components that respond to viewport changes
pub trait ViewportAware {
    fn on_viewport_changed(&mut self, viewport: &Viewport) -> Result<()>;
    fn requires_viewport_updates(&self) -> bool;
}

/// Trait for components that can be dynamically cast
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Generic implementation for AsAny trait
impl<T: 'static> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Common UI events
#[derive(Debug, Clone)]
pub enum UiEvent {
    Click { position: LatLng },
    DoubleClick { position: LatLng },
    Drag { delta: Vec2 },
    Scroll { delta: f64, position: LatLng },
    KeyPress { key: String },
    Touch { phase: TouchPhase, position: LatLng },
    Hover { position: Option<LatLng> },
    Focus { gained: bool },
}

#[derive(Debug, Clone)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Common UI event results
#[derive(Debug, Clone)]
pub enum UiEventResult {
    Handled,
    NotHandled,
    Consumed,
    Propagate,
}

/// Base configuration for UI components
#[derive(Debug, Clone)]
pub struct BaseConfig {
    pub visible: bool,
    pub interactive: bool,
    pub z_index: i32,
    pub margin: f32,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            visible: true,
            interactive: true,
            z_index: 0,
            margin: 10.0,
        }
    }
}
