use crate::animation::{
    interpolation::{EasingFunction, Interpolation},
    tweening::TweenManager,
};
use crate::core::{geo::LatLng, viewport::Viewport};
use instant::Instant;

/// Types of map transitions
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionType {
    /// Simple pan to a new center
    Pan { target_center: LatLng },
    /// Simple zoom to a new level
    Zoom {
        target_zoom: f64,
        focus_point: Option<crate::core::geo::Point>,
    },
    /// Combined pan and zoom
    SetView {
        target_center: LatLng,
        target_zoom: f64,
    },
    /// Smooth fly-to animation (like Google Maps)
    FlyTo {
        target_center: LatLng,
        target_zoom: f64,
        /// Maximum zoom level to reach during the flight
        max_zoom: Option<f64>,
    },
    /// Fit bounds with animation
    FitBounds {
        bounds: crate::core::geo::LatLngBounds,
        padding: f64,
    },
}

/// State of a transition
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionState {
    NotStarted,
    Running,
    Completed,
    Cancelled,
}

/// A viewport transition animation
pub struct Transition {
    /// Unique identifier
    pub id: String,
    /// Type of transition
    pub transition_type: TransitionType,
    /// Duration in seconds
    pub duration: f64,
    /// Easing function
    pub easing: EasingFunction,
    /// Current state
    pub state: TransitionState,
    /// Start time
    pub start_time: Option<Instant>,
    /// Elapsed time
    pub elapsed_time: f64,
    /// Initial viewport state
    pub start_viewport: Viewport,
    /// Target viewport state
    pub target_viewport: Viewport,
    /// Current viewport state (interpolated)
    pub current_viewport: Viewport,
    /// Callbacks
    pub on_update: Option<Box<dyn Fn(&Viewport) + Send + Sync>>,
    pub on_complete: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_start: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Transition {
    /// Create a new transition
    pub fn new(
        id: String,
        transition_type: TransitionType,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Self> {
        let target_viewport = Self::calculate_target_viewport(&transition_type, &current_viewport)?;

        Ok(Self {
            id,
            transition_type,
            duration,
            easing: EasingFunction::EaseInOutQuad,
            state: TransitionState::NotStarted,
            start_time: None,
            elapsed_time: 0.0,
            start_viewport: current_viewport.clone(),
            target_viewport,
            current_viewport,
            on_update: None,
            on_complete: None,
            on_start: None,
        })
    }

    /// Calculate the target viewport based on transition type
    fn calculate_target_viewport(
        transition_type: &TransitionType,
        current: &Viewport,
    ) -> crate::Result<Viewport> {
        let mut target = current.clone();

        match transition_type {
            TransitionType::Pan { target_center } => {
                target.set_center(*target_center);
            }
            TransitionType::Zoom { target_zoom, .. } => {
                target.set_zoom(*target_zoom);
            }
            TransitionType::SetView {
                target_center,
                target_zoom,
            } => {
                target.set_center(*target_center);
                target.set_zoom(*target_zoom);
            }
            TransitionType::FlyTo {
                target_center,
                target_zoom,
                ..
            } => {
                target.set_center(*target_center);
                target.set_zoom(*target_zoom);
            }
            TransitionType::FitBounds { bounds, padding } => {
                target.fit_bounds(bounds, Some(*padding));
            }
        }

        Ok(target)
    }

    /// Set the easing function
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Set update callback
    pub fn on_update<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Viewport) + Send + Sync + 'static,
    {
        self.on_update = Some(Box::new(callback));
        self
    }

    /// Set completion callback
    pub fn on_complete<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_complete = Some(Box::new(callback));
        self
    }

