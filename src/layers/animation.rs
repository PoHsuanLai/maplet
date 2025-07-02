use crate::core::geo::{LatLng, Point};
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
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingType::Linear => t,
            EasingType::EaseOut => 1.0 - (1.0 - t).powi(3),
            EasingType::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            EasingType::Smooth => {
                let overshoot = 1.03;
                let bounce_back = 0.97;
                if t < 0.7 {
                    let normalized = t / 0.7;
                    let base = 1.0 - (1.0 - normalized).powi(4);
                    base * overshoot
                } else {
                    let normalized = (t - 0.7) / 0.3;
                    let bounce = overshoot - (overshoot - bounce_back) * normalized;
                    bounce + (1.0 - bounce_back) * normalized
                }
            }
            EasingType::UltraSmooth => {
                let x = t * std::f64::consts::PI;
                0.5 * (1.0 - (x).cos())
            }
            EasingType::SpacecraftZoom => {
                if t < 0.1 {
                    4.0 * t * t
                } else if t < 0.8 {
                    let normalized = (t - 0.1) / 0.7;
                    0.04 + 0.92 * normalized
                } else {
                    let normalized = (t - 0.8) / 0.2;
                    0.96 + 0.06 * (1.0 - (1.0 - normalized).powi(3))
                }
            }
            EasingType::DynamicZoom => {
                let bounce = 0.05;
                if t < 0.8 {
                    let base = t / 0.8;
                    let smooth = 1.0 - (1.0 - base).powi(3);
                    smooth * (1.0 + bounce)
                } else {
                    let overshoot = 1.0 + bounce;
                    let return_phase = (t - 0.8) / 0.2;
                    overshoot - bounce * return_phase
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameTiming {
    target_fps: Option<u32>,
    last_frame_time: Instant,
    frame_duration_ms: f64,
    adaptive_timing: bool,
    display_refresh_rate: f64,
    frame_times: std::collections::VecDeque<f64>,
}

impl FrameTiming {
    pub fn new(target_fps: Option<u32>) -> Self {
        Self {
            target_fps,
            last_frame_time: Instant::now(),
            frame_duration_ms: 16.67, // Default to ~60fps
            adaptive_timing: true,
            display_refresh_rate: 60.0,
            frame_times: std::collections::VecDeque::with_capacity(10),
        }
    }

    pub fn with_promotion_support(mut self) -> Self {
        self.adaptive_timing = true;
        self.display_refresh_rate = 120.0; // Assume 120Hz capable
        self
    }

    pub fn update_frame_timing(&mut self) -> bool {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_frame_time).as_secs_f64() * 1000.0;
        
        self.frame_times.push_back(elapsed_ms);
        if self.frame_times.len() > 10 {
            self.frame_times.pop_front();
        }
        
        let avg_duration: f64 = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
        self.frame_duration_ms = avg_duration;
        
        self.last_frame_time = now;
        
        if let Some(target_fps) = self.target_fps {
            let target_duration = 1000.0 / target_fps as f64;
            elapsed_ms >= target_duration * 0.95 // 5% tolerance
        } else {
            true
        }
    }

    pub fn current_fps(&self) -> f64 {
        if self.frame_duration_ms > 0.0 {
            1000.0 / self.frame_duration_ms
        } else {
            60.0
        }
    }

    pub fn is_hitting_target(&self) -> bool {
        if let Some(target) = self.target_fps {
            let current = self.current_fps();
            (current - target as f64).abs() < 5.0
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translate: Point,
    pub scale: f64,
    pub origin: Point,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translate: Point::new(0.0, 0.0),
            scale: 1.0,
            origin: Point::new(0.0, 0.0),
        }
    }
}

impl Transform {
    pub fn new(translate: Point, scale: f64, origin: Point) -> Self {
        Self {
            translate,
            scale,
            origin,
        }
    }

    /// Create identity transform (no change)
    pub fn identity() -> Self {
        Self::default()
    }

    /// Check if this is effectively an identity transform
    pub fn is_identity(&self) -> bool {
        (self.scale - 1.0).abs() < 0.001
            && self.translate.x.abs() < 0.1
            && self.translate.y.abs() < 0.1
    }

    /// Interpolate between two transforms with easing
    pub fn lerp_with_easing(&self, other: &Transform, t: f64, easing: EasingType) -> Transform {
        let eased_t = easing.apply(t);
        Transform {
            translate: Point::new(
                self.translate.x + (other.translate.x - self.translate.x) * eased_t,
                self.translate.y + (other.translate.y - self.translate.y) * eased_t,
            ),
            scale: self.scale + (other.scale - self.scale) * eased_t,
            origin: Point::new(
                self.origin.x + (other.origin.x - self.origin.x) * eased_t,
                self.origin.y + (other.origin.y - self.origin.y) * eased_t,
            ),
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
    frame_timing: FrameTiming,
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
        ).with_promotion_support()
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
            frame_timing: FrameTiming::new(Some(60)),
        }
    }

    pub fn with_promotion_support(mut self) -> Self {
        self.frame_timing = self.frame_timing.with_promotion_support();
        self
    }

    pub fn update(&mut self) -> Option<ZoomAnimationState> {
        if !self.active {
            return None;
        }

        if !self.frame_timing.update_frame_timing() {
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
                fps: self.frame_timing.current_fps(),
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
            fps: self.frame_timing.current_fps(),
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
            current_fps: self.frame_timing.current_fps(),
            is_hitting_target: self.frame_timing.is_hitting_target(),
            frame_duration_ms: self.frame_timing.frame_duration_ms,
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
    frame_timing: FrameTiming,
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
            frame_timing: FrameTiming::new(Some(60)).with_promotion_support(),
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
        self.frame_timing.update_frame_timing();

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
                current_fps: self.frame_timing.current_fps(),
                is_hitting_target: self.frame_timing.is_hitting_target(),
                frame_duration_ms: self.frame_timing.frame_duration_ms,
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

