use crate::{
    core::geo::Point,
    input::events::{EventHandled, EventPriority, InputEvent},
    input::gestures::GestureRecognizer,
    Result,
};
use std::collections::{HashMap, VecDeque};

/// Trait for objects that can handle input events
pub trait InputEventHandler {
    /// Handle an input event, returning whether it was handled and should stop propagation
    fn handle_event(&mut self, event: &InputEvent) -> EventHandled;

    /// Get the priority of this handler
    fn priority(&self) -> EventPriority;

    /// Whether this handler is currently enabled
    fn is_enabled(&self) -> bool;
}

/// Handler registration information
struct HandlerInfo {
    id: u32,
    priority: EventPriority,
    enabled: bool,
}

/// Main input handler that manages the entire input pipeline
pub struct InputHandler {
    pub enabled: bool,
    gesture_recognizer: GestureRecognizer,
    handlers: HashMap<u32, Box<dyn InputEventHandler>>,
    handler_order: Vec<HandlerInfo>,
    next_handler_id: u32,
    event_queue: VecDeque<InputEvent>,
    focus_handler: Option<u32>,
    capture_handler: Option<u32>,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            enabled: true,
            gesture_recognizer: GestureRecognizer::new(),
            handlers: HashMap::new(),
            handler_order: Vec::new(),
            next_handler_id: 0,
            event_queue: VecDeque::new(),
            focus_handler: None,
            capture_handler: None,
        }
    }

    /// Registers a new input event handler
    pub fn register_handler(&mut self, handler: Box<dyn InputEventHandler>) -> u32 {
        let id = self.next_handler_id;
        self.next_handler_id += 1;

        let priority = handler.priority();
        let enabled = handler.is_enabled();

        self.handlers.insert(id, handler);

        // Insert in priority order (higher priority first)
        let handler_info = HandlerInfo {
            id,
            priority,
            enabled,
        };
        let insert_pos = self
            .handler_order
            .iter()
            .position(|h| h.priority < priority)
            .unwrap_or(self.handler_order.len());

        self.handler_order.insert(insert_pos, handler_info);

        id
    }

    /// Unregisters an input event handler
    pub fn unregister_handler(&mut self, handler_id: u32) -> bool {
        if self.handlers.remove(&handler_id).is_some() {
            self.handler_order.retain(|h| h.id != handler_id);

            // Clear focus/capture if this handler had it
            if self.focus_handler == Some(handler_id) {
                self.focus_handler = None;
            }
            if self.capture_handler == Some(handler_id) {
                self.capture_handler = None;
            }

            true
        } else {
            false
        }
    }

    /// Sets focus to a specific handler (keyboard events go here first)
    pub fn set_focus(&mut self, handler_id: Option<u32>) {
        if let Some(id) = handler_id {
            if self.handlers.contains_key(&id) {
                self.focus_handler = Some(id);
            }
        } else {
            self.focus_handler = None;
        }
    }

    /// Sets capture for a specific handler (all events go here first)
    pub fn set_capture(&mut self, handler_id: Option<u32>) {
        if let Some(id) = handler_id {
            if self.handlers.contains_key(&id) {
                self.capture_handler = Some(id);
            }
        } else {
            self.capture_handler = None;
        }
    }

    /// Enables or disables a specific handler
    pub fn set_handler_enabled(&mut self, handler_id: u32, enabled: bool) {
        if let Some(handler_info) = self.handler_order.iter_mut().find(|h| h.id == handler_id) {
            handler_info.enabled = enabled;
        }
    }

    /// Main event handling method
    pub fn handle_event(&mut self, event: InputEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // First, pass through gesture recognizer
        let processed_events = self.gesture_recognizer.process_input(event);

        // Process each event from gesture recognition
        for processed_event in processed_events {
            self.dispatch_event(processed_event)?;
        }

        Ok(())
    }

    /// Dispatches an event to appropriate handlers
    fn dispatch_event(&mut self, event: InputEvent) -> Result<()> {
        // Handle capture phase
        if let Some(capture_id) = self.capture_handler {
            if let Some(handler) = self.handlers.get_mut(&capture_id) {
                match handler.handle_event(&event) {
                    EventHandled::HandledStopPropagation => return Ok(()),
                    EventHandled::Handled | EventHandled::NotHandled => {
                        // Continue to normal dispatch
                    }
                }
            }
        }

        // Handle focus for keyboard events
        if event.is_keyboard_event() {
            if let Some(focus_id) = self.focus_handler {
                if let Some(handler) = self.handlers.get_mut(&focus_id) {
                    if handler.is_enabled() {
                        match handler.handle_event(&event) {
                            EventHandled::HandledStopPropagation => return Ok(()),
                            EventHandled::Handled | EventHandled::NotHandled => {
                                // Continue to normal dispatch if not handled
                            }
                        }
                    }
                }
            }
        }

        // Normal event bubbling (highest priority first)
        for handler_info in &self.handler_order {
            if !handler_info.enabled {
                continue;
            }

            if let Some(handler) = self.handlers.get_mut(&handler_info.id) {
                if handler.is_enabled() {
                    match handler.handle_event(&event) {
                        EventHandled::HandledStopPropagation => break,
                        EventHandled::Handled | EventHandled::NotHandled => {
                            // Continue to next handler
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Queues an event for later processing
    pub fn queue_event(&mut self, event: InputEvent) {
        self.event_queue.push_back(event);
    }

    /// Processes all queued events
    pub fn process_queued_events(&mut self) -> Result<()> {
        while let Some(event) = self.event_queue.pop_front() {
            self.handle_event(event)?;
        }
        Ok(())
    }

    /// Gets the gesture recognizer for configuration
    pub fn gesture_recognizer(&mut self) -> &mut GestureRecognizer {
        &mut self.gesture_recognizer
    }

    /// Clears all event state
    pub fn clear(&mut self) {
        self.event_queue.clear();
        self.gesture_recognizer.reset();
        self.focus_handler = None;
        self.capture_handler = None;
    }

    /// Gets information about registered handlers
    pub fn handler_info(&self) -> Vec<(u32, EventPriority, bool)> {
        self.handler_order
            .iter()
            .map(|h| (h.id, h.priority, h.enabled))
            .collect()
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in handler for basic map interactions
pub struct MapInputHandler {
    enabled: bool,
    priority: EventPriority,
    last_mouse_position: Option<Point>,
    is_dragging: bool,
}

impl MapInputHandler {
    pub fn new() -> Self {
        Self {
            enabled: true,
            priority: EventPriority::Normal,
            last_mouse_position: None,
            is_dragging: false,
        }
    }

    pub fn with_priority(priority: EventPriority) -> Self {
        Self {
            enabled: true,
            priority,
            last_mouse_position: None,
            is_dragging: false,
        }
    }
}

impl InputEventHandler for MapInputHandler {
    fn handle_event(&mut self, event: &InputEvent) -> EventHandled {
        if !self.enabled {
            return EventHandled::NotHandled;
        }

        match event {
            InputEvent::Click { position } => {
                self.last_mouse_position = Some(*position);
                // Map click - could emit map click event here
                EventHandled::Handled
            }
            InputEvent::DoubleClick { position } => {
                self.last_mouse_position = Some(*position);
                // Double click for zoom
                EventHandled::Handled
            }
            InputEvent::MouseMove { position } => {
                self.last_mouse_position = Some(*position);
                if !self.is_dragging {
                    // Just mouse move, update hover state
                    EventHandled::Handled
                } else {
                    EventHandled::NotHandled // Let drag handler take it
                }
            }
            InputEvent::DragStart { position } => {
                self.is_dragging = true;
                self.last_mouse_position = Some(*position);
                // Start map pan
                EventHandled::Handled
            }
            InputEvent::Drag { delta: _ } => {
                if self.is_dragging {
                    // Continue map pan
                    EventHandled::Handled
                } else {
                    EventHandled::NotHandled
                }
            }
            InputEvent::DragEnd => {
                if self.is_dragging {
                    self.is_dragging = false;
                    // End map pan
                    EventHandled::Handled
                } else {
                    EventHandled::NotHandled
                }
            }
            InputEvent::Scroll { delta: _, position } => {
                self.last_mouse_position = Some(*position);
                // Map zoom
                EventHandled::Handled
            }
            InputEvent::KeyPress {
                key: _,
                modifiers: _,
            } => {
                // Handle keyboard navigation
                EventHandled::Handled
            }
            InputEvent::Resize { size: _ } => {
                // Handle viewport resize
                EventHandled::Handled
            }
            InputEvent::Touch { .. } => {
                // Handle touch events
                EventHandled::Handled
            }
        }
    }

    fn priority(&self) -> EventPriority {
        self.priority
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for MapInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler for UI controls (buttons, menus, etc.)
pub struct ControlInputHandler {
    enabled: bool,
    priority: EventPriority,
    hot_control: Option<String>,
    active_control: Option<String>,
}

impl ControlInputHandler {
    pub fn new() -> Self {
        Self {
            enabled: true,
            priority: EventPriority::High, // UI controls have higher priority than map
            hot_control: None,
            active_control: None,
        }
    }

    pub fn set_hot_control(&mut self, control_id: Option<String>) {
        self.hot_control = control_id;
    }

    pub fn set_active_control(&mut self, control_id: Option<String>) {
        self.active_control = control_id;
    }
}

impl InputEventHandler for ControlInputHandler {
    fn handle_event(&mut self, event: &InputEvent) -> EventHandled {
        if !self.enabled {
            return EventHandled::NotHandled;
        }

        match event {
            InputEvent::Click { position: _ } => {
                if self.hot_control.is_some() {
                    // Click on control
                    self.active_control = self.hot_control.clone();
                    EventHandled::HandledStopPropagation
                } else {
                    EventHandled::NotHandled
                }
            }
            InputEvent::MouseMove { position: _ } => {
                // Update hot control based on mouse position
                // This would typically involve hit testing against control bounds
                EventHandled::NotHandled // Don't stop propagation for mouse moves
            }
            _ => EventHandled::NotHandled,
        }
    }

    fn priority(&self) -> EventPriority {
        self.priority
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for ControlInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler {
        handled_events: Vec<InputEvent>,
        return_value: EventHandled,
        priority: EventPriority,
    }

    impl TestHandler {
        fn new(return_value: EventHandled) -> Self {
            Self {
                handled_events: Vec::new(),
                return_value,
                priority: EventPriority::Normal,
            }
        }

        fn with_priority(return_value: EventHandled, priority: EventPriority) -> Self {
            Self {
                handled_events: Vec::new(),
                return_value,
                priority,
            }
        }
    }

    impl InputEventHandler for TestHandler {
        fn handle_event(&mut self, event: &InputEvent) -> EventHandled {
            self.handled_events.push(event.clone());
            self.return_value
        }

        fn priority(&self) -> EventPriority {
            self.priority
        }

        fn is_enabled(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_handler_registration() {
        let mut input_handler = InputHandler::new();
        let test_handler = Box::new(TestHandler::new(EventHandled::Handled));

        let id = input_handler.register_handler(test_handler);
        assert_eq!(id, 0);
        assert_eq!(input_handler.handlers.len(), 1);
    }

    #[test]
    fn test_handler_unregistration() {
        let mut input_handler = InputHandler::new();
        let test_handler = Box::new(TestHandler::new(EventHandled::Handled));

        let id = input_handler.register_handler(test_handler);
        assert!(input_handler.unregister_handler(id));
        assert_eq!(input_handler.handlers.len(), 0);
    }

    #[test]
    fn test_event_priority() {
        let mut input_handler = InputHandler::new();

        let low_handler = Box::new(TestHandler::with_priority(
            EventHandled::NotHandled,
            EventPriority::Low,
        ));
        let high_handler = Box::new(TestHandler::with_priority(
            EventHandled::NotHandled,
            EventPriority::High,
        ));

        input_handler.register_handler(low_handler);
        input_handler.register_handler(high_handler);

        // High priority should come first in handler order
        let info = input_handler.handler_info();
        assert_eq!(info[0].1, EventPriority::High);
        assert_eq!(info[1].1, EventPriority::Low);
    }

    #[test]
    fn test_event_handling() {
        let mut input_handler = InputHandler::new();
        let test_handler = Box::new(TestHandler::new(EventHandled::Handled));

        input_handler.register_handler(test_handler);

        let event = InputEvent::Click {
            position: Point::new(100.0, 100.0),
        };

        input_handler.handle_event(event).unwrap();

        // Event should have been processed through gesture recognizer and handlers
        // Can't easily test this without access to internal handler state
    }

    #[test]
    fn test_focus_handling() {
        let mut input_handler = InputHandler::new();
        let test_handler = Box::new(TestHandler::new(EventHandled::Handled));

        let id = input_handler.register_handler(test_handler);
        input_handler.set_focus(Some(id));

        assert_eq!(input_handler.focus_handler, Some(id));
    }
}