    /// Set start callback
    pub fn on_start<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_start = Some(Box::new(callback));
        self
    }

    /// Start the transition
    pub fn start(&mut self) {
        if self.state == TransitionState::NotStarted {
            self.start_time = Some(Instant::now());
            self.state = TransitionState::Running;
            self.elapsed_time = 0.0;

            if let Some(ref callback) = self.on_start {
                callback();
            }
        }
    }

    /// Stop the transition
    pub fn stop(&mut self) {
        self.state = TransitionState::Cancelled;
    }

    /// Update the transition
    pub fn update(&mut self, delta_time: f64) -> bool {
        match self.state {
            TransitionState::NotStarted => {
                self.start();
                false
            }
            TransitionState::Running => {
                self.elapsed_time += delta_time;
                let progress = (self.elapsed_time / self.duration).clamp(0.0, 1.0);

                // Apply easing
                let eased_progress = self.easing.apply(progress);

                // Update viewport based on transition type
                self.update_viewport(eased_progress);

                if let Some(ref callback) = self.on_update {
                    callback(&self.current_viewport);
                }

                // Check completion
                if progress >= 1.0 {
                    self.complete();
                }

                true
            }
            _ => false,
        }
    }

    /// Update the viewport based on progress and transition type
    fn update_viewport(&mut self, progress: f64) {
        match &self.transition_type {
            TransitionType::FlyTo {
                target_center,
                target_zoom,
                max_zoom,
            } => {
                self.update_flyto_viewport(progress, *target_center, *target_zoom, *max_zoom);
            }
            _ => {
                // Standard interpolation for other transition types
                self.current_viewport.center = Interpolation::lat_lng(
                    &self.start_viewport.center,
                    &self.target_viewport.center,
                    progress,
                );

                self.current_viewport.zoom = Interpolation::linear(
                    self.start_viewport.zoom,
                    self.target_viewport.zoom,
                    progress,
                );
            }
        }
    }

    /// Special handling for fly-to animation
    fn update_flyto_viewport(
        &mut self,
        progress: f64,
        target_center: LatLng,
        target_zoom: f64,
        max_zoom: Option<f64>,
    ) {
        // Fly-to uses a more complex interpolation that simulates flying up and then down
        let start_center = self.start_viewport.center;
        let start_zoom = self.start_viewport.zoom;

        // Calculate the maximum zoom during flight
        let flight_max_zoom = max_zoom.unwrap_or_else(|| {
            let distance = start_center.distance_to(&target_center);
            let zoom_for_distance = (-distance / 1000.0).log2() + 10.0; // Rough heuristic
            (start_zoom.min(target_zoom) - 2.0).max(zoom_for_distance)
        });

        // Use a parabolic arc for zoom during flight
        let zoom_progress = if progress < 0.5 {
            // Flying up (zooming out)
            let t = progress * 2.0;
            let zoom_out_amount = start_zoom - flight_max_zoom;
            start_zoom - zoom_out_amount * (2.0 * t - t * t)
        } else {
            // Flying down (zooming in)
            let t = (progress - 0.5) * 2.0;
            let zoom_in_amount = target_zoom - flight_max_zoom;
            flight_max_zoom + zoom_in_amount * (t * t)
        };

        // Use spherical interpolation for smooth path
        self.current_viewport.center =
            Interpolation::slerp_lat_lng(&start_center, &target_center, progress);

        self.current_viewport.zoom = zoom_progress;
    }

    /// Complete the transition
    fn complete(&mut self) {
        self.state = TransitionState::Completed;
        self.current_viewport = self.target_viewport.clone();

        if let Some(ref callback) = self.on_complete {
            callback();
        }
    }

    /// Check if the transition is finished
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state,
            TransitionState::Completed | TransitionState::Cancelled
        )
    }

    /// Get the current progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.duration == 0.0 {
            1.0
        } else {
            (self.elapsed_time / self.duration).clamp(0.0, 1.0)
        }
    }
}

/// Manager for viewport transitions
pub struct TransitionManager {
    /// Currently active transition
    current_transition: Option<Transition>,
    /// Tween manager for additional animations
    tween_manager: TweenManager,
    /// Queued transitions
    transition_queue: Vec<Transition>,
    /// Whether to interrupt current transition when starting a new one
    interrupt_on_new: bool,
}

impl TransitionManager {
    pub fn new() -> Self {
        Self {
            current_transition: None,
            tween_manager: TweenManager::new(),
            transition_queue: Vec::new(),
            interrupt_on_new: true,
        }
    }

    /// Start a new transition
    pub fn start_transition(&mut self, transition: Transition) -> crate::Result<()> {
        if self.interrupt_on_new || self.current_transition.is_none() {
            // Stop current transition if it exists
            if let Some(ref mut current) = self.current_transition {
                current.stop();
            }

            // Start new transition
            let mut new_transition = transition;
            new_transition.start();
            self.current_transition = Some(new_transition);
        } else {
            // Queue the transition
            self.transition_queue.push(transition);
        }

        Ok(())
    }

