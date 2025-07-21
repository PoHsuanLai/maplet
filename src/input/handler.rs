use crate::{
    core::geo::{LatLng, LatLngBounds, Point},
    input::events::{InputEvent, MapEvent},
    prelude::{Duration, HashMap, Instant, VecDeque},
    traits::Lerp,
    Result,
};

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
    /// Pan with inertia (momentum-based panning)
    PanInertia {
        offset: Point,
        duration: Duration,
        ease_linearity: f64,
    },
    /// Start dragging mode (DOM-based dragging like Leaflet)
    StartDrag,
    /// End dragging mode and commit center changes
    EndDrag,
}

/// Active animation state
#[derive(Debug, Clone)]
pub struct Animation {
    pub action: Action,
    pub start_time: Instant,
    pub initial_center: LatLng,
    pub initial_zoom: f64,
}

impl Animation {
    fn new(action: Action, current_center: LatLng, current_zoom: f64) -> Self {
        Self {
            action,
            start_time: Instant::now(),
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
            Action::StartDrag | Action::EndDrag => return None, // No duration for drag actions
        };

        if elapsed >= duration {
            return None; // Animation finished
        }

        // Use the fixed ease-out cubic function from the animation module
        let progress = crate::layers::animation::ease_out_cubic(
            elapsed.as_secs_f64() / duration.as_secs_f64(),
        );

        match &self.action {
            Action::SetView { center, zoom, .. } => {
                let current_center = self.initial_center.lerp(center, progress);
                let current_zoom = self.initial_zoom.lerp(zoom, progress);
                Some((current_center, current_zoom))
            }
            Action::Zoom { level, .. } => {
                let current_zoom = self.initial_zoom.lerp(level, progress);
                Some((self.initial_center, current_zoom))
            }
            Action::Pan { .. } | Action::PanInertia { .. } => {
                // Pan animations are handled differently - they modify the target directly
                Some((self.initial_center, self.initial_zoom))
            }
            Action::StartDrag | Action::EndDrag => {
                // Drag actions don't have animations
                None
            }
        }
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
                // During drag, only update the map pane position (visual offset)
                // Do NOT update the viewport center here
                viewport.raw_pan_by(delta);
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
            Action::StartDrag => {
                viewport.start_drag();
            }
            Action::EndDrag => {
                // On drag end, update the center based on the final map pane position
                viewport.end_drag();
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
            InputEvent::Click {
                position,
                button: _,
            } => {
                // Simple click - emit click event
                self.event_manager.emit(MapEvent::Click {
                    lat_lng: LatLng::new(0.0, 0.0), // Placeholder - map will convert
                    pixel: position,
                });
            }
            InputEvent::MouseMove { position } => {
                // Emit mouse move event
                self.event_manager.emit(MapEvent::MouseMove {
                    lat_lng: LatLng::new(0.0, 0.0), // Placeholder - map will convert
                    pixel: position,
                });
            }
            InputEvent::DragStart { position: _ } => {
                // Drag start from egui - use built-in detection completely
                if self.pan_on_drag {
                    actions.push(Action::StartDrag);
                    self.event_manager.emit(MapEvent::MoveStart {
                        center: current_center,
                    });
                }
            }
            InputEvent::Drag { delta } => {
                // Drag in progress from egui - just use the delta directly
                if self.pan_on_drag {
                    actions.push(Action::Pan {
                        delta,
                        animate: false,
                        duration: Duration::from_millis(0),
                    });
                }
            }
            InputEvent::DragEnd => {
                // Drag end from egui - clean up
                if self.pan_on_drag {
                    actions.push(Action::EndDrag);
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
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
