use crate::core::geo::{LatLng, Point};
use crate::core::viewport::Transform;
use std::time::{Duration, Instant};

/// Simple interpolation trait
pub trait Lerp {
    fn lerp(&self, other: &Self, t: f64) -> Self;
}

impl Lerp for f64 {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for Point {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        Point::new(self.x.lerp(&other.x, t), self.y.lerp(&other.y, t))
    }
}

impl Lerp for LatLng {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        LatLng::new(self.lat.lerp(&other.lat, t), self.lng.lerp(&other.lng, t))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingType {
    Linear,
    EaseOut,
    EaseInOut,
    Smooth,
    UltraSmooth,
    SpacecraftZoom,
    DynamicZoom,
}

impl EasingType {
    pub fn apply(self, t: f64) -> f64 {
        match self {
            EasingType::Linear => t,
            EasingType::EaseOut => {
                let t = t - 1.0;
                1.0 + t * t * t
            }
            EasingType::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    let t = t - 1.0;
                    1.0 + 2.0 * t * t * t
                }
            }
            EasingType::Smooth => {
                // Smooth step (3t^2 - 2t^3)
                t * t * (3.0 - 2.0 * t)
            }
            EasingType::UltraSmooth => {
                // Ultra smooth step (6t^5 - 15t^4 + 10t^3)
                t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
            }
            EasingType::SpacecraftZoom => {
                // Dramatic zoom with initial pause and rapid acceleration
                if t < 0.1 {
                    t * 2.0 // Slow start
                } else if t < 0.7 {
                    let adjusted_t = (t - 0.1) / 0.6;
                    0.2 + 0.6 * adjusted_t * adjusted_t * adjusted_t // Rapid acceleration
                } else {
                    let adjusted_t = (t - 0.7) / 0.3;
                    0.8 + 0.2 * (1.0 - (1.0 - adjusted_t).powi(3)) // Smooth landing
                }
            }
            EasingType::DynamicZoom => {
                // Zoom with slight overshoot and settle
                if t < 0.8 {
                    let adjusted_t = t / 0.8;
                    1.1 * adjusted_t * adjusted_t * (3.0 - 2.0 * adjusted_t) // Overshoot to 110%
                } else {
                    let adjusted_t = (t - 0.8) / 0.2;
                    1.1 - 0.1 * adjusted_t * adjusted_t // Settle back to 100%
                }
            }
        }
    }
}

/// Zed-inspired smooth zoom animation
#[derive(Debug, Clone)]
pub struct ZoomAnimation {
    start_time: Instant,
    duration: Duration,
    easing: EasingType,
    from_transform: Transform,
    to_transform: Transform,
    from_center: LatLng,
    to_center: LatLng,
    from_zoom: f64,
    to_zoom: f64,
    active: bool,
    focus_point: Option<Point>,
}

impl ZoomAnimation {
    pub fn new(
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
    ) -> Self {
        let duration = Duration::from_millis(350);
        Self::with_easing(
            from_center,
            to_center,
            from_zoom,
            to_zoom,
            focus_point,
            duration,
            EasingType::Smooth,
        )
    }

    pub fn with_easing(
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
        duration: Duration,
        easing: EasingType,
    ) -> Self {
        let scale_factor = 2_f64.powf(to_zoom - from_zoom);
        let origin = focus_point.unwrap_or(Point::new(0.0, 0.0));

        Self {
            start_time: Instant::now(),
            duration,
            easing,
            from_transform: Transform::identity(),
            to_transform: Transform::new(Point::new(0.0, 0.0), scale_factor, origin),
            from_center,
            to_center,
            from_zoom,
            to_zoom,
            active: true,
            focus_point,
        }
    }

