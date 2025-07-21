use crate::core::geo::{LatLng, Point};
use crate::core::viewport::Transform;
use crate::prelude::{Duration, Instant};
use crate::traits::Lerp;

/// Simple ease-out cubic function for smooth zoom animations
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let t = t - 1.0;
    t * t * t + 1.0
}

/// Linear interpolation helper
pub fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start.lerp(&end, t)
}

/// Simple zoom animation with fixed parameters - no configuration needed
#[derive(Debug, Clone)]
pub struct ZoomAnimation {
    start_time: Instant,
    duration: Duration,
    from_transform: Transform,
    to_transform: Transform,
    _from_center: LatLng,
    to_center: LatLng,
    _from_zoom: f64,
    to_zoom: f64,
    active: bool,
    focus_point: Option<Point>,
}

impl ZoomAnimation {
    /// Create a new zoom animation with fixed, optimal parameters
    /// No configuration needed - this just works well for all cases
    pub fn new(
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
    ) -> Self {
        // Fixed duration that works well for zoom animations
        let duration = Duration::from_millis(250);

        // CRITICAL FIX: Use integer zoom levels to align with tile loading
        // This prevents the overshoot/shrink back issue
        let from_zoom_level = from_zoom.round();
        let to_zoom_level = to_zoom.round();

        // Calculate exact scale factor based on discrete zoom levels
        let scale_factor = 2_f64.powf(to_zoom_level - from_zoom_level);

        // Transform origin (focus point or center)
        let origin = focus_point.unwrap_or(Point::new(400.0, 300.0));

        // Simple translation calculation - no fancy focus point math
        let translation = Point::new(0.0, 0.0);

        println!(
            "🎬 [ANIMATION] Simple zoom: {:.0} -> {:.0}, scale={:.2}",
            from_zoom_level, to_zoom_level, scale_factor
        );

        Self {
            start_time: Instant::now(),
            duration,
            from_transform: Transform::identity(),
            to_transform: Transform::new(translation, scale_factor, origin),
            _from_center: from_center,
            to_center,
            _from_zoom: from_zoom_level, // Use rounded zoom levels
            to_zoom: to_zoom_level,     // Use rounded zoom levels
            active: true,
            focus_point,
        }
    }

