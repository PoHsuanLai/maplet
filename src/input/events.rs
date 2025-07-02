use crate::core::geo::{LatLng, Point};
use serde::{Deserialize, Serialize};

/// Input events that can be handled by the map and layers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Single click/tap
    Click {
        position: Point,
        button: MouseButton,
    },
    /// Double click/tap
    DoubleClick { position: Point },
    /// Mouse/finger move
    MouseMove { position: Point },
    /// Start of drag operation
    DragStart { position: Point },
    /// Drag in progress
    Drag { delta: Point },
    /// End of drag operation
    DragEnd,
    /// Scroll wheel or pinch zoom
    Scroll { delta: f64, position: Point },
    /// Keyboard input
    KeyPress {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    /// Viewport/window resize
    Resize { size: Point },
    /// Touch events (multi-touch)
    Touch {
        event_type: TouchEventType,
        touches: Vec<TouchPoint>,
    },
}

/// Types of touch events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TouchEventType {
    Start,
    Move,
    End,
    Cancel,
}

/// Individual touch point
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchPoint {
    pub id: u64,
    pub position: Point,
    pub previous_position: Option<Point>,
    pub pressure: f32,
}

/// Keyboard key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Plus,
    Minus,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Enter,
    Space,
    Tab,
    Other(u32),
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Priority levels for input events
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Whether an event was handled
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventHandled {
    Handled,
    NotHandled,
}

/// Map event types that can be emitted by the map
#[derive(Debug, Clone, PartialEq)]
pub enum MapEvent {
    /// Map view has changed (center, zoom, or size)
    ViewChanged { center: LatLng, zoom: f64 },
    /// Mouse/touch click on the map
    Click { lat_lng: LatLng, pixel: Point },
    /// Mouse/touch move over the map
    MouseMove { lat_lng: LatLng, pixel: Point },
    /// Zoom started
    ZoomStart { zoom: f64 },
    /// Zoom ended
    ZoomEnd { zoom: f64 },
    /// Pan started
    MoveStart { center: LatLng },
    /// Pan ended
    MoveEnd { center: LatLng },
    /// Layer was added to the map
    LayerAdd { layer_id: String },
    /// Layer was removed from the map  
    LayerRemove { layer_id: String },
    /// Base layer was changed
    BaseLayerChange { layer_id: String },
    /// Overlay layer was added
    OverlayAdd { layer_id: String },
    /// Overlay layer was removed
    OverlayRemove { layer_id: String },
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

impl InputEvent {
    /// Gets the primary position associated with this event, if any
    pub fn position(&self) -> Option<Point> {
        match self {
            InputEvent::Click { position, .. } => Some(*position),
            InputEvent::DoubleClick { position } => Some(*position),
            InputEvent::MouseMove { position } => Some(*position),
            InputEvent::DragStart { position } => Some(*position),
            InputEvent::Scroll { position, .. } => Some(*position),
            InputEvent::Touch { touches, .. } => touches.first().map(|t| t.position),
            _ => None,
        }
    }

    /// Checks if this is a mouse/pointer event
    pub fn is_pointer_event(&self) -> bool {
        matches!(
            self,
            InputEvent::Click { .. }
                | InputEvent::DoubleClick { .. }
                | InputEvent::MouseMove { .. }
                | InputEvent::DragStart { .. }
                | InputEvent::Drag { .. }
                | InputEvent::DragEnd
                | InputEvent::Scroll { .. }
        )
    }

    /// Checks if this is a touch event
    pub fn is_touch_event(&self) -> bool {
        matches!(self, InputEvent::Touch { .. })
    }

    /// Checks if this is a keyboard event
    pub fn is_keyboard_event(&self) -> bool {
        matches!(self, InputEvent::KeyPress { .. })
    }
}

// Event conversion utilities
impl From<MapEvent> for InputEvent {
    fn from(map_event: MapEvent) -> Self {
        match map_event {
            MapEvent::Click { pixel, .. } => InputEvent::Click {
                position: pixel,
                button: MouseButton::Left,
            },
            MapEvent::MouseMove { pixel, .. } => InputEvent::MouseMove { position: pixel },
            _ => InputEvent::MouseMove {
                position: Point::new(0.0, 0.0),
            }, // Fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_position() {
        let click = InputEvent::Click {
            position: Point::new(100.0, 200.0),
            button: MouseButton::Left,
        };
        assert_eq!(click.position(), Some(Point::new(100.0, 200.0)));

        let move_event = InputEvent::MouseMove {
            position: Point::new(50.0, 75.0),
        };
        assert_eq!(move_event.position(), Some(Point::new(50.0, 75.0)));
    }

    #[test]
    fn test_event_type_checks() {
        let click = InputEvent::Click {
            position: Point::new(0.0, 0.0),
            button: MouseButton::Left,
        };
        assert!(click.is_pointer_event());
        assert!(!click.is_touch_event());
        assert!(!click.is_keyboard_event());

        let key_press = InputEvent::KeyPress {
            key: KeyCode::Space,
            modifiers: KeyModifiers::default(),
        };
        assert!(!key_press.is_pointer_event());
        assert!(!key_press.is_touch_event());
        assert!(key_press.is_keyboard_event());
    }

    #[test]
    fn test_key_modifiers() {
        let modifiers = KeyModifiers {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        assert!(modifiers.shift);
        assert!(!modifiers.ctrl);
        assert!(modifiers.alt);
        assert!(!modifiers.meta);
    }
}
