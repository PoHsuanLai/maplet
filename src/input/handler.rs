use crate::{
    core::geo::{LatLng, LatLngBounds, Point},
    input::events::{InputEvent, MapEvent},
    Result,
};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Unified action that combines input response and animation
#[derive(Debug, Clone)]
pub enum Action {
    /// Pan with optional animation
    Pan {
        delta: Point,
        animate: bool,
        duration: Duration,
    },
    /// Zoom with optional animation
    Zoom {
        level: f64,
        focus_point: Option<Point>,
        animate: bool,
        duration: Duration,
    },
    /// Set view with optional animation
    SetView {
        center: LatLng,
        zoom: f64,
        animate: bool,
        duration: Duration,
    },
    /// Pan with inertia (like Leaflet's inertia system)
    PanInertia {
        offset: Point,
        duration: Duration,
        ease_linearity: f64,
    },
}

/// Easing functions for animations
#[derive(Debug, Clone, Copy)]
pub enum Easing {
    Linear,
    EaseOut,
    EaseInOut,
}

impl Easing {
    fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
        }
    }
}

/// Active animation state
#[derive(Debug, Clone)]
pub struct Animation {
    pub action: Action,
    pub start_time: Instant,
    pub easing: Easing,
    pub initial_center: LatLng,
    pub initial_zoom: f64,
}

impl Animation {
    fn new(action: Action, current_center: LatLng, current_zoom: f64) -> Self {
        Self {
            action,
            start_time: Instant::now(),
            easing: Easing::EaseOut,
            initial_center: current_center,
            initial_zoom: current_zoom,
        }
    }

    /// Update animation and return current state, or None if finished
    fn update(&self) -> Option<(LatLng, f64)> {
        let elapsed = self.start_time.elapsed();
        let duration = match &self.action {
            Action::Pan { duration, .. } => *duration,
            Action::Zoom { duration, .. } => *duration,
            Action::SetView { duration, .. } => *duration,
            Action::PanInertia { duration, .. } => *duration,
        };

        if elapsed >= duration {
            return None; // Animation finished
        }

        let progress = self
            .easing
            .apply(elapsed.as_secs_f64() / duration.as_secs_f64());

        match &self.action {
            Action::SetView { center, zoom, .. } => {
                let current_center = LatLng::new(
                    self.initial_center.lat + (center.lat - self.initial_center.lat) * progress,
                    self.initial_center.lng + (center.lng - self.initial_center.lng) * progress,
                );
                let current_zoom = self.initial_zoom + (zoom - self.initial_zoom) * progress;
                Some((current_center, current_zoom))
            }
            Action::Zoom { level, .. } => {
                let current_zoom = self.initial_zoom + (level - self.initial_zoom) * progress;
                Some((self.initial_center, current_zoom))
            }
            Action::Pan { .. } | Action::PanInertia { .. } => {
                // Pan animations are handled differently - they modify the target directly
                Some((self.initial_center, self.initial_zoom))
            }
        }
    }
}

/// Drag state tracking (like Leaflet's Draggable)
#[derive(Debug, Clone)]
struct DragState {
    /// Whether dragging is currently active
    active: bool,
    /// Initial mouse position when drag started
    start_point: Option<Point>,
    /// Last mouse position
    last_point: Option<Point>,
    /// Positions for inertia calculation (like Leaflet's position tracking)
    positions: VecDeque<(Point, Instant)>,
    /// Times for inertia calculation
    times: VecDeque<Instant>,
    /// Whether the drag has moved (for distinguishing from clicks)
    moved: bool,
    /// Click tolerance in pixels
    click_tolerance: f64,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            active: false,
            start_point: None,
            last_point: None,
            positions: VecDeque::new(),
            times: VecDeque::new(),
            moved: false,
            click_tolerance: 3.0, // Like Leaflet's default
        }
    }
}

impl DragState {
    /// Start a new drag operation
    fn start(&mut self, point: Point) {
        self.active = true;
        self.start_point = Some(point);
        self.last_point = Some(point);
        self.moved = false;
        self.positions.clear();
        self.times.clear();
        self.positions.push_back((point, Instant::now()));
        self.times.push_back(Instant::now());
    }

