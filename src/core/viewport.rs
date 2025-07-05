use crate::core::geo::{LatLng, LatLngBounds, Point};
use crate::traits::{PointMath, GeometryOps};
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
    /// Pixel origin for coordinate transformations (to avoid precision issues)
    pixel_origin: Option<Point>,
    /// Maximum bounds for the map
    max_bounds: Option<LatLngBounds>,
    /// Viscosity for bounds enforcement (0.0 = loose, 1.0 = solid)
    max_bounds_viscosity: f64,
    /// Current transform for zoom animations (CSS-style transforms)
    pub current_transform: Transform,
}

/// Transform state for animations (CSS-style transforms)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// Translation in pixels
    pub translate: Point,
    /// Scale factor (1.0 = no scaling)
    pub scale: f64,
    /// Transform origin point in pixels
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

    /// Interpolate between two transforms with easing (moved from animation.rs)
    pub fn lerp_with_easing(&self, other: &Transform, t: f64, easing: crate::layers::animation::EasingType) -> Transform {
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

impl Viewport {
    /// Creates a new viewport
    pub fn new(center: LatLng, zoom: f64, size: Point) -> Self {
        Self {
            center,
            zoom: zoom.clamp(0.0, 18.0),
            size,
            min_zoom: 0.0,
            max_zoom: 18.0,
            pixel_origin: None,
            max_bounds: None,
            max_bounds_viscosity: 0.0,
            current_transform: Transform::identity(),
        }
    }

    /// Sets the maximum bounds for the map
    pub fn set_max_bounds(&mut self, bounds: Option<LatLngBounds>, viscosity: Option<f64>) {
        self.max_bounds = bounds;
        self.max_bounds_viscosity = viscosity.unwrap_or(0.0).clamp(0.0, 1.0);
    }

    /// Sets the center of the viewport with bounds checking
    pub fn set_center(&mut self, center: LatLng) {
        self.center = self.clamp_center(center);
        self.update_pixel_origin();
    }

    /// Sets the zoom level, clamping to valid range
    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self.update_pixel_origin();
    }

    /// Sets the viewport size
    pub fn set_size(&mut self, size: Point) {
        self.size = size;
        self.update_pixel_origin();
    }

    /// Sets the zoom limits
    pub fn set_zoom_limits(&mut self, min_zoom: f64, max_zoom: f64) {
        self.min_zoom = min_zoom;
        self.max_zoom = max_zoom;
        self.zoom = self.zoom.clamp(min_zoom, max_zoom);
    }

    /// Sets the current transform for animations
    pub fn set_transform(&mut self, transform: Transform) {
        self.current_transform = transform;
    }

    /// Clears the current transform (sets to identity)
    pub fn clear_transform(&mut self) {
        self.current_transform = Transform::identity();
    }

    /// Gets the scale factor for the current zoom level
    pub fn scale(&self) -> f64 {
        2_f64.powf(self.zoom)
    }

    /// Projects a LatLng to world pixel coordinates at the given zoom level
    /// This is the unified Web Mercator projection implementation (EPSG:3857)
    pub fn project(&self, lat_lng: &LatLng, zoom: Option<f64>) -> Point {
        let z = zoom.unwrap_or(self.zoom);
        let scale = 256.0 * 2_f64.powf(z);

        // Standard Web Mercator projection (EPSG:3857)
        const EARTH_RADIUS: f64 = 6378137.0;
        let x = lat_lng.lng.to_radians() * EARTH_RADIUS;
        let y = ((std::f64::consts::PI / 4.0 + lat_lng.lat.to_radians() / 2.0).tan().ln()) * EARTH_RADIUS;
        
        // Convert from raw Mercator coordinates to pixel coordinates at the given zoom
        // Standard transformation: scale = 0.5 / (π * R), offset = 0.5
        let pixel_x = (x + std::f64::consts::PI * EARTH_RADIUS) / (2.0 * std::f64::consts::PI * EARTH_RADIUS) * scale;
        let pixel_y = (-y + std::f64::consts::PI * EARTH_RADIUS) / (2.0 * std::f64::consts::PI * EARTH_RADIUS) * scale;

        Point::new(pixel_x, pixel_y)
    }

    /// Unprojects world pixel coordinates back to LatLng at the given zoom level
    /// This is the unified Web Mercator inverse projection implementation (EPSG:3857)
    pub fn unproject(&self, pixel: &Point, zoom: Option<f64>) -> LatLng {
        let z = zoom.unwrap_or(self.zoom);
        let scale = 256.0 * 2_f64.powf(z);

        // Convert from pixel coordinates to raw Mercator coordinates
        let x = (pixel.x / scale) * (2.0 * std::f64::consts::PI * EARTH_RADIUS) - std::f64::consts::PI * EARTH_RADIUS;
        let y = std::f64::consts::PI * EARTH_RADIUS - (pixel.y / scale) * (2.0 * std::f64::consts::PI * EARTH_RADIUS);

        // Standard Web Mercator inverse projection
        const EARTH_RADIUS: f64 = 6378137.0;
        let lng = x / EARTH_RADIUS * 180.0 / std::f64::consts::PI;
        let lat = (2.0 * (y / EARTH_RADIUS).exp().atan() - std::f64::consts::PI / 2.0) * 180.0 / std::f64::consts::PI;

        LatLng::new(lat, lng)
    }

    /// Gets or calculates the pixel origin for this viewport
    /// This is used to keep pixel coordinates manageable and avoid precision issues
    pub fn get_pixel_origin(&self) -> Point {
        self.pixel_origin
            .unwrap_or_else(|| self.project(&self.center, None).floor())
    }

    /// Updates the pixel origin based on current center
    fn update_pixel_origin(&mut self) {
        self.pixel_origin = Some(self.project(&self.center, None).floor());
    }

    /// Converts a geographical coordinate to screen pixel coordinates (container relative)
    pub fn lat_lng_to_pixel(&self, lat_lng: &LatLng) -> Point {
        let layer_point = self.lat_lng_to_layer_point(lat_lng);
        self.layer_point_to_container_point(&layer_point)
    }

    /// Converts screen pixel coordinates back to geographical coordinates
    pub fn pixel_to_lat_lng(&self, pixel: &Point) -> LatLng {
        let layer_point = self.container_point_to_layer_point(pixel);
        self.layer_point_to_lat_lng(&layer_point)
    }

    /// Converts LatLng to layer point (relative to pixel origin)
    pub fn lat_lng_to_layer_point(&self, lat_lng: &LatLng) -> Point {
        let projected_point = self.project(lat_lng, None);
        projected_point.subtract(&self.get_pixel_origin())
    }

    /// Converts layer point back to LatLng
    pub fn layer_point_to_lat_lng(&self, point: &Point) -> LatLng {
        let projected_point = point.add(&self.get_pixel_origin());
        self.unproject(&projected_point, None)
    }

    /// Converts layer point to container point (screen coordinates)
    /// This method supports CSS-style transforms during animation
    pub fn layer_point_to_container_point(&self, point: &Point) -> Point {
        // Apply current transform (CSS transforms during animation)
        let mut result = Point::new(point.x + self.size.x / 2.0, point.y + self.size.y / 2.0);

        if !self.current_transform.is_identity() {
            // Apply scale around transform origin
            let origin = self.current_transform.origin;
            let translate = self.current_transform.translate;
            let scale = self.current_transform.scale;

            // Transform: translate to origin, scale, translate back, then apply translation
            result.x = (result.x - origin.x) * scale + origin.x + translate.x;
            result.y = (result.y - origin.y) * scale + origin.y + translate.y;
        }

        result
    }

    /// Converts container point to layer point
    /// This method supports CSS-style transforms during animation
    pub fn container_point_to_layer_point(&self, point: &Point) -> Point {
        let mut result = *point;

        // Reverse transform if active
        if !self.current_transform.is_identity() {
            let origin = self.current_transform.origin;
            let translate = self.current_transform.translate;
            let scale = self.current_transform.scale;

            // Reverse the transform
            result.x = (result.x - translate.x - origin.x) / scale + origin.x;
            result.y = (result.y - translate.y - origin.y) / scale + origin.y;
        }

        Point::new(result.x - self.size.x / 2.0, result.y - self.size.y / 2.0)
    }

    /// Pans the viewport by the given pixel offset with bounds checking
    /// This implements drag behavior with viscous bounds
    pub fn pan(&mut self, delta: Point) -> Point {
        let current_layer_point = self.lat_lng_to_layer_point(&self.center); // Actual current center in layer coordinates
        let mut new_layer_point = current_layer_point.subtract(&delta);

        // Apply bounds limiting if max_bounds is set
        if let Some(bounds) = &self.max_bounds {
            if self.max_bounds_viscosity > 0.0 {
                new_layer_point = self.limit_offset_to_bounds(new_layer_point, bounds);
            }
        }

        let new_center = self.layer_point_to_lat_lng(&new_layer_point);
        self.set_center(new_center);

        // Return the actual delta that was applied (may be limited by bounds)
        let actual_new_layer_point = self.lat_lng_to_layer_point(&self.center);
        actual_new_layer_point.subtract(&current_layer_point)
    }

    /// Limits an offset to stay within bounds (viscous bounds)
    fn limit_offset_to_bounds(&self, layer_point: Point, bounds: &LatLngBounds) -> Point {
        // Calculate the offset limit
        let nw =
            self.lat_lng_to_layer_point(&LatLng::new(bounds.north_east.lat, bounds.south_west.lng));
        let se =
            self.lat_lng_to_layer_point(&LatLng::new(bounds.south_west.lat, bounds.north_east.lng));

        let limit_min = Point::new(nw.x, nw.y);
        let limit_max = Point::new(se.x - self.size.x, se.y - self.size.y);

        let mut limited_point = layer_point;

        // Apply viscous limiting
        if layer_point.x < limit_min.x {
            limited_point.x = self.viscous_limit(layer_point.x, limit_min.x);
        }
        if layer_point.y < limit_min.y {
            limited_point.y = self.viscous_limit(layer_point.y, limit_min.y);
        }
        if layer_point.x > limit_max.x {
            limited_point.x = self.viscous_limit(layer_point.x, limit_max.x);
        }
        if layer_point.y > limit_max.y {
            limited_point.y = self.viscous_limit(layer_point.y, limit_max.y);
        }

        limited_point
    }

    /// Applies viscous resistance to boundary violations
    fn viscous_limit(&self, value: f64, threshold: f64) -> f64 {
        value - (value - threshold) * self.max_bounds_viscosity
    }

    /// Zooms the viewport to a specific level at a given point
    pub fn zoom_to(&mut self, zoom: f64, focus_point: Option<Point>) {
        let new_zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        let old_zoom = self.zoom;

        // No-op if zoom does not change significantly
        if (new_zoom - old_zoom).abs() < 0.001 {
            return;
        }

        if let Some(focus_screen) = focus_point {
            // Zoom around the provided focus point
            // Get the LatLng at the focus point before zoom
            let focus_latlng = self.pixel_to_lat_lng(&focus_screen);

            // Update zoom first
            self.zoom = new_zoom;
            self.update_pixel_origin();

            // Calculate where the focus point would be after zoom with the old center
            let new_focus_screen = self.lat_lng_to_pixel(&focus_latlng);

            // Calculate the offset needed to keep the focus point stationary
            let offset = new_focus_screen.subtract(&focus_screen);

            // Pan to compensate for the offset
            self.pan(offset);
        } else {
            // Simple zoom without focus point - just zoom to center
            self.zoom = new_zoom;
            self.update_pixel_origin();
        }
    }

    /// Smooth zoom animation method that handles intermediate zoom levels
    /// This method uses CSS-style transforms for smooth zoom animation
    pub fn animate_zoom_to(&mut self, target_zoom: f64, focus_point: Option<Point>, progress: f64) {
        if progress >= 1.0 {
            self.zoom_to(target_zoom, focus_point);
            self.clear_transform(); // Clear transform when animation is complete
            return;
        }

        let start_zoom = self.zoom;
        let zoom_diff = target_zoom - start_zoom;

        // Use unified eased progress for smoother animation
        let eased_progress = crate::layers::animation::ease_out_cubic(progress);
        let eased_zoom = start_zoom + (zoom_diff * eased_progress);

        // Calculate scale and translation for accurate animation
        let scale_factor = 2_f64.powf(eased_zoom - start_zoom);
        let origin = focus_point.unwrap_or(Point::new(self.size.x / 2.0, self.size.y / 2.0));
        
        // Calculate translation to keep focus point stationary during zoom
        let translation = if let Some(focus) = focus_point {
            let center_offset = focus.subtract(&Point::new(self.size.x / 2.0, self.size.y / 2.0));
            center_offset.multiply(1.0 - 1.0 / scale_factor)
        } else {
            Point::new(0.0, 0.0)
        };

        self.current_transform = Transform::new(
            translation,
            scale_factor,
            origin,
        );

        // Don't update the actual zoom until animation is complete
        // This keeps tile loading stable during animation
    }
    
    /// Apply transform-aware coordinate conversion
    /// This ensures coordinates are properly transformed during animations
    pub fn transform_aware_lat_lng_to_pixel(&self, lat_lng: &LatLng) -> Point {
        let layer_point = self.lat_lng_to_layer_point(lat_lng);
        self.layer_point_to_container_point(&layer_point)
    }
    
    /// Apply transform-aware reverse coordinate conversion
    pub fn transform_aware_pixel_to_lat_lng(&self, pixel: &Point) -> LatLng {
        let layer_point = self.container_point_to_layer_point(pixel);
        self.layer_point_to_lat_lng(&layer_point)
    }

    /// Gets the current viewport bounds in geographical coordinates
    pub fn bounds(&self) -> LatLngBounds {
        let nw_pixel = Point::new(0.0, 0.0);
        let se_pixel = Point::new(self.size.x, self.size.y);

        let nw = self.pixel_to_lat_lng(&nw_pixel);
        let se = self.pixel_to_lat_lng(&se_pixel);

        LatLngBounds::new(LatLng::new(se.lat, nw.lng), LatLng::new(nw.lat, se.lng))
    }

    /// Fits the viewport to contain the given bounds
    pub fn fit_bounds(&mut self, bounds: &LatLngBounds, padding: Option<f64>) {
        log::warn!("🚨 fit_bounds called! This might override the zoom level.");

        let padding = padding.unwrap_or(20.0);

        // Calculate the center
        self.center = bounds.center();

        // Calculate the required zoom level using proper projection
        let viewport_size = Point::new(self.size.x - 2.0 * padding, self.size.y - 2.0 * padding);

        // Project bounds to pixels at different zoom levels to find the best fit
        let mut best_zoom = self.min_zoom;

        for test_zoom in (self.min_zoom as i32)..=(self.max_zoom as i32) {
            let zoom = test_zoom as f64;

            let nw = self.project(
                &LatLng::new(bounds.north_east.lat, bounds.south_west.lng),
                Some(zoom),
            );
            let se = self.project(
                &LatLng::new(bounds.south_west.lat, bounds.north_east.lng),
                Some(zoom),
            );

            let bounds_width = (se.x - nw.x).abs();
            let bounds_height = (se.y - nw.y).abs();

            if bounds_width <= viewport_size.x && bounds_height <= viewport_size.y {
                best_zoom = zoom;
            } else {
                break;
            }
        }

        self.set_zoom(best_zoom);
        self.update_pixel_origin();
    }

    /// Gets the resolution in meters per pixel at the current zoom level
    pub fn resolution(&self) -> f64 {
        // Earth circumference at equator is ~40,075,000 meters
        // At zoom 0, the world is 256 pixels wide
        let earth_circumference = 40_075_016.0;
        let scale = self.scale();
        earth_circumference / (256.0 * scale)
    }

    /// Clamps center to world bounds or max_bounds if set
    fn clamp_center(&self, center: LatLng) -> LatLng {
        if let Some(bounds) = &self.max_bounds {
            LatLng::new(
                center
                    .lat
                    .clamp(bounds.south_west.lat, bounds.north_east.lat),
                center
                    .lng
                    .clamp(bounds.south_west.lng, bounds.north_east.lng),
            )
        } else {
            // Clamp to world bounds
            LatLng::new(
                center.lat.clamp(-85.0511287798, 85.0511287798),
                center.lng.clamp(-180.0, 180.0),
            )
        }
    }

    /// Get the current transform for rendering
    pub fn get_transform(&self) -> &Transform {
        &self.current_transform
    }

    /// Check if a transform is currently active
    pub fn has_active_transform(&self) -> bool {
        !self.current_transform.is_identity()
    }

    /// Get the maximum bounds for the map if set
    pub fn max_bounds(&self) -> Option<&LatLngBounds> {
        self.max_bounds.as_ref()
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(LatLng::new(0.0, 0.0), 0.0, Point::new(800.0, 600.0))
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
