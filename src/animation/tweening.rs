use crate::core::geo::{LatLng, Point};
use std::time::{Duration, Instant};

/// Easing functions for smooth animations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
}

impl EasingFunction {
    /// Apply the easing function to a normalized time value (0.0 to 1.0)
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);

        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            EasingFunction::EaseInQuad => t * t,
            EasingFunction::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t).powi(2)
                }
            }
            EasingFunction::EaseInCubic => t * t * t,
            EasingFunction::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            EasingFunction::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - 4.0 * (1.0 - t).powi(3)
                }
            }
            EasingFunction::EaseInQuart => t.powi(4),
            EasingFunction::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            EasingFunction::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t.powi(4)
                } else {
                    1.0 - 8.0 * (1.0 - t).powi(4)
                }
            }
            EasingFunction::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    2.0_f64.powf(10.0 * (t - 1.0))
                }
            }
            EasingFunction::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f64.powf(-10.0 * t)
                }
            }
            EasingFunction::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    0.5 * 2.0_f64.powf(20.0 * t - 10.0)
                } else {
                    0.5 * (2.0 - 2.0_f64.powf(-20.0 * t + 10.0))
                }
            }
            EasingFunction::EaseInCirc => 1.0 - (1.0 - t * t).sqrt(),
            EasingFunction::EaseOutCirc => (1.0 - (t - 1.0).powi(2)).sqrt(),
            EasingFunction::EaseInOutCirc => {
                if t < 0.5 {
                    0.5 * (1.0 - (1.0 - 4.0 * t * t).sqrt())
                } else {
                    0.5 * ((1.0 - (2.0 * t - 2.0).powi(2)).sqrt() + 1.0)
                }
            }
            EasingFunction::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            EasingFunction::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            EasingFunction::EaseInOutBack => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    0.5 * ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2))
                } else {
                    0.5 * ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (2.0 * t - 2.0) + c2) + 2.0)
                }
            }
            EasingFunction::EaseInBounce => 1.0 - EasingFunction::EaseOutBounce.apply(1.0 - t),
            EasingFunction::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;

                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            EasingFunction::EaseInOutBounce => {
                if t < 0.5 {
                    0.5 * EasingFunction::EaseInBounce.apply(2.0 * t)
                } else {
                    0.5 * EasingFunction::EaseOutBounce.apply(2.0 * t - 1.0) + 0.5
                }
            }
        }
    }
}

/// Represents an animatable value that can be tweened
pub trait Tweenable {
    /// Interpolate between self and other by factor t (0.0 to 1.0)
    fn lerp(&self, other: &Self, t: f64) -> Self;
}

impl Tweenable for f64 {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Tweenable for Point {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        Point::new(self.x.lerp(&other.x, t), self.y.lerp(&other.y, t))
    }
}

impl Tweenable for LatLng {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        LatLng::new(self.lat.lerp(&other.lat, t), self.lng.lerp(&other.lng, t))
    }
}

/// A tween animation between two values
#[derive(Debug, Clone)]
pub struct Tween<T: Tweenable + Clone> {
    /// Starting value
    pub from: T,
    /// Ending value
    pub to: T,
    /// Animation duration
    pub duration: Duration,
    /// Easing function to use
    pub easing: EasingFunction,
    /// When the animation started
    start_time: Option<Instant>,
    /// Whether the animation is paused
    paused: bool,
    /// Time elapsed while paused
    pause_time: Duration,
}

impl<T: Tweenable + Clone> Tween<T> {
    /// Create a new tween
    pub fn new(from: T, to: T, duration: Duration) -> Self {
        Self {
            from,
            to,
            duration,
            easing: EasingFunction::EaseInOut,
            start_time: None,
            paused: false,
            pause_time: Duration::ZERO,
        }
    }

    /// Create a new tween with custom easing
    pub fn with_easing(from: T, to: T, duration: Duration, easing: EasingFunction) -> Self {
        Self {
            from,
            to,
            duration,
            easing,
            start_time: None,
            paused: false,
            pause_time: Duration::ZERO,
        }
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.paused = false;
        self.pause_time = Duration::ZERO;
    }

    /// Pause the animation
    pub fn pause(&mut self) {
        if !self.paused && self.start_time.is_some() {
            self.paused = true;
        }
    }

    /// Resume the animation
    pub fn resume(&mut self) {
        if self.paused {
            self.paused = false;
            // Adjust start time to account for pause duration
            if let Some(start) = self.start_time {
                self.start_time = Some(start + self.pause_time);
            }
            self.pause_time = Duration::ZERO;
        }
    }

    /// Stop the animation
    pub fn stop(&mut self) {
        self.start_time = None;
        self.paused = false;
        self.pause_time = Duration::ZERO;
    }

    /// Check if the animation is running
    pub fn is_running(&self) -> bool {
        self.start_time.is_some() && !self.paused
    }