    /// Update drag position and track for inertia
    fn update(&mut self, point: Point) -> Option<Point> {
        if !self.active {
            return None;
        }

        let start = self.start_point?;
        let last = self.last_point?;

        // Check if we've moved beyond click tolerance
        if !self.moved {
            let delta = point.subtract(&start);
            if delta.x.abs() + delta.y.abs() >= self.click_tolerance {
                self.moved = true;
            }
        }

        if self.moved {
            let delta = point.subtract(&last);
            self.last_point = Some(point);

            // Track positions for inertia (like Leaflet's position tracking)
            let now = Instant::now();
            self.positions.push_back((point, now));
            self.times.push_back(now);

            // Prune old positions (keep only last 50ms like Leaflet)
            self.prune_positions(now);

            Some(delta)
        } else {
            None
        }
    }

    /// Prune old positions for inertia calculation
    fn prune_positions(&mut self, time: Instant) {
        while let Some(&front_time) = self.times.front() {
            if time.duration_since(front_time).as_millis() > 50 {
                self.positions.pop_front();
                self.times.pop_front();
            } else {
                break;
            }
        }
    }

    /// End drag and calculate inertia
    fn end(&mut self) -> Option<Point> {
        if !self.active || !self.moved {
            self.active = false;
            return None;
        }

        self.active = false;

        // Calculate inertia like Leaflet's _onDragEnd
        if self.positions.len() < 2 {
            return None;
        }

        let last_pos = self.positions.back()?.0;
        let first_pos = self.positions.front()?.0;
        let last_time = *self.times.back()?;
        let first_time = *self.times.front()?;

        let direction = last_pos.subtract(&first_pos);
        let duration = last_time.duration_since(first_time).as_secs_f64();

        if duration <= 0.0 {
            return None;
        }

        // Calculate inertia offset like Leaflet
        let ease_linearity = 0.2; // Like Leaflet's default
        let speed_vector = direction.scale(ease_linearity / duration);
        let speed = (speed_vector.x * speed_vector.x + speed_vector.y * speed_vector.y).sqrt();

        let max_speed = 1500.0; // Like Leaflet's default inertiaMaxSpeed
        let limited_speed = speed.min(max_speed);

        if limited_speed < 10.0 {
            return None; // Too slow for inertia
        }

        let limited_speed_vector = speed_vector.scale(limited_speed / speed);
        let deceleration = 3400.0; // Like Leaflet's default inertiaDeceleration
        let deceleration_duration = limited_speed / (deceleration * ease_linearity);
        let offset = limited_speed_vector.scale(-deceleration_duration / 2.0);

        Some(offset)
    }

    /// Check if drag is active
    fn is_active(&self) -> bool {
        self.active
    }

    /// Check if drag has moved
    fn has_moved(&self) -> bool {
        self.moved
    }
}

/// Event listener callback type
pub type EventCallback = Box<dyn Fn(&MapEvent) + Send + Sync>;

/// Event management system for the map
#[derive(Default)]
pub struct EventManager {
    /// Event listeners by event type
    listeners: HashMap<String, Vec<EventCallback>>,
    /// Event queue for processing
    event_queue: VecDeque<MapEvent>,
}

