use std::f64::consts::PI;

/// Utility functions for animation operations
pub struct AnimationUtils;

/// Common easing functions for animations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingType {
    /// Linear interpolation (no easing)
    Linear,
    /// Smooth start and end
    EaseInOut,
    /// Smooth start
    EaseIn,
    /// Smooth end
    EaseOut,
    /// Bounce effect
    Bounce,
    /// Elastic effect
    Elastic,
    /// Back effect (overshoot)
    Back,
    /// Custom easing function
    Custom(fn(f64) -> f64),
}

impl AnimationUtils {
    /// Apply easing function to a value between 0.0 and 1.0
    pub fn ease(t: f64, easing: EasingType) -> f64 {
        match easing {
            EasingType::Linear => t,
            EasingType::EaseInOut => Self::ease_in_out(t),
            EasingType::EaseIn => Self::ease_in(t),
            EasingType::EaseOut => Self::ease_out(t),
            EasingType::Bounce => Self::bounce(t),
            EasingType::Elastic => Self::elastic(t),
            EasingType::Back => Self::back(t),
            EasingType::Custom(func) => func(t),
        }
    }

    /// Ease-in-out function (smooth start and end)
    pub fn ease_in_out(t: f64) -> f64 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            -1.0 + (4.0 - 2.0 * t) * t
        }
    }

    /// Ease-in function (smooth start)
    pub fn ease_in(t: f64) -> f64 {
        t * t
    }

    /// Ease-out function (smooth end)
    pub fn ease_out(t: f64) -> f64 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    /// Bounce easing function
    pub fn bounce(t: f64) -> f64 {
        if t < 1.0 / 2.75 {
            7.5625 * t * t
        } else if t < 2.0 / 2.75 {
            let t = t - 1.5 / 2.75;
            7.5625 * t * t + 0.75
        } else if t < 2.5 / 2.75 {
            let t = t - 2.25 / 2.75;
            7.5625 * t * t + 0.9375
        } else {
            let t = t - 2.625 / 2.75;
            7.5625 * t * t + 0.984375
        }
    }

    /// Elastic easing function
    pub fn elastic(t: f64) -> f64 {
        if t == 0.0 {
            0.0
        } else if t == 1.0 {
            1.0
        } else {
            let p = 0.3;
            let s = p / 4.0;
            let t = t - 1.0;
            -2.0_f64.powf(10.0 * t) * ((t - s) * 2.0 * PI / p).sin()
        }
    }

    /// Back easing function (overshoot)
    pub fn back(t: f64) -> f64 {
        let s = 1.70158;
        t * t * ((s + 1.0) * t - s)
    }

    /// Interpolate between two values using a given easing function
    pub fn interpolate<T>(start: T, end: T, t: f64, easing: EasingType) -> T
    where
        T: Interpolatable,
    {
        let eased_t = Self::ease(t, easing);
        start.interpolate(&end, eased_t)
    }

    /// Create a ping-pong animation (forward then reverse)
    pub fn ping_pong(t: f64) -> f64 {
        if t < 0.5 {
            t * 2.0
        } else {
            1.0 - (t - 0.5) * 2.0
        }
    }

    /// Create a loop animation that repeats
    pub fn loop_animation(t: f64) -> f64 {
        t - t.floor()
    }

    /// Clamp a value between 0.0 and 1.0
    pub fn clamp_t(t: f64) -> f64 {
        t.clamp(0.0, 1.0)
    }

    /// Convert time in seconds to normalized time (0.0 to 1.0) given duration
    pub fn normalize_time(current_time: f64, duration: f64) -> f64 {
        (current_time / duration).clamp(0.0, 1.0)
    }

    /// Calculate the current animation state given start time, duration, and current time
    pub fn calculate_animation_state(
        start_time: f64,
        duration: f64,
        current_time: f64,
        loop_animation: bool,
        ping_pong: bool,
    ) -> AnimationState {
        let elapsed = current_time - start_time;

        if elapsed < 0.0 {
            return AnimationState::NotStarted;
        }

        if !loop_animation && elapsed >= duration {
            return AnimationState::Finished;
        }

        let normalized_time = if loop_animation {
            let loop_time = elapsed % duration;
            if ping_pong {
                Self::ping_pong(loop_time / duration)
            } else {
                Self::loop_animation(loop_time / duration)
            }
        } else {
            elapsed / duration
        };

        AnimationState::Playing {
            progress: normalized_time.clamp(0.0, 1.0),
            elapsed,
        }
    }

    /// Create a smooth step function
    pub fn smooth_step(t: f64) -> f64 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Create a smoother step function
    pub fn smoother_step(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    /// Create a custom easing function from control points (Bezier curve)
    pub fn bezier_easing(p1: f64, p2: f64, p3: f64, p4: f64) -> impl Fn(f64) -> f64 {
        move |t| {
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;

            mt3 * p1 + 3.0 * mt2 * t * p2 + 3.0 * mt * t2 * p3 + t3 * p4
        }
    }

    /// Create a spring animation function
    pub fn spring_easing(stiffness: f64, damping: f64) -> impl Fn(f64) -> f64 {
        move |t| {
            let omega = (stiffness / damping).sqrt();
            let zeta = damping / (2.0 * (stiffness * damping).sqrt());

            if zeta < 1.0 {
                let omega_d = omega * (1.0 - zeta * zeta).sqrt();
                let a = 1.0;
                let phi = (zeta * omega).atan2(omega_d);

                1.0 - a * (-zeta * omega * t).exp() * (omega_d * t + phi).cos()
            } else {
                1.0 - (-omega * t).exp() * (1.0 + omega * t)
            }
        }
    }

    /// Calculate the derivative of an easing function (useful for velocity)
    pub fn ease_derivative(t: f64, easing: EasingType) -> f64 {
        const EPSILON: f64 = 1e-6;
        let t1 = (t - EPSILON).max(0.0);
        let t2 = (t + EPSILON).min(1.0);
        (Self::ease(t2, easing) - Self::ease(t1, easing)) / (t2 - t1)
    }

    /// Create a custom easing function from a list of keyframes
    pub fn custom_easing_from_keyframes(keyframes: &[(f64, f64)]) -> impl Fn(f64) -> f64 {
        let keyframes = keyframes.to_vec();
        move |t| {
            if keyframes.is_empty() {
                return t;
            }

            if t <= keyframes[0].0 {
                return keyframes[0].1;
            }

            if t >= keyframes.last().unwrap().0 {
                return keyframes.last().unwrap().1;
            }

            // Find the appropriate segment
            for i in 0..keyframes.len() - 1 {
                let (t1, v1) = keyframes[i];
                let (t2, v2) = keyframes[i + 1];

                if t >= t1 && t <= t2 {
                    let segment_t = (t - t1) / (t2 - t1);
                    return v1 + (v2 - v1) * segment_t;
                }
            }

            t
        }
    }
}

