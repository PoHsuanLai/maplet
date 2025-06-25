use crate::core::geo::{LatLng, Point};
use serde::{Deserialize, Serialize};

/// Manages the current view of the map: center, zoom, and screen dimensions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// The center of the map view in geographical coordinates
    pub center: LatLng,
    /// The current zoom level
    pub zoom: f64,
    /// The size of the viewport in pixels
    pub size: Point,
    /// The minimum allowed zoom level
    pub min_zoom: f64,
    /// The maximum allowed zoom level
    pub max_zoom: f64,
}

impl Viewport {
    /// Creates a new viewport
    pub fn new(center: LatLng, zoom: f64, size: Point) -> Self {
        Self {
            center,
            zoom: zoom.clamp(0.0, 18.0),
            size,
            min_zoom: 0.0,
            max_zoom: 18.0,
        }
    }

    /// Sets the center of the viewport
    pub fn set_center(&mut self, center: LatLng) {
        self.center = self.clamp_center(center);
    }

    /// Sets the zoom level, clamping to valid range
    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
    }

    /// Sets the viewport size
    pub fn set_size(&mut self, size: Point) {
        self.size = size;
    }

    /// Sets the zoom limits
    pub fn set_zoom_limits(&mut self, min_zoom: f64, max_zoom: f64) {
        self.min_zoom = min_zoom;
        self.max_zoom = max_zoom;
        self.zoom = self.zoom.clamp(min_zoom, max_zoom);
    }

    /// Gets the scale factor for the current zoom level
    pub fn scale(&self) -> f64 {
        2_f64.powf(self.zoom)
    }

    /// Converts a geographical coordinate to screen pixel coordinates (top-left origin)
    /// using standard Web-Mercator world pixel math.
    pub fn lat_lng_to_pixel(&self, lat_lng: &LatLng) -> Point {
        // World pixel coordinates for the target point and the viewport center
        let world_scale = 256.0 * self.scale();

        // Helper to convert to world pixel (unwrapped)
        fn to_world_px(lat: f64, lng: f64, scale: f64) -> (f64, f64) {
            // Clamp latitude to the Web-Mercator limit to avoid infinity at the poles
            let lat = LatLng::clamp_lat(lat);
            let lat_rad = lat.to_radians();

            let x = (lng + 180.0) / 360.0 * scale;
            let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * scale;
            (x, y)
        }

        let (center_x, center_y) = to_world_px(self.center.lat, self.center.lng, world_scale);
        let (pt_x, pt_y) = to_world_px(lat_lng.lat, lat_lng.lng, world_scale);

        let dx = pt_x - center_x;
        let dy = pt_y - center_y;

        Point::new(self.size.x / 2.0 + dx, self.size.y / 2.0 + dy)
    }

    /// Converts screen pixel coordinates (relative to viewport) back to geographical coordinates.
    pub fn pixel_to_lat_lng(&self, pixel: &Point) -> LatLng {
        let world_scale = 256.0 * self.scale();

        // Helper to convert from world pixel to lat/lng
        fn from_world_px(x: f64, y: f64, scale: f64) -> LatLng {
            let lng = x / scale * 360.0 - 180.0;
            // Robust inverse Web-Mercator using equirectangular y
            // 1. Convert from world pixel Y back to normalized Mercator (0..1).
            let merc_y = y / scale; // 0 at top, 1 at bottom
            // 2. Transform: lat = 90° - 360° * atan(exp((0.5 - merc_y) * 2π)) / π
            let lat_rad = std::f64::consts::FRAC_PI_2
                - 2.0 * ((0.5 - merc_y) * 2.0 * std::f64::consts::PI).exp().atan();
            let lat = lat_rad.to_degrees();
            LatLng::new(lat, lng)
        }

        // World pixel of viewport center
        let center_world_x = (self.center.lng + 180.0) / 360.0 * world_scale;
        let lat_rad_center = LatLng::clamp_lat(self.center.lat).to_radians();
        let center_world_y = (1.0 - (lat_rad_center.tan() + 1.0 / lat_rad_center.cos()).ln() / std::f64::consts::PI) / 2.0 * world_scale;

        // World pixel of requested screen pixel
        let world_x = center_world_x + (pixel.x - self.size.x / 2.0);
        let world_y = center_world_y + (pixel.y - self.size.y / 2.0);

        from_world_px(world_x, world_y, world_scale)
    }

    /// Pans the viewport by the given pixel offset
    pub fn pan(&mut self, delta: Point) {
        let current_pixel = Point::new(self.size.x / 2.0, self.size.y / 2.0);
        let new_pixel = current_pixel.subtract(&delta);
        self.center = self.clamp_center(self.pixel_to_lat_lng(&new_pixel));
    }

    /// Zooms the viewport to a specific level at a given point
    pub fn zoom_to(&mut self, zoom: f64, focus_point: Option<Point>) {
        // --- New implementation mirrors Leaflet's `setZoomAround` logic ---
        let new_zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        let old_zoom = self.zoom;

        // No-op if zoom does not change
        if (new_zoom - old_zoom).abs() < f64::EPSILON {
            return;
        }

        // Screen point we zoom around (defaults to viewport center)
        let focus_screen = focus_point.unwrap_or(Point::new(self.size.x / 2.0, self.size.y / 2.0));

        // Scale factor between zooms
        let scale_factor = 2_f64.powf(new_zoom - old_zoom);

        // Vector from viewport center to the focus point (in screen px)
        let view_half = Point::new(self.size.x / 2.0, self.size.y / 2.0);
        let center_offset_screen = focus_screen.subtract(&view_half);

        // How much the center must shift in world-pixel coordinates so that the focus stays stationary
        let offset = center_offset_screen.multiply(1.0 - 1.0 / scale_factor);

        // Helper closures for world-pixel conversions at an arbitrary scale
        fn to_world_px(lat: f64, lng: f64, world_scale: f64) -> (f64, f64) {
            let lat = crate::core::geo::LatLng::clamp_lat(lat);
            let lat_rad = lat.to_radians();
            let x = (lng + 180.0) / 360.0 * world_scale;
            let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * world_scale;
            (x, y)
        }

        fn from_world_px(x: f64, y: f64, world_scale: f64) -> crate::core::geo::LatLng {
            let lng = x / world_scale * 360.0 - 180.0;
            let merc_y = y / world_scale;
            let lat_rad = std::f64::consts::FRAC_PI_2
                - 2.0 * ((0.5 - merc_y) * 2.0 * std::f64::consts::PI).exp().atan();
            let lat = lat_rad.to_degrees();
            crate::core::geo::LatLng::new(lat, lng)
        }

        // Center in world-pixel space at *new* zoom level
        let world_scale_new = 256.0 * 2_f64.powf(new_zoom);
        let (center_world_x, center_world_y) = to_world_px(self.center.lat, self.center.lng, world_scale_new);

        // Apply offset so that the focus point remains stationary
        let new_center_world_x = center_world_x + offset.x;
        let new_center_world_y = center_world_y + offset.y;

        let new_center_latlng = from_world_px(new_center_world_x, new_center_world_y, world_scale_new);

        // Commit
        self.zoom = new_zoom;
        self.center = self.clamp_center(new_center_latlng);
    }

    /// Zooms in by one level
    pub fn zoom_in(&mut self, focus_point: Option<Point>) {
        self.zoom_to(self.zoom + 1.0, focus_point);
    }

    /// Zooms out by one level  
    pub fn zoom_out(&mut self, focus_point: Option<Point>) {
        self.zoom_to(self.zoom - 1.0, focus_point);
    }

    /// Gets the current viewport bounds in geographical coordinates
    pub fn bounds(&self) -> crate::core::geo::LatLngBounds {
        let nw_pixel = Point::new(0.0, 0.0);
        let se_pixel = Point::new(self.size.x, self.size.y);

        let nw = self.pixel_to_lat_lng(&nw_pixel);
        let se = self.pixel_to_lat_lng(&se_pixel);

        crate::core::geo::LatLngBounds::new(
            LatLng::new(se.lat, nw.lng),
            LatLng::new(nw.lat, se.lng),
        )
    }

    /// Fits the viewport to contain the given bounds
    pub fn fit_bounds(&mut self, bounds: &crate::core::geo::LatLngBounds, padding: Option<f64>) {
        let padding = padding.unwrap_or(20.0);

        // Calculate the center
        self.center = bounds.center();

        // Calculate the required zoom level
        let bounds_size = bounds.span();
        let viewport_size = Point::new(self.size.x - 2.0 * padding, self.size.y - 2.0 * padding);

        // Convert degrees to pixels at zoom 0
        let bounds_pixels_x = bounds_size.lng * 256.0 / 360.0;
        let bounds_pixels_y = bounds_size.lat * 256.0 / 180.0;

        let zoom_x = (viewport_size.x / bounds_pixels_x).log2();
        let zoom_y = (viewport_size.y / bounds_pixels_y).log2();

        self.zoom = zoom_x.min(zoom_y).clamp(self.min_zoom, self.max_zoom);
    }

    /// Checks if a geographical point is visible in the current viewport
    pub fn contains_lat_lng(&self, lat_lng: &LatLng) -> bool {
        let pixel = self.lat_lng_to_pixel(lat_lng);
        pixel.x >= 0.0 && pixel.x <= self.size.x && pixel.y >= 0.0 && pixel.y <= self.size.y
    }

    /// Gets the resolution in meters per pixel at the current zoom level
    pub fn resolution(&self) -> f64 {
        // At zoom 0, one pixel = 156543.03 meters at the equator
        let base_resolution = 156543.03392804097;
        base_resolution / self.scale()
    }

    /// Gets the scale denominator for the current zoom level
    pub fn scale_denominator(&self) -> f64 {
        // Standard scale denominator calculation
        let meters_per_inch = 0.0254;
        let inches_per_meter = 1.0 / meters_per_inch;
        let dpi = 96.0; // Standard screen DPI

        self.resolution() * inches_per_meter * dpi
    }

    /// Clamp a candidate center so that the viewport remains inside the Web-Mercator world square.
    fn clamp_center(&self, center: LatLng) -> LatLng {
        // World pixel scale at this zoom
        let world_scale = 256.0 * self.scale();

        // Helper for conversions (duplicated logic from lat_lng_to_pixel)
        fn to_world_px(lat: f64, lng: f64, scale: f64) -> (f64, f64) {
            let lat = LatLng::clamp_lat(lat);
            let lat_rad = lat.to_radians();
            let x = (lng + 180.0) / 360.0 * scale;
            let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * scale;
            (x, y)
        }

        fn from_world_px(x: f64, y: f64, scale: f64) -> LatLng {
            let lng = x / scale * 360.0 - 180.0;
            let merc_y = y / scale;
            let lat_rad = std::f64::consts::FRAC_PI_2
                - 2.0 * ((0.5 - merc_y) * 2.0 * std::f64::consts::PI).exp().atan();
            let lat = lat_rad.to_degrees();
            LatLng::new(lat, lng)
        }

        // Amount of world pixels visible horizontally/vertically from center to edge
        let half_x = self.size.x / 2.0;
        let half_y = self.size.y / 2.0;

        let (mut world_x, mut world_y) = to_world_px(center.lat, center.lng, world_scale);

        // Clamp X
        if world_scale > half_x * 2.0 {
            let min_x = half_x;
            let max_x = world_scale - half_x;
            world_x = world_x.clamp(min_x, max_x);
        } else {
            // Viewport is wider than world: pin X to middle of world
            world_x = world_scale / 2.0;
        }

        // Clamp Y (latitude)
        if world_scale > half_y * 2.0 {
            let min_y = half_y;
            let max_y = world_scale - half_y;
            world_y = world_y.clamp(min_y, max_y);
        } else {
            world_y = world_scale / 2.0;
        }

        from_world_px(world_x, world_y, world_scale)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(LatLng::new(0.0, 0.0), 1.0, Point::new(800.0, 600.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_creation() {
        let viewport = Viewport::new(
            LatLng::new(40.7128, -74.0060),
            10.0,
            Point::new(800.0, 600.0),
        );

        assert_eq!(viewport.zoom, 10.0);
        assert_eq!(viewport.center.lat, 40.7128);
        assert_eq!(viewport.size.x, 800.0);
    }

    #[test]
    fn test_coordinate_conversion() {
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 1.0, Point::new(512.0, 512.0));

        let center_pixel = Point::new(256.0, 256.0);
        let center_lat_lng = viewport.pixel_to_lat_lng(&center_pixel);

        // Should be approximately at the center (0, 0)
        assert!((center_lat_lng.lat - 0.0).abs() < 0.01);
        assert!((center_lat_lng.lng - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_zoom_limits() {
        let mut viewport = Viewport::default();
        viewport.set_zoom_limits(2.0, 15.0);

        viewport.set_zoom(1.0); // Below minimum
        assert_eq!(viewport.zoom, 2.0);

        viewport.set_zoom(20.0); // Above maximum
        assert_eq!(viewport.zoom, 15.0);
    }

    #[test]
    fn test_pan() {
        let mut viewport = Viewport::new(LatLng::new(0.0, 0.0), 1.0, Point::new(512.0, 512.0));

        let original_center = viewport.center;
        viewport.pan(Point::new(10.0, 10.0));

        // Center should have moved
        assert_ne!(viewport.center, original_center);
    }
}