impl EventManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an event listener
    pub fn on<F>(&mut self, event_type: &str, callback: F)
    where
        F: Fn(&MapEvent) + Send + Sync + 'static,
    {
        self.listeners
            .entry(event_type.to_string())
            .or_default()
            .push(Box::new(callback));
    }

    /// Emit an event to the queue
    pub fn emit(&mut self, event: MapEvent) {
        self.event_queue.push_back(event);
    }

    /// Process all queued events
    pub fn process_events(&mut self) -> Vec<MapEvent> {
        let events: Vec<_> = self.event_queue.drain(..).collect();

        for event in &events {
            let event_type = match event {
                MapEvent::ViewChanged { .. } => "viewchanged",
                MapEvent::Click { .. } => "click",
                MapEvent::MouseMove { .. } => "mousemove",
                MapEvent::ZoomStart { .. } => "zoomstart",
                MapEvent::ZoomEnd { .. } => "zoomend",
                MapEvent::MoveStart { .. } => "movestart",
                MapEvent::MoveEnd { .. } => "moveend",
                MapEvent::LayerAdd { .. } => "layeradd",
                MapEvent::LayerRemove { .. } => "layerremove",
                MapEvent::BaseLayerChange { .. } => "baselayerchange",
                MapEvent::OverlayAdd { .. } => "overlayadd",
                MapEvent::OverlayRemove { .. } => "overlayremove",
            };

            if let Some(callbacks) = self.listeners.get(event_type) {
                for callback in callbacks {
                    callback(event);
                }
            }
        }

        events
    }

    /// Clear all events from the queue
    pub fn clear_events(&mut self) {
        self.event_queue.clear();
    }

    /// Get number of pending events
    pub fn pending_events(&self) -> usize {
        self.event_queue.len()
    }
}

/// Map operation implementations
pub struct MapOperations;

impl MapOperations {
    /// Set the map view to a specific center and zoom
    pub fn set_view(
        viewport: &mut crate::core::viewport::Viewport,
        center: LatLng,
        zoom: f64,
    ) -> Result<()> {
        let old_center = viewport.center;
        let old_zoom = viewport.zoom;

        if old_center != center || old_zoom != zoom {
            viewport.set_center(center);
            viewport.set_zoom(zoom);
        }

        Ok(())
    }

    /// Pan the map by a pixel delta with bounds checking
    pub fn pan(viewport: &mut crate::core::viewport::Viewport, delta: Point) -> Result<Point> {
        let actual_delta = viewport.pan(delta);
        Ok(actual_delta)
    }

    /// Zoom to a specific level with optional focus point
    pub fn zoom_to(
        viewport: &mut crate::core::viewport::Viewport,
        zoom: f64,
        focus_point: Option<Point>,
    ) -> Result<()> {
        viewport.zoom_to(zoom, focus_point);
        Ok(())
    }

    /// Fit the map to specific bounds
    pub fn fit_bounds(
        viewport: &mut crate::core::viewport::Viewport,
        bounds: &LatLngBounds,
        padding: Option<f64>,
    ) -> Result<()> {
        viewport.fit_bounds(bounds, padding);
        Ok(())
    }

    /// Execute a pan inertia action
    pub fn pan_inertia(
        viewport: &mut crate::core::viewport::Viewport,
        offset: Point,
    ) -> Result<()> {
        viewport.pan(offset);
        Ok(())
    }

    /// Execute any action
    pub fn execute_action(
        viewport: &mut crate::core::viewport::Viewport,
        action: Action,
    ) -> Result<()> {
        match action {
            Action::Pan { delta, .. } => {
                Self::pan(viewport, delta)?;
            }
            Action::Zoom {
                level, focus_point, ..
            } => {
                Self::zoom_to(viewport, level, focus_point)?;
            }
            Action::SetView { center, zoom, .. } => {
                Self::set_view(viewport, center, zoom)?;
            }
            Action::PanInertia { offset, .. } => {
                Self::pan_inertia(viewport, offset)?;
            }
        }
        Ok(())
    }
}

/// Input handler that manages events and produces map actions
pub struct InputHandler {
    pub enabled: bool,
    event_queue: VecDeque<InputEvent>,
    current_animation: Option<Animation>,

    // Drag state management (like Leaflet's Draggable)
    drag_state: DragState,
    
    // Mouse state tracking for proper drag detection
    mouse_down: bool,
    mouse_down_position: Option<Point>,
    mouse_position: Option<Point>,
    drag_threshold: f64, // Minimum movement to start drag

    // Event management
    event_manager: EventManager,