/// Animation state information
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationState {
    /// Animation hasn't started yet
    NotStarted,
    /// Animation is currently playing
    Playing {
        /// Progress from 0.0 to 1.0
        progress: f64,
        /// Elapsed time in seconds
        elapsed: f64,
    },
    /// Animation has finished
    Finished,
}

/// Trait for types that can be interpolated
pub trait Interpolatable {
    /// Interpolate between self and other at the given time t (0.0 to 1.0)
    fn interpolate(&self, other: &Self, t: f64) -> Self;
}

impl Interpolatable for f64 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Interpolatable for f32 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t as f32
    }
}

impl Interpolatable for i32 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        let result = *self as f64 + (*other as f64 - *self as f64) * t;
        result.round() as i32
    }
}

impl Interpolatable for u32 {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        let result = *self as f64 + (*other as f64 - *self as f64) * t;
        result.round().max(0.0) as u32
    }
}

impl Interpolatable for crate::core::geo::Point {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        crate::core::geo::Point {
            x: self.x.interpolate(&other.x, t),
            y: self.y.interpolate(&other.y, t),
        }
    }
}

impl Interpolatable for crate::core::geo::LatLng {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        crate::core::geo::LatLng {
            lat: self.lat.interpolate(&other.lat, t),
            lng: self.lng.interpolate(&other.lng, t),
        }
    }
}

impl<T: Interpolatable> Interpolatable for Vec<T> {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        let len = self.len().min(other.len());
        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            result.push(self[i].interpolate(&other[i], t));
        }

        result
    }
}

impl<T: Interpolatable + Clone> Interpolatable for Option<T> {
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        match (self, other) {
            (Some(a), Some(b)) => Some(a.interpolate(b, t)),
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        assert_eq!(AnimationUtils::ease(0.0, EasingType::Linear), 0.0);
        assert_eq!(AnimationUtils::ease(1.0, EasingType::Linear), 1.0);
        assert_eq!(AnimationUtils::ease(0.5, EasingType::Linear), 0.5);

        assert_eq!(AnimationUtils::ease(0.0, EasingType::EaseInOut), 0.0);
        assert_eq!(AnimationUtils::ease(1.0, EasingType::EaseInOut), 1.0);

        assert_eq!(AnimationUtils::ease(0.0, EasingType::EaseIn), 0.0);
        assert_eq!(AnimationUtils::ease(1.0, EasingType::EaseIn), 1.0);

        assert_eq!(AnimationUtils::ease(0.0, EasingType::EaseOut), 0.0);
        assert_eq!(AnimationUtils::ease(1.0, EasingType::EaseOut), 1.0);
    }

    #[test]
    fn test_interpolation() {
        let start = 0.0;
        let end = 100.0;

        assert_eq!(
            AnimationUtils::interpolate(start, end, 0.0, EasingType::Linear),
            0.0
        );
        assert_eq!(
            AnimationUtils::interpolate(start, end, 0.5, EasingType::Linear),
            50.0
        );
        assert_eq!(
            AnimationUtils::interpolate(start, end, 1.0, EasingType::Linear),
            100.0
        );
    }

    #[test]
    fn test_animation_state() {
        let state = AnimationUtils::calculate_animation_state(0.0, 10.0, 5.0, false, false);
        assert!(
            matches!(state, AnimationState::Playing { progress, elapsed } if progress == 0.5 && elapsed == 5.0)
        );

        let state = AnimationUtils::calculate_animation_state(0.0, 10.0, 15.0, false, false);
        assert!(matches!(state, AnimationState::Finished));

        let state = AnimationUtils::calculate_animation_state(0.0, 10.0, -5.0, false, false);
        assert!(matches!(state, AnimationState::NotStarted));
    }

    #[test]
    fn test_ping_pong() {
        assert_eq!(AnimationUtils::ping_pong(0.0), 0.0);
        assert_eq!(AnimationUtils::ping_pong(0.5), 1.0);
        assert_eq!(AnimationUtils::ping_pong(1.0), 0.0);
    }

    #[test]
    fn test_loop_animation() {
        assert_eq!(AnimationUtils::loop_animation(0.0), 0.0);
        assert_eq!(AnimationUtils::loop_animation(0.5), 0.5);
        assert_eq!(AnimationUtils::loop_animation(1.0), 0.0);
        assert_eq!(AnimationUtils::loop_animation(1.5), 0.5);
    }
}
