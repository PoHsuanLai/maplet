use crate::core::geo::{LatLng, Point};
use std::f64::consts::PI;

/// Interpolation trait for values that can be smoothly transitioned
pub trait Interpolatable {
    fn lerp(&self, other: &Self, t: f64) -> Self;
}

/// Various easing functions for animations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
}

impl EasingFunction {
    /// Apply the easing function to a normalized time value (0.0 to 1.0)
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseInQuad => t * t,
            EasingFunction::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            EasingFunction::EaseInCubic => t * t * t,
            EasingFunction::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            EasingFunction::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            EasingFunction::EaseInSine => 1.0 - (t * PI / 2.0).cos(),
            EasingFunction::EaseOutSine => (t * PI / 2.0).sin(),
            EasingFunction::EaseInOutSine => -(((PI * t).cos() - 1.0) / 2.0),
            EasingFunction::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    (2.0_f64).powf(10.0 * (t - 1.0))
                }
            }
            EasingFunction::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - (2.0_f64).powf(-10.0 * t)
                }
            }
            EasingFunction::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    (2.0_f64).powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - (2.0_f64).powf(-20.0 * t + 10.0)) / 2.0
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
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
                }
            }
        }
    }
}

/// Main interpolation utilities
pub struct Interpolation;

impl Interpolation {
    /// Linear interpolation between two f64 values
    pub fn linear(start: f64, end: f64, t: f64) -> f64 {
        start + (end - start) * t
    }

    /// Interpolation with easing function
    pub fn ease(start: f64, end: f64, t: f64, easing: EasingFunction) -> f64 {
        let eased_t = easing.apply(t);
        Self::linear(start, end, eased_t)
    }

    /// Interpolate between two LatLng coordinates
    pub fn lat_lng(start: &LatLng, end: &LatLng, t: f64) -> LatLng {
        start.lerp(end, t)
    }

    /// Interpolate between two Points
    pub fn point(start: &Point, end: &Point, t: f64) -> Point {
        start.lerp(end, t)
    }

    /// Spherical interpolation for geographical coordinates (great circle path)
    pub fn slerp_lat_lng(start: &LatLng, end: &LatLng, t: f64) -> LatLng {
        let start_rad = (start.lat.to_radians(), start.lng.to_radians());
        let end_rad = (end.lat.to_radians(), end.lng.to_radians());

        // Convert to Cartesian coordinates
        let start_cart = (
            start_rad.0.cos() * start_rad.1.cos(),
            start_rad.0.cos() * start_rad.1.sin(),
            start_rad.0.sin(),
        );
        let end_cart = (
            end_rad.0.cos() * end_rad.1.cos(),
            end_rad.0.cos() * end_rad.1.sin(),
            end_rad.0.sin(),
        );

        // Calculate the angle between vectors
        let dot = start_cart.0 * end_cart.0 + start_cart.1 * end_cart.1 + start_cart.2 * end_cart.2;
        let theta = dot.clamp(-1.0, 1.0).acos();

        if theta.abs() < 1e-6 {
            // Points are very close, use linear interpolation
            return Self::lat_lng(start, end, t);
        }

        let sin_theta = theta.sin();
        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;

        let result_cart = (
            a * start_cart.0 + b * end_cart.0,
            a * start_cart.1 + b * end_cart.1,
            a * start_cart.2 + b * end_cart.2,
        );

        // Convert back to lat/lng
        let lat = result_cart.2.asin().to_degrees();
        let lng = result_cart.1.atan2(result_cart.0).to_degrees();

        LatLng::new(lat, lng)
    }

    /// Interpolate along a bezier curve
    pub fn bezier_cubic(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        uuu * p0 + 3.0 * uu * t * p1 + 3.0 * u * tt * p2 + ttt * p3
    }
}

// Implement Interpolatable for basic types
impl Interpolatable for f64 {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        Interpolation::linear(*self, *other, t)
    }
}

impl Interpolatable for f32 {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        *self + (*other - *self) * t as f32
    }
}

impl Interpolatable for LatLng {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        LatLng::new(
            Interpolation::linear(self.lat, other.lat, t),
            Interpolation::linear(self.lng, other.lng, t),
        )
    }
}

impl Interpolatable for Point {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        Point::new(
            Interpolation::linear(self.x, other.x, t),
            Interpolation::linear(self.y, other.y, t),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_interpolation() {
        assert_eq!(Interpolation::linear(0.0, 10.0, 0.5), 5.0);
        assert_eq!(Interpolation::linear(0.0, 10.0, 0.0), 0.0);
        assert_eq!(Interpolation::linear(0.0, 10.0, 1.0), 10.0);
    }

    #[test]
    fn test_easing_functions() {
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert!(EasingFunction::EaseInQuad.apply(0.5) < 0.5);
        assert!(EasingFunction::EaseOutQuad.apply(0.5) > 0.5);
    }

    #[test]
    fn test_lat_lng_interpolation() {
        let start = LatLng::new(0.0, 0.0);
        let end = LatLng::new(10.0, 10.0);
        let mid = start.lerp(&end, 0.5);
        assert_eq!(mid, LatLng::new(5.0, 5.0));
    }
}