    /// Update all transitions and tweens
    pub fn update(&mut self, delta_time: f64) -> Option<Viewport> {
        let mut result_viewport = None;

        // Update current transition
        if let Some(ref mut transition) = self.current_transition {
            if transition.update(delta_time) {
                result_viewport = Some(transition.current_viewport.clone());
            }

            // Check if transition finished
            if transition.is_finished() {
                self.current_transition = None;

                // Start next queued transition
                if let Some(next_transition) = self.transition_queue.pop() {
                    let _ = self.start_transition(next_transition);
                }
            }
        }

        // Update tweens
        self.tween_manager.update();

        result_viewport
    }

    /// Stop current transition
    pub fn stop_current(&mut self) {
        if let Some(ref mut transition) = self.current_transition {
            transition.stop();
        }
    }

    /// Clear all queued transitions
    pub fn clear_queue(&mut self) {
        self.transition_queue.clear();
    }

    /// Stop all transitions and tweens
    pub fn stop_all(&mut self) {
        self.stop_current();
        self.clear_queue();
        self.tween_manager.stop_all();
    }

    /// Check if there's an active transition
    pub fn has_active_transition(&self) -> bool {
        self.current_transition
            .as_ref()
            .map(|t| !t.is_finished())
            .unwrap_or(false)
    }

    /// Get reference to current transition
    pub fn current_transition(&self) -> Option<&Transition> {
        self.current_transition.as_ref()
    }

    /// Get mutable reference to tween manager
    pub fn tween_manager_mut(&mut self) -> &mut TweenManager {
        &mut self.tween_manager
    }

    /// Set whether to interrupt current transitions
    pub fn set_interrupt_on_new(&mut self, interrupt: bool) {
        self.interrupt_on_new = interrupt;
    }
}

impl Default for TransitionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating common transitions
pub struct TransitionBuilder;

impl TransitionBuilder {
    /// Create a pan transition
    pub fn pan(
        id: String,
        target: LatLng,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Transition> {
        Transition::new(
            id,
            TransitionType::Pan {
                target_center: target,
            },
            current_viewport,
            duration,
        )
    }

    /// Create a zoom transition
    pub fn zoom(
        id: String,
        target_zoom: f64,
        focus_point: Option<crate::core::geo::Point>,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Transition> {
        Transition::new(
            id,
            TransitionType::Zoom {
                target_zoom,
                focus_point,
            },
            current_viewport,
            duration,
        )
    }

    /// Create a set view transition
    pub fn set_view(
        id: String,
        target_center: LatLng,
        target_zoom: f64,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Transition> {
        Transition::new(
            id,
            TransitionType::SetView {
                target_center,
                target_zoom,
            },
            current_viewport,
            duration,
        )
    }

    /// Create a fly-to transition
    pub fn fly_to(
        id: String,
        target_center: LatLng,
        target_zoom: f64,
        max_zoom: Option<f64>,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Transition> {
        Transition::new(
            id,
            TransitionType::FlyTo {
                target_center,
                target_zoom,
                max_zoom,
            },
            current_viewport,
            duration,
        )
    }

    /// Create a fit bounds transition
    pub fn fit_bounds(
        id: String,
        bounds: crate::core::geo::LatLngBounds,
        padding: f64,
        current_viewport: Viewport,
        duration: f64,
    ) -> crate::Result<Transition> {
        Transition::new(
            id,
            TransitionType::FitBounds { bounds, padding },
            current_viewport,
            duration,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geo::{LatLng, Point};

    #[test]
    fn test_pan_transition() {
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 10.0, Point::new(800.0, 600.0));
        let target = LatLng::new(10.0, 10.0);

        let transition =
            TransitionBuilder::pan("test_pan".to_string(), target, viewport, 1.0).unwrap();

        assert_eq!(transition.target_viewport.center, target);
    }

    #[test]
    fn test_transition_manager() {
        let mut manager = TransitionManager::new();
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 10.0, Point::new(800.0, 600.0));

        let transition =
            TransitionBuilder::pan("test".to_string(), LatLng::new(10.0, 10.0), viewport, 1.0)
                .unwrap();

        manager.start_transition(transition).unwrap();
        assert!(manager.has_active_transition());

        // Update should return a viewport
        let result = manager.update(0.5);
        assert!(result.is_some());
    }
}
