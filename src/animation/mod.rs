pub mod data;
pub mod interpolation;
pub mod transitions;
pub mod tweening;
pub mod utils;

// Re-export commonly used types and functions for convenience
pub use interpolation::{EasingFunction, Interpolatable, Interpolation};
pub use transitions::{Transition, TransitionBuilder, TransitionManager, TransitionType};
pub use tweening::{Tween, TweenManager, TweenState};