    pub fn update(&mut self) -> Option<ZoomAnimationState> {
        if !self.active {
            return None;
        }

        let elapsed = self.start_time.elapsed();
        if elapsed >= self.duration {
            self.active = false;
            return Some(ZoomAnimationState {
                transform: self.to_transform,
                center: self.to_center,
                zoom: self.to_zoom,
                progress: 1.0,
                fps: 60.0, // Default FPS for completed animations
            });
        }

        let progress = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        let current_transform = self.from_transform.lerp_with_easing(&self.to_transform, progress, self.easing);

        let eased_progress = self.easing.apply(progress);
        let current_center = LatLng::new(
            self.from_center.lat + (self.to_center.lat - self.from_center.lat) * eased_progress,
            self.from_center.lng + (self.to_center.lng - self.from_center.lng) * eased_progress,
        );
        let current_zoom = self.from_zoom + (self.to_zoom - self.from_zoom) * eased_progress;

        Some(ZoomAnimationState {
            transform: current_transform,
            center: current_center,
            zoom: current_zoom,
            progress: eased_progress,
            fps: 60.0, // Default FPS for active animations
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

#[derive(Debug)]
pub struct AnimationManager {
    current_zoom_animation: Option<ZoomAnimation>,
    zoom_animation_threshold: f64,
    zoom_animation_enabled: bool,
    zoom_easing: EasingType,
    zoom_duration: Duration,
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
            zoom_animation_threshold: 1.0,
            zoom_animation_enabled: true,
            zoom_easing: EasingType::Smooth,
            zoom_duration: Duration::from_millis(350),
            keep_rendering_until: None,
        }
    }

    pub fn start_zed_zoom(
        &mut self,
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
    ) {
        if !self.zoom_animation_enabled {
            return;
        }

        let zoom_diff = (to_zoom - from_zoom).abs();
        if zoom_diff > self.zoom_animation_threshold {
            return;
        }

        let animation = ZoomAnimation::new(
            from_center, to_center, from_zoom, to_zoom, focus_point
        );

        self.current_zoom_animation = Some(animation);
        
        self.keep_rendering_until = Some(Instant::now() + self.zoom_duration + Duration::from_millis(100));
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

    pub fn set_zoom_animation_enabled(&mut self, enabled: bool) {
        self.zoom_animation_enabled = enabled;
    }

    pub fn set_zoom_animation_threshold(&mut self, threshold: f64) {
        self.zoom_animation_threshold = threshold;
    }

    pub fn set_zoom_style(&mut self, easing: EasingType, duration: Duration) {
        self.zoom_easing = easing;
        self.zoom_duration = duration;
    }

    pub fn is_animating(&self) -> bool {
        self.current_zoom_animation.is_some()
    }

    pub fn try_animate_zoom(
        &mut self,
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
        _options: Option<()>,
    ) -> bool {
        if !self.zoom_animation_enabled {
            return false;
        }

        let zoom_diff = (to_zoom - from_zoom).abs();
        if zoom_diff > self.zoom_animation_threshold {
            return false;
        }

        self.start_zed_zoom(from_center, to_center, from_zoom, to_zoom, focus_point);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        assert_eq!(EasingType::Linear.apply(0.0), 0.0);
        assert_eq!(EasingType::Linear.apply(1.0), 1.0);
        assert_eq!(EasingType::Linear.apply(0.5), 0.5);

        let ease_out = EasingType::EaseOut.apply(0.5);
        assert!(ease_out > 0.5); // Should be faster than linear
    }

    #[test]
    fn test_transform_interpolation() {
        let from = Transform::identity();
        let to = Transform::new(Point::new(100.0, 50.0), 2.0, Point::new(0.0, 0.0));
        let mid = from.lerp_with_easing(&to, 0.5, EasingType::Linear);

        assert_eq!(mid.translate.x, 50.0);
        assert_eq!(mid.translate.y, 25.0);
        assert_eq!(mid.scale, 1.5);
    }

    #[test]
    fn test_animation_manager() {
        let mut manager = AnimationManager::new();

        let from_center = LatLng::new(0.0, 0.0);
        let to_center = LatLng::new(1.0, 1.0);

        assert!(manager.try_animate_zoom(from_center, to_center, 10.0, 11.0, None, None));

        assert!(!manager.try_animate_zoom(from_center, to_center, 1.0, 10.0, None, None));
    }
}