    /// Check if the animation is finished
    pub fn is_finished(&self) -> bool {
        if let Some(start) = self.start_time {
            if !self.paused {
                Instant::now().duration_since(start) >= self.duration
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get the current progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if let Some(start) = self.start_time {
            if self.paused {
                // Return progress at pause time
                self.pause_time.as_secs_f64() / self.duration.as_secs_f64()
            } else {
                let elapsed = Instant::now().duration_since(start);
                (elapsed.as_secs_f64() / self.duration.as_secs_f64()).min(1.0)
            }
        } else {
            0.0
        }
    }

    /// Get the current value
    pub fn current_value(&self) -> T {
        let progress = self.progress();
        let eased_progress = self.easing.apply(progress);
        self.from.lerp(&self.to, eased_progress)
    }

    /// Update the tween and return the current value
    pub fn update(&mut self) -> Option<T> {
        self.start_time?;

        if self.paused {
            self.pause_time += Duration::from_millis(16); // Approximate frame time
            return Some(self.current_value());
        }

        if self.is_finished() {
            self.stop();
            Some(self.to.clone())
        } else {
            Some(self.current_value())
        }
    }

    /// Set the easing function
    pub fn set_easing(&mut self, easing: EasingFunction) {
        self.easing = easing;
    }

    /// Change the target value (useful for chaining animations)
    pub fn change_target(&mut self, new_to: T) {
        if self.is_running() {
            // Start from current position
            self.from = self.current_value();
            self.to = new_to;
            self.start();
        } else {
            self.to = new_to;
        }
    }
}

/// A sequence of tweens that play one after another
#[derive(Debug, Clone)]
pub struct TweenSequence<T: Tweenable + Clone> {
    tweens: Vec<Tween<T>>,
    current_index: usize,
    loops: Option<u32>,
    current_loop: u32,
}

impl<T: Tweenable + Clone> TweenSequence<T> {
    /// Create a new empty sequence
    pub fn new() -> Self {
        Self {
            tweens: Vec::new(),
            current_index: 0,
            loops: None,
            current_loop: 0,
        }
    }

    /// Add a tween to the sequence
    pub fn add_tween(mut self, tween: Tween<T>) -> Self {
        self.tweens.push(tween);
        self
    }

    /// Set the number of loops (None for infinite)
    pub fn set_loops(mut self, loops: Option<u32>) -> Self {
        self.loops = loops;
        self
    }

    /// Start the sequence
    pub fn start(&mut self) {
        if !self.tweens.is_empty() {
            self.current_index = 0;
            self.current_loop = 0;
            self.tweens[0].start();
        }
    }

    /// Update the sequence and return current value
    pub fn update(&mut self) -> Option<T> {
        if self.tweens.is_empty() || self.current_index >= self.tweens.len() {
            return None;
        }

        let current_tween = &mut self.tweens[self.current_index];

        if let Some(value) = current_tween.update() {
            if current_tween.is_finished() {
                // Move to next tween
                self.current_index += 1;

                if self.current_index >= self.tweens.len() {
                    // End of sequence
                    if let Some(max_loops) = self.loops {
                        self.current_loop += 1;
                        if self.current_loop < max_loops {
                            // Start next loop
                            self.current_index = 0;
                            self.tweens[0].start();
                        }
                    } else {
                        // Infinite loops
                        self.current_index = 0;
                        self.tweens[0].start();
                    }
                } else {
                    // Start next tween
                    self.tweens[self.current_index].start();
                }
            }

            Some(value)
        } else {
            None
        }
    }

    /// Check if the sequence is finished
    pub fn is_finished(&self) -> bool {
        if let Some(max_loops) = self.loops {
            self.current_loop >= max_loops
        } else {
            false // Infinite loops never finish
        }
    }
}

impl<T: Tweenable + Clone> Default for TweenSequence<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for common animation patterns
pub struct TweenBuilder;

impl TweenBuilder {
    /// Create a smooth zoom animation
    pub fn zoom(from: f64, to: f64, duration_ms: u64) -> Tween<f64> {
        Tween::with_easing(
            from,
            to,
            Duration::from_millis(duration_ms),
            EasingFunction::EaseInOutQuad,
        )
    }

    /// Create a smooth pan animation
    pub fn pan(from: LatLng, to: LatLng, duration_ms: u64) -> Tween<LatLng> {
        Tween::with_easing(
            from,
            to,
            Duration::from_millis(duration_ms),
            EasingFunction::EaseInOutCubic,
        )
    }

    /// Create a bouncy appearance animation
    pub fn bounce_in(from: f64, to: f64, duration_ms: u64) -> Tween<f64> {
        Tween::with_easing(
            from,
            to,
            Duration::from_millis(duration_ms),
            EasingFunction::EaseOutBounce,
        )
    }

    /// Create a smooth fade animation
    pub fn fade(from: f64, to: f64, duration_ms: u64) -> Tween<f64> {
        Tween::with_easing(
            from,
            to,
            Duration::from_millis(duration_ms),
            EasingFunction::EaseInOut,
        )
    }
}

/// State of a tween animation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TweenState {
    NotStarted,
    Running,
    Paused,
    Finished,
    Stopped,
}

/// Manages multiple tween animations
pub struct TweenManager {
    tweens: std::collections::HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    next_id: u64,
}

impl TweenManager {
    /// Create a new tween manager
    pub fn new() -> Self {
        Self {
            tweens: std::collections::HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a tween with a custom ID
    pub fn add_tween_with_id<T: Tweenable + Clone + Send + Sync + 'static>(
        &mut self,
        id: String,
        tween: Tween<T>,
    ) {
        self.tweens.insert(id, Box::new(tween));
    }

    /// Add a tween and return its auto-generated ID
    pub fn add_tween<T: Tweenable + Clone + Send + Sync + 'static>(
        &mut self,
        tween: Tween<T>,
    ) -> String {
        let id = format!("tween_{}", self.next_id);
        self.next_id += 1;
        self.add_tween_with_id(id.clone(), tween);
        id
    }

