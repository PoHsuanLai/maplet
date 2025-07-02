pub mod events;
pub mod handler;

// Re-export the essential types
pub use events::{
    EventHandled, EventPriority, InputEvent, KeyCode, KeyModifiers, MapEvent, MouseButton,
    TouchEventType, TouchPoint,
};
pub use handler::{Action, EventManager, InputHandler, MapOperations};