    // Configuration (like Leaflet's Map options)
    pub zoom_on_wheel: bool,
    pub zoom_on_double_click: bool,
    pub pan_on_drag: bool,
    pub animate_zoom: bool,
    pub animate_pan: bool,
    pub zoom_duration: Duration,
    pub pan_duration: Duration,
    pub inertia: bool,
    pub inertia_deceleration: f64,
    pub inertia_max_speed: f64,
    pub ease_linearity: f64,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            enabled: true,
            event_queue: VecDeque::new(),
            current_animation: None,
            drag_state: DragState::default(),
            mouse_down: false,
            mouse_down_position: None,
            mouse_position: None,
            drag_threshold: 3.0, // 3 pixels like Leaflet's default
            event_manager: EventManager::new(),
            zoom_on_wheel: true,
            zoom_on_double_click: true,
            pan_on_drag: true,
            animate_zoom: true,
            animate_pan: true,
            zoom_duration: Duration::from_millis(250),
            pan_duration: Duration::from_millis(300),
            inertia: true,
            inertia_deceleration: 3400.0,
            inertia_max_speed: 1500.0,
            ease_linearity: 0.2,
        }
    }

    /// Handle input events and generate actions (like Leaflet's event handling)
    pub fn handle_event(
        &mut self,
        event: InputEvent,
        current_center: LatLng,
        current_zoom: f64,
    ) -> Vec<Action> {
        if !self.enabled {
            return vec![];
        }

        let mut actions = vec![];

        match event {
            InputEvent::Click { position, button: _ } => {
                // Handle mouse down for potential dragging
                self.handle_mouse_down(position);
                
                // Simple click - emit click event
                self.event_manager.emit(MapEvent::Click {
                    lat_lng: LatLng::new(0.0, 0.0), // Placeholder - map will convert
                    pixel: position,
                });
            }
            InputEvent::MouseMove { position } => {
                // Convert mouse movement to drag if conditions are met
                actions.extend(self.convert_mouse_move_to_drag(position, current_center));
            }
            InputEvent::DragStart { position } => {
                // Direct drag start (from external gesture recognizer)
                if self.pan_on_drag {
                    self.drag_state.start(position);
                    self.event_manager.emit(MapEvent::MoveStart {
                        center: current_center,
                    });
                }
            }
            InputEvent::Drag { delta } => {
                // Direct drag (from external gesture recognizer)
                if self.pan_on_drag && self.drag_state.is_active() {
                    actions.push(Action::Pan {
                        delta,
                        animate: false,
                        duration: Duration::from_millis(0),
                    });
                }
            }
            InputEvent::DragEnd => {
                // Direct drag end (from external gesture recognizer) 
                actions.extend(self.handle_mouse_up());
                
                if self.drag_state.has_moved() {
                    self.event_manager.emit(MapEvent::MoveEnd {
                        center: current_center,
                    });
                }
            }
            InputEvent::Scroll { delta, position } => {
                if self.zoom_on_wheel {
                    let zoom_delta = if delta > 0.0 { 1.0 } else { -1.0 };
                    let new_zoom = (current_zoom + zoom_delta).clamp(0.0, 18.0);

                    self.event_manager
                        .emit(MapEvent::ZoomStart { zoom: current_zoom });

                    actions.push(Action::Zoom {
                        level: new_zoom,
                        focus_point: Some(position),
                        animate: self.animate_zoom,
                        duration: self.zoom_duration,
                    });
                }
            }
            InputEvent::DoubleClick { position } => {
                if self.zoom_on_double_click {
                    let new_zoom = (current_zoom + 1.0).clamp(0.0, 18.0);

                    self.event_manager
                        .emit(MapEvent::ZoomStart { zoom: current_zoom });

                    actions.push(Action::Zoom {
                        level: new_zoom,
                        focus_point: Some(position),
                        animate: self.animate_zoom,
                        duration: self.zoom_duration,
                    });
                }
            }
            InputEvent::KeyPress { .. } => {
                // Handle keyboard events if needed
            }
            InputEvent::Resize { .. } => {
                // Handle viewport resize
            }
            InputEvent::Touch { .. } => {
                // Handle touch events if needed
            }
        }

        actions
    }

    /// Register an event listener
    pub fn on<F>(&mut self, event_type: &str, callback: F)
    where
        F: Fn(&MapEvent) + Send + Sync + 'static,
    {
        self.event_manager.on(event_type, callback);
    }

    /// Emit an event manually
    pub fn emit_event(&mut self, event: MapEvent) {
        self.event_manager.emit(event);
    }

    /// Process all queued events and return them
    pub fn process_events(&mut self) -> Vec<MapEvent> {
        self.event_manager.process_events()
    }

    /// Start a new animation
    pub fn start_animation(&mut self, action: Action, current_center: LatLng, current_zoom: f64) {
        self.current_animation = Some(Animation::new(action, current_center, current_zoom));
    }

    /// Update current animation and return the current state
    pub fn update_animation(&mut self) -> Option<(LatLng, f64)> {
        if let Some(ref mut animation) = self.current_animation {
            let result = animation.update();
            if result.is_none() {
                self.current_animation = None;
            }
            result
        } else {
            None
        }
    }

    /// Check if there's an active animation
    pub fn has_animation(&self) -> bool {
        self.current_animation.is_some()
    }

    /// Stop current animation
    pub fn stop_animation(&mut self) {
        self.current_animation = None;
    }

    /// Queue an event for later processing
    pub fn queue_event(&mut self, event: InputEvent) {
        self.event_queue.push_back(event);
    }

    /// Process all queued events and return resulting actions
    pub fn process_queued_events(
        &mut self,
        current_center: LatLng,
        current_zoom: f64,
    ) -> Vec<Action> {
        let mut actions = vec![];
        while let Some(event) = self.event_queue.pop_front() {
            actions.extend(self.handle_event(event, current_center, current_zoom));
        }
        actions
    }

    /// Clear the event queue
    pub fn clear_queue(&mut self) {
        self.event_queue.clear();
    }

    /// Handle mouse down for gesture recognition
    fn handle_mouse_down(&mut self, position: Point) {
        self.mouse_down = true;
        self.mouse_down_position = Some(position);
        self.mouse_position = Some(position);
    }

    /// Handle mouse up for gesture recognition
    fn handle_mouse_up(&mut self) -> Vec<Action> {
        let mut actions = vec![];
        
        if self.drag_state.is_active() {
            // End drag
            if let Some(inertia_offset) = self.drag_state.end() {
                if self.inertia && inertia_offset.x.abs() + inertia_offset.y.abs() > 1.0 {
                    let deceleration_duration = (inertia_offset.x * inertia_offset.x
                        + inertia_offset.y * inertia_offset.y)
                        .sqrt()
                        / (self.inertia_deceleration * self.ease_linearity);

                    actions.push(Action::PanInertia {
                        offset: inertia_offset,
                        duration: Duration::from_secs_f64(deceleration_duration),
                        ease_linearity: self.ease_linearity,
                    });
                }
            }
        }

        self.mouse_down = false;
        self.mouse_down_position = None;
        actions
    }

    /// Convert mouse movement to drag if conditions are met
    fn convert_mouse_move_to_drag(&mut self, position: Point, current_center: LatLng) -> Vec<Action> {
        let mut actions = vec![];

        if !self.mouse_down || !self.pan_on_drag {
            return actions;
        }

        let start_position = match self.mouse_down_position {
            Some(pos) => pos,
            None => return actions,
        };

        // Check if we've moved beyond drag threshold
        let distance = position.distance_to(&start_position);
        
        if !self.drag_state.is_active() && distance >= self.drag_threshold {
            // Start dragging
            self.drag_state.start(start_position);
            self.event_manager.emit(MapEvent::MoveStart {
                center: current_center,
            });
        }

        if self.drag_state.is_active() {
            // Continue dragging
            if let Some(delta) = self.drag_state.update(position) {
                actions.push(Action::Pan {
                    delta,
                    animate: false,
                    duration: Duration::from_millis(0),
                });
            }
        }

        self.mouse_position = Some(position);
        actions
    }

    /// Handle mouse up event (call this when mouse button is released)
    pub fn handle_mouse_release(&mut self, current_center: LatLng) -> Vec<Action> {
        let actions = self.handle_mouse_up();
        
        if self.drag_state.has_moved() {
            self.event_manager.emit(MapEvent::MoveEnd {
                center: current_center,
            });
        }
        
        actions
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
