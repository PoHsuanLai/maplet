//! Core constants derived from Leaflet defaults and common web-map conventions.
//! Keeping them in a single place makes it easier to tweak engine-wide magic numbers.

/// Default square tile size in pixels.
pub const TILE_SIZE: u32 = 256;

/// How far the camera is allowed to translate (in CSS pixel space)
/// before precision errors appear (2^23 for 32-bit float).
pub const TRANSFORM_3D_LIMIT: i32 = 8_388_608;

/// Snap zoom levels to these quanta (1 → integer zooms).
pub const DEFAULT_ZOOM_SNAP: f64 = 1.0;

/// Programmatic +/- zoom step when calling `zoom_in/zoom_out`.
pub const DEFAULT_ZOOM_DELTA: f64 = 1.0;

/// Do not animate a zoom if the delta exceeds this threshold.
pub const ZOOM_ANIMATION_THRESHOLD: u8 = 4;

/// Marker icon default size (regular PNG).
pub const MARKER_ICON_SIZE: (u32, u32) = (25, 41);

/// Retina (2×) marker icon size.
pub const MARKER_ICON_SIZE_2X: (u32, u32) = (41, 65);

/// Anchor inside the icon (hot-spot) in pixel coords.
pub const MARKER_ICON_ANCHOR: (u32, u32) = (12, 41);