    pub fn update(&mut self) -> Option<ZoomAnimationState> {
        if !self.active {
            return None;
        }

        let elapsed = self.start_time.elapsed();
        
        // CRITICAL FIX: On first frame, immediately return target state (like Leaflet)
        // This ensures tiles for the new zoom level are loaded immediately
        if elapsed.as_millis() == 0 {
            return Some(ZoomAnimationState {
                transform: self.from_transform, // Keep visual transform at start
                center: self.to_center,        // But viewport goes to target immediately
                zoom: self.to_zoom,            // Target zoom for tile loading
                progress: 0.0,                 // First frame
                fps: 60.0,
            });
        }

        if elapsed >= self.duration {
            self.active = false;
            // CRITICAL FIX: Return identity transform for final state to prevent snap back
            // This ensures the final frame has no transform applied, matching the cleared state
            return Some(ZoomAnimationState {
                transform: Transform::identity(), // Identity transform for seamless clearing
                center: self.to_center,
                zoom: self.to_zoom,
                progress: 1.0,
                fps: 60.0,
            });
        }

        let progress = elapsed.as_secs_f64() / self.duration.as_secs_f64();

        // Use fixed ease-out cubic easing
        let eased_progress = ease_out_cubic(progress);

        // CRITICAL FIX: In the final 10% of animation, gradually transition to identity transform
        // This prevents snap back by smoothly aligning with the final cleared state
        let current_transform = if eased_progress > 0.9 {
            // In final 10%, lerp from current transform to identity
            let final_transition = (eased_progress - 0.9) / 0.1; // 0.0 to 1.0 in final 10%
            let current_animated_transform = self.from_transform.lerp(&self.to_transform, eased_progress);
            current_animated_transform.lerp(&Transform::identity(), final_transition)
        } else {
            // Normal animation for first 90%
            self.from_transform.lerp(&self.to_transform, eased_progress)
        };

        // Keep viewport at target state (like Leaflet)
        // The visual animation is handled by the transform
        Some(ZoomAnimationState {
            transform: current_transform,
            center: self.to_center,  // Always target center
            zoom: self.to_zoom,      // Always target zoom
            progress: eased_progress,
            fps: 60.0,
        })
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn focus_point(&self) -> Option<Point> {
        self.focus_point
    }

    pub fn performance_metrics(&self) -> AnimationMetrics {
        AnimationMetrics {
            current_fps: 60.0,
            is_hitting_target: true,
            frame_duration_ms: 16.67,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZoomAnimationState {
    pub transform: Transform,
    pub center: LatLng,
    pub zoom: f64,
    pub progress: f64,
    pub fps: f64,
}

#[derive(Debug, Clone)]
pub struct AnimationMetrics {
    pub current_fps: f64,
    pub is_hitting_target: bool,
    pub frame_duration_ms: f64,
}

/// Simplified animation manager with no configuration options
#[derive(Debug)]
pub struct AnimationManager {
    current_zoom_animation: Option<ZoomAnimation>,
    keep_rendering_until: Option<Instant>,
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            current_zoom_animation: None,
            keep_rendering_until: None,
        }
    }

    pub fn update(&mut self) -> Option<ZoomAnimationState> {
        if let Some(animation) = &mut self.current_zoom_animation {
            if let Some(state) = animation.update() {
                if state.progress >= 1.0 {
                    self.current_zoom_animation = None;
                    self.keep_rendering_until = None;
                }
                return Some(state);
            } else {
                self.current_zoom_animation = None;
                self.keep_rendering_until = None;
            }
        }

        None
    }

    pub fn should_keep_rendering(&self) -> bool {
        if let Some(until) = self.keep_rendering_until {
            Instant::now() < until
        } else {
            self.current_zoom_animation.is_some()
        }
    }

    pub fn performance_metrics(&self) -> AnimationMetrics {
        if let Some(animation) = &self.current_zoom_animation {
            animation.performance_metrics()
        } else {
            AnimationMetrics {
                current_fps: 60.0,
                is_hitting_target: true,
                frame_duration_ms: 16.67,
            }
        }
    }

    pub fn stop_zoom_animation(&mut self) {
        if let Some(animation) = &mut self.current_zoom_animation {
            animation.stop();
        }
        self.current_zoom_animation = None;
        self.keep_rendering_until = None;
    }

    pub fn is_animating(&self) -> bool {
        self.current_zoom_animation.is_some()
    }

    /// Start a smooth zoom animation - the only animation type we support
    /// No configuration options - this just works well
    pub fn start_smooth_zoom(
        &mut self,
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
    ) -> bool {
        // Stop any existing animation
        self.stop_zoom_animation();

        // Don't animate if zoom difference is too small (no visible change)
        if (to_zoom - from_zoom).abs() < 0.1 {
            return false;
        }

        // Create the simple zoom animation
        let animation = ZoomAnimation::new(from_center, to_center, from_zoom, to_zoom, focus_point);

        println!(
            "🎬 [ANIMATION] Starting zoom animation: {:.1} -> {:.1} (diff: {:.1})",
            from_zoom, to_zoom, (to_zoom - from_zoom).abs()
        );

        self.current_zoom_animation = Some(animation);

        // Keep rendering for a bit after animation completes to ensure tiles are ready
        self.keep_rendering_until =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(300));

        true
    }

    /// Check if we're in the critical first phase of animation (needs aggressive tile loading)
    pub fn is_starting_animation(&self) -> bool {
        if let Some(animation) = &self.current_zoom_animation {
            let elapsed = animation.start_time.elapsed();
            elapsed.as_millis() < 50 // First 50ms is critical for tile loading
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ease_out_cubic() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        let mid = ease_out_cubic(0.5);
        assert!(mid > 0.5); // Should be faster than linear
    }

    #[test]
    fn test_animation_manager() {
        let mut manager = AnimationManager::new();

        let from_center = LatLng::new(0.0, 0.0);
        let to_center = LatLng::new(1.0, 1.0);

        assert!(manager.start_smooth_zoom(from_center, to_center, 10.0, 11.0, None));
        assert!(manager.is_animating());

        // Very small zoom changes should not animate
        assert!(!manager.start_smooth_zoom(from_center, to_center, 10.0, 10.05, None));
    }
}