    /// Remove a tween by ID
    pub fn remove_tween(&mut self, id: &str) -> bool {
        self.tweens.remove(id).is_some()
    }

    /// Update all tweens and return their current values
    pub fn update(
        &mut self,
    ) -> std::collections::HashMap<String, Box<dyn std::any::Any + Send + Sync>> {
        let results = std::collections::HashMap::new();
        let mut finished_tweens = Vec::new();

        // Check for finished tweens first
        for id in self.tweens.keys() {
            if self.is_tween_finished(id) {
                finished_tweens.push(id.clone());
            }
        }

        // Remove finished tweens
        for id in finished_tweens {
            self.tweens.remove(&id);
        }

        results
    }

    /// Check if a specific tween is finished
    fn is_tween_finished(&self, id: &str) -> bool {
        if let Some(tween_any) = self.tweens.get(id) {
            // Try to downcast to different tween types
            if let Some(tween) = tween_any.downcast_ref::<Tween<f64>>() {
                return tween.is_finished();
            }
            if let Some(tween) = tween_any.downcast_ref::<Tween<Point>>() {
                return tween.is_finished();
            }
            if let Some(tween) = tween_any.downcast_ref::<Tween<LatLng>>() {
                return tween.is_finished();
            }
        }
        false
    }

    /// Get the number of active tweens
    pub fn active_count(&self) -> usize {
        self.tweens.len()
    }

    /// Clear all tweens
    pub fn clear(&mut self) {
        self.tweens.clear();
    }

    /// Pause all tweens
    pub fn pause_all(&mut self) {
        for (_, tween_any) in self.tweens.iter_mut() {
            // Try to downcast to different tween types and pause them
            if let Some(tween) = tween_any.downcast_mut::<Tween<f64>>() {
                tween.pause();
            } else if let Some(tween) = tween_any.downcast_mut::<Tween<Point>>() {
                tween.pause();
            } else if let Some(tween) = tween_any.downcast_mut::<Tween<LatLng>>() {
                tween.pause();
            }
        }
    }

    /// Resume all tweens
    pub fn resume_all(&mut self) {
        for (_, tween_any) in self.tweens.iter_mut() {
            // Try to downcast to different tween types and resume them
            if let Some(tween) = tween_any.downcast_mut::<Tween<f64>>() {
                tween.resume();
            } else if let Some(tween) = tween_any.downcast_mut::<Tween<Point>>() {
                tween.resume();
            } else if let Some(tween) = tween_any.downcast_mut::<Tween<LatLng>>() {
                tween.resume();
            }
        }
    }

    /// Stop all tweens
    pub fn stop_all(&mut self) {
        self.tweens.clear();
    }
}

impl Default for TweenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        assert_eq!(EasingFunction::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFunction::Linear.apply(1.0), 1.0);
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);

        assert_eq!(EasingFunction::EaseIn.apply(0.0), 0.0);
        assert_eq!(EasingFunction::EaseIn.apply(1.0), 1.0);
        assert!(EasingFunction::EaseIn.apply(0.5) < 0.5); // Should be slower at start
    }

    #[test]
    fn test_f64_lerp() {
        assert_eq!(0.0_f64.lerp(&10.0, 0.0), 0.0);
        assert_eq!(0.0_f64.lerp(&10.0, 1.0), 10.0);
        assert_eq!(0.0_f64.lerp(&10.0, 0.5), 5.0);
    }

    #[test]
    fn test_point_lerp() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(10.0, 20.0);
        let result = p1.lerp(&p2, 0.5);
        assert_eq!(result.x, 5.0);
        assert_eq!(result.y, 10.0);
    }

    #[test]
    fn test_tween_creation() {
        let tween = Tween::new(0.0, 10.0, Duration::from_millis(1000));
        assert_eq!(tween.from, 0.0);
        assert_eq!(tween.to, 10.0);
        assert!(!tween.is_running());
    }

    #[test]
    fn test_tween_progress() {
        let mut tween = Tween::new(0.0, 10.0, Duration::from_millis(100));
        tween.start();

        // Progress should be between 0 and 1
        let progress = tween.progress();
        assert!((0.0..=1.0).contains(&progress));
    }

    #[test]
    fn test_tween_builder() {
        let zoom_tween = TweenBuilder::zoom(1.0, 10.0, 1000);
        assert_eq!(zoom_tween.from, 1.0);
        assert_eq!(zoom_tween.to, 10.0);
        assert_eq!(zoom_tween.easing, EasingFunction::EaseInOutQuad);
    }
}
