use crate::{
    core::geo::Point,
    input::events::{InputEvent, TouchEventType, TouchPoint},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Different types of gestures that can be recognized
#[derive(Debug, Clone, PartialEq)]
pub enum GestureType {
    /// Single finger drag/pan
    Drag {
        start_position: Point,
        current_position: Point,
        delta: Point,
    },
    /// Pinch to zoom with two fingers
    Pinch {
        center: Point,
        scale: f64,
        initial_distance: f64,
        current_distance: f64,
    },
    /// Tap gesture (short touch)
    Tap { position: Point, duration: Duration },
    /// Double tap gesture
    DoubleTap { position: Point },
    /// Long press gesture
    LongPress { position: Point, duration: Duration },
    /// Swipe gesture
    Swipe {
        start_position: Point,
        end_position: Point,
        direction: SwipeDirection,
        velocity: f64,
    },
    /// Two-finger rotation
    Rotation {
        center: Point,
        angle: f64,
        initial_angle: f64,
    },
}

/// Swipe directions
#[derive(Debug, Clone, PartialEq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// State of a gesture in progress
#[derive(Debug, Clone)]
pub struct GestureState {
    pub gesture_type: GestureType,
    pub start_time: Instant,
    pub is_active: bool,
}

/// Configuration for gesture recognition
#[derive(Debug, Clone)]
pub struct GestureConfig {
    /// Minimum distance for drag to start
    pub drag_threshold: f64,
    /// Maximum time for tap gesture
    pub tap_timeout: Duration,
    /// Maximum time between double taps
    pub double_tap_timeout: Duration,
    /// Minimum time for long press
    pub long_press_timeout: Duration,
    /// Minimum velocity for swipe detection
    pub swipe_velocity_threshold: f64,
    /// Maximum distance between taps for double tap
    pub double_tap_distance_threshold: f64,
    /// Minimum distance change for pinch gesture
    pub pinch_threshold: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            drag_threshold: 10.0,
            tap_timeout: Duration::from_millis(200),
            double_tap_timeout: Duration::from_millis(300),
            long_press_timeout: Duration::from_millis(500),
            swipe_velocity_threshold: 100.0,
            double_tap_distance_threshold: 50.0,
            pinch_threshold: 10.0,
        }
    }
}

/// Touch tracking information
#[derive(Debug, Clone)]
struct TouchInfo {
    id: u64,
    start_position: Point,
    current_position: Point,
    start_time: Instant,
    last_update: Instant,
}

/// Gesture recognizer that processes input events and detects gestures
pub struct GestureRecognizer {
    pub enabled: bool,
    config: GestureConfig,
    active_touches: HashMap<u64, TouchInfo>,
    current_gesture: Option<GestureState>,
    last_tap: Option<(Point, Instant)>,
    mouse_state: MouseState,
}

#[derive(Debug, Clone)]
#[derive(Default)]
struct MouseState {
    is_pressed: bool,
    start_position: Option<Point>,
    current_position: Option<Point>,
    start_time: Option<Instant>,
}


impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            enabled: true,
            config: GestureConfig::default(),
            active_touches: HashMap::new(),
            current_gesture: None,
            last_tap: None,
            mouse_state: MouseState::default(),
        }
    }

    pub fn with_config(config: GestureConfig) -> Self {
        Self {
            enabled: true,
            config,
            active_touches: HashMap::new(),
            current_gesture: None,
            last_tap: None,
            mouse_state: MouseState::default(),
        }
    }

    /// Processes input events and returns recognized gestures
    pub fn process_input(&mut self, input: InputEvent) -> Vec<InputEvent> {
        if !self.enabled {
            return vec![input];
        }

        let mut output_events = Vec::new();
        let now = Instant::now();

        match input {
            InputEvent::Touch {
                event_type,
                touches,
            } => {
                self.process_touch_event(event_type, touches, now, &mut output_events);
            }
            InputEvent::Click { position } => {
                self.process_mouse_down(position, now, &mut output_events);
            }
            InputEvent::MouseMove { position } => {
                self.process_mouse_move(position, now, &mut output_events);
            }
            InputEvent::DragStart { position } => {
                self.process_mouse_down(position, now, &mut output_events);
            }
            InputEvent::Drag { delta } => {
                if let Some(current_pos) = self.mouse_state.current_position {
                    let new_pos = current_pos.add(&delta);
                    self.process_mouse_move(new_pos, now, &mut output_events);
                }
            }
            InputEvent::DragEnd => {
                self.process_mouse_up(now, &mut output_events);
            }
            _ => {
                output_events.push(input);
            }
        }

        output_events
    }

    fn process_touch_event(
        &mut self,
        event_type: TouchEventType,
        touches: Vec<TouchPoint>,
        now: Instant,
        output_events: &mut Vec<InputEvent>,
    ) {
        match event_type {
            TouchEventType::Start => {
                for touch in touches {
                    self.active_touches.insert(
                        touch.id,
                        TouchInfo {
                            id: touch.id,
                            start_position: touch.position,
                            current_position: touch.position,
                            start_time: now,
                            last_update: now,
                        },
                    );
                }
                self.update_touch_gestures(now, output_events);
            }
            TouchEventType::Move => {
                for touch in touches {
                    if let Some(touch_info) = self.active_touches.get_mut(&touch.id) {
                        touch_info.current_position = touch.position;
                        touch_info.last_update = now;
                    }
                }
                self.update_touch_gestures(now, output_events);
            }
            TouchEventType::End | TouchEventType::Cancel => {
                for touch in touches {
                    self.active_touches.remove(&touch.id);
                }
                self.finalize_touch_gestures(now, output_events);
            }
        }
    }

    fn process_mouse_down(
        &mut self,
        position: Point,
        now: Instant,
        output_events: &mut Vec<InputEvent>,
    ) {
        self.mouse_state = MouseState {
            is_pressed: true,
            start_position: Some(position),
            current_position: Some(position),
            start_time: Some(now),
        };

        // Check for double tap
        if let Some((last_pos, last_time)) = self.last_tap {
            let time_diff = now.duration_since(last_time);
            let distance = position.distance_to(&last_pos);

            if time_diff <= self.config.double_tap_timeout
                && distance <= self.config.double_tap_distance_threshold
            {
                output_events.push(InputEvent::DoubleClick { position });
                self.last_tap = None;
                return;
            }
        }

        // Start potential tap gesture
        self.last_tap = Some((position, now));
    }

    fn process_mouse_move(
        &mut self,
        position: Point,
        now: Instant,
        output_events: &mut Vec<InputEvent>,
    ) {
        if !self.mouse_state.is_pressed {
            output_events.push(InputEvent::MouseMove { position });
            return;
        }

        self.mouse_state.current_position = Some(position);

        if let Some(start_pos) = self.mouse_state.start_position {
            let distance = position.distance_to(&start_pos);

            if distance > self.config.drag_threshold {
                // Start or continue drag
                if self.current_gesture.is_none() {
                    output_events.push(InputEvent::DragStart {
                        position: start_pos,
                    });
                }

                let delta = position.subtract(&start_pos);
                output_events.push(InputEvent::Drag { delta });

                self.current_gesture = Some(GestureState {
                    gesture_type: GestureType::Drag {
                        start_position: start_pos,
                        current_position: position,
                        delta,
                    },
                    start_time: self.mouse_state.start_time.unwrap_or(now),
                    is_active: true,
                });
            }
        }
    }

    fn process_mouse_up(&mut self, now: Instant, output_events: &mut Vec<InputEvent>) {
        if let (Some(start_pos), Some(start_time)) =
            (self.mouse_state.start_position, self.mouse_state.start_time)
        {
            let duration = now.duration_since(start_time);

            if let Some(gesture) = &self.current_gesture {
                if let GestureType::Drag { .. } = &gesture.gesture_type {
                    output_events.push(InputEvent::DragEnd);
                }
            } else {
                // Check for tap or long press
                if duration <= self.config.tap_timeout {
                    output_events.push(InputEvent::Click {
                        position: start_pos,
                    });
                } else if duration >= self.config.long_press_timeout {
                    // This would be a long press, but we don't have that event type in InputEvent
                    output_events.push(InputEvent::Click {
                        position: start_pos,
                    });
                }
            }
        }

        self.mouse_state = MouseState::default();
        self.current_gesture = None;
    }

    fn update_touch_gestures(&mut self, now: Instant, output_events: &mut Vec<InputEvent>) {
        match self.active_touches.len() {
            1 => self.process_single_touch_gesture(now, output_events),
            2 => self.process_two_finger_gestures(now, output_events),
            _ => {} // Ignore 3+ finger gestures for now
        }
    }

    fn process_single_touch_gesture(&mut self, _now: Instant, output_events: &mut Vec<InputEvent>) {
        if let Some(touch) = self.active_touches.values().next() {
            let distance = touch.current_position.distance_to(&touch.start_position);

            if distance > self.config.drag_threshold {
                let delta = touch.current_position.subtract(&touch.start_position);

                if self.current_gesture.is_none() {
                    output_events.push(InputEvent::DragStart {
                        position: touch.start_position,
                    });
                }

                output_events.push(InputEvent::Drag { delta });
            }
        }
    }

    fn process_two_finger_gestures(&mut self, _now: Instant, output_events: &mut Vec<InputEvent>) {
        let touches: Vec<&TouchInfo> = self.active_touches.values().collect();
        if touches.len() != 2 {
            return;
        }

        let touch1 = touches[0];
        let touch2 = touches[1];

        // Calculate current distance and center
        let current_distance = touch1
            .current_position
            .distance_to(&touch2.current_position);
        let initial_distance = touch1.start_position.distance_to(&touch2.start_position);

        let center = Point::new(
            (touch1.current_position.x + touch2.current_position.x) / 2.0,
            (touch1.current_position.y + touch2.current_position.y) / 2.0,
        );

        // Check for pinch gesture
        let distance_change = (current_distance - initial_distance).abs();
        if distance_change > self.config.pinch_threshold {
            let scale = current_distance / initial_distance;

            // Convert pinch to scroll event for zoom
            let zoom_delta = if scale > 1.0 { 1.0 } else { -1.0 };
            output_events.push(InputEvent::Scroll {
                delta: zoom_delta,
                position: center,
            });
        }
    }

    fn finalize_touch_gestures(&mut self, now: Instant, output_events: &mut Vec<InputEvent>) {
        if self.active_touches.is_empty() {
            if let Some(gesture) = &self.current_gesture {
                if let GestureType::Drag { .. } = &gesture.gesture_type {
                    output_events.push(InputEvent::DragEnd);
                }
            }
            self.current_gesture = None;
        }

        // Check for tap gestures on single touch end
        if self.active_touches.len() == 1 {
            if let Some(touch) = self.active_touches.values().next() {
                let duration = now.duration_since(touch.start_time);
                let distance = touch.current_position.distance_to(&touch.start_position);

                if distance <= self.config.drag_threshold && duration <= self.config.tap_timeout {
                    // Check for double tap
                    if let Some((last_pos, last_time)) = self.last_tap {
                        let time_diff = now.duration_since(last_time);
                        let tap_distance = touch.current_position.distance_to(&last_pos);

                        if time_diff <= self.config.double_tap_timeout
                            && tap_distance <= self.config.double_tap_distance_threshold
                        {
                            output_events.push(InputEvent::DoubleClick {
                                position: touch.current_position,
                            });
                            self.last_tap = None;
                            return;
                        }
                    }

                    output_events.push(InputEvent::Click {
                        position: touch.current_position,
                    });
                    self.last_tap = Some((touch.current_position, now));
                }
            }
        }
    }

    /// Sets the gesture configuration
    pub fn set_config(&mut self, config: GestureConfig) {
        self.config = config;
    }

    /// Gets the current active gesture, if any
    pub fn current_gesture(&self) -> Option<&GestureState> {
        self.current_gesture.as_ref()
    }

    /// Resets all gesture state
    pub fn reset(&mut self) {
        self.active_touches.clear();
        self.current_gesture = None;
        self.last_tap = None;
        self.mouse_state = MouseState::default();
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gesture_recognizer_creation() {
        let recognizer = GestureRecognizer::new();
        assert!(recognizer.enabled);
        assert!(recognizer.active_touches.is_empty());
    }

    #[test]
    fn test_click_event_passthrough() {
        let mut recognizer = GestureRecognizer::new();
        let move_event = InputEvent::MouseMove {
            position: Point::new(100.0, 100.0),
        };

        let result = recognizer.process_input(move_event);
        // Mouse move events should pass through
        assert!(!result.is_empty());
        assert!(matches!(result[0], InputEvent::MouseMove { .. }));
    }

    #[test]
    fn test_drag_threshold() {
        let mut recognizer = GestureRecognizer::new();

        // Start drag
        let drag_start = InputEvent::DragStart {
            position: Point::new(0.0, 0.0),
        };
        recognizer.process_input(drag_start);

        // Small movement (under threshold)
        let small_drag = InputEvent::Drag {
            delta: Point::new(5.0, 5.0),
        };
        let result = recognizer.process_input(small_drag);

        // Should not trigger drag gesture yet
        assert!(result
            .iter()
            .all(|e| !matches!(e, InputEvent::DragStart { .. })));
    }

    #[test]
    fn test_gesture_config() {
        let config = GestureConfig {
            drag_threshold: 20.0,
            ..Default::default()
        };

        let recognizer = GestureRecognizer::with_config(config.clone());
        assert_eq!(recognizer.config.drag_threshold, 20.0);
    }
}
