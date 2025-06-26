use crate::core::geo::Point;
use serde::{Deserialize, Serialize};

/// Input events that can be handled by the map and layers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Single click/tap
    Click { position: Point },
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

impl InputEvent {
    /// Gets the primary position associated with this event, if any
    pub fn position(&self) -> Option<Point> {
        match self {
            InputEvent::Click { position } => Some(*position),
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

/// Event priority levels for handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Event handling result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHandled {
    /// Event was not handled, continue propagation
    NotHandled,
    /// Event was handled, continue propagation
    Handled,
    /// Event was handled, stop propagation
    HandledStopPropagation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_position() {
        let click = InputEvent::Click {
            position: Point::new(100.0, 200.0),
        };
        assert_eq!(click.position(), Some(Point::new(100.0, 200.0)));

        let resize = InputEvent::Resize {
            size: Point::new(800.0, 600.0),
        };
        assert_eq!(resize.position(), None);
    }

    #[test]
    fn test_event_type_checks() {
        let click = InputEvent::Click {
            position: Point::new(0.0, 0.0),
        };
        assert!(click.is_pointer_event());
        assert!(!click.is_touch_event());
        assert!(!click.is_keyboard_event());

        let key_press = InputEvent::KeyPress {
            key: KeyCode::Enter,
            modifiers: KeyModifiers::default(),
        };
        assert!(!key_press.is_pointer_event());
        assert!(!key_press.is_touch_event());
        assert!(key_press.is_keyboard_event());
    }

    #[test]
    fn test_key_modifiers() {
        let mut modifiers = KeyModifiers::default();
        assert!(!modifiers.shift);
        assert!(!modifiers.ctrl);

        modifiers.shift = true;
        modifiers.ctrl = true;
        assert!(modifiers.shift);
        assert!(modifiers.ctrl);
    }
}
