use crate::core::geo::{LatLng, LatLngBounds, Point};
use crate::traits::{GeometryOps, PointMath};
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
    /// Map pane position for DOM-based dragging (like Leaflet's _mapPane position)
    map_pane_position: Point,
    /// Maximum bounds for the map
    max_bounds: Option<LatLngBounds>,
    /// Viscosity for bounds enforcement (0.0 = loose, 1.0 = solid)
    max_bounds_viscosity: f64,
    /// Current transform for zoom animations (CSS-style transforms)
    pub current_transform: Transform,
    /// Whether the viewport is currently being dragged
    is_dragging: bool,
    /// Transformation matrix for coordinate conversion (like Leaflet's transformation)
    transformation: Transformation,
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

/// Represents an affine transformation like Leaflet's Transformation class
/// Used for converting world coordinates to pixel coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transformation {
    /// Transformation coefficients (a, b, c, d) for transforming (x, y) to (a*x + b, c*y + d)
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

impl Transformation {
    /// Creates a new transformation with the given coefficients
    pub fn new(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self { a, b, c, d }
    }

    /// Transform a point using the transformation matrix
    pub fn transform(&self, point: &Point, scale: f64) -> Point {
        Point::new(
            scale * (self.a * point.x + self.b),
            scale * (self.c * point.y + self.d),
        )
    }

    /// Reverse transform a point using the transformation matrix
    pub fn untransform(&self, point: &Point, scale: f64) -> Point {
        Point::new(
            (point.x / scale - self.b) / self.a,
            (point.y / scale - self.d) / self.c,
        )
    }

    /// Create the standard Web Mercator transformation used by Leaflet
    pub fn web_mercator() -> Self {
        const EARTH_RADIUS: f64 = 6378137.0;
        let scale = 0.5 / (std::f64::consts::PI * EARTH_RADIUS);
        Self::new(scale, 0.5, -scale, 0.5)
    }
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
    pub fn lerp_with_easing(&self, other: &Transform, t: f64) -> Transform {
        let eased_t = crate::layers::animation::ease_out_cubic(t);
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

/// Implement Lerp trait for Transform to support animation interpolation
impl crate::traits::Lerp for Transform {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        Transform {
            translate: Point::new(
                self.translate.x + (other.translate.x - self.translate.x) * t,
                self.translate.y + (other.translate.y - self.translate.y) * t,
            ),
            scale: self.scale + (other.scale - self.scale) * t,
            origin: Point::new(
                self.origin.x + (other.origin.x - self.origin.x) * t,
                self.origin.y + (other.origin.y - self.origin.y) * t,
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
            map_pane_position: Point::new(0.0, 0.0),
            max_bounds: None,
            max_bounds_viscosity: 0.0,
            current_transform: Transform::identity(),
            is_dragging: false,
            transformation: Transformation::web_mercator(),
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
        // Force update pixel origin when clearing transform to prevent jumps
        self.force_update_pixel_origin();
    }

    /// Gets the scale factor for the current zoom level
    pub fn scale(&self) -> f64 {
        2_f64.powf(self.zoom)
    }

    /// Projects a LatLng to world coordinates using SphericalMercator projection (like Leaflet)
    /// This is the first step in Leaflet's projection pipeline
    pub fn project_to_world(&self, lat_lng: &LatLng) -> Point {
        // Leaflet's SphericalMercator projection - exact implementation
        const EARTH_RADIUS: f64 = 6378137.0;
        const MAX_LATITUDE: f64 = 85.0511287798;
        const D: f64 = std::f64::consts::PI / 180.0;

        let lat = lat_lng.lat.clamp(-MAX_LATITUDE, MAX_LATITUDE);
        let sin_lat = (lat * D).sin();

        Point::new(
            EARTH_RADIUS * lat_lng.lng * D,
            EARTH_RADIUS * ((1.0 + sin_lat) / (1.0 - sin_lat)).ln() / 2.0,
        )
    }

    /// Unprojects world coordinates back to LatLng using SphericalMercator (like Leaflet)
    /// This is the reverse of project_to_world
    pub fn unproject_from_world(&self, point: &Point) -> LatLng {
        // Leaflet's SphericalMercator unprojection - exact implementation
        const EARTH_RADIUS: f64 = 6378137.0;
        const D: f64 = 180.0 / std::f64::consts::PI;

        LatLng::new(
            (2.0 * (point.y / EARTH_RADIUS).exp().atan() - std::f64::consts::PI / 2.0) * D,
            point.x * D / EARTH_RADIUS,
        )
    }

    /// Projects a LatLng to pixel coordinates using Leaflet's two-step approach
    /// Step 1: Project to world coordinates, Step 2: Transform to pixels
    pub fn project(&self, lat_lng: &LatLng, zoom: Option<f64>) -> Point {
        let z = zoom.unwrap_or(self.zoom);
        let scale = 256.0 * 2_f64.powf(z);

        // Step 1: Project to world coordinates (SphericalMercator)
        let world_point = self.project_to_world(lat_lng);

        // Step 2: Transform to pixel coordinates using transformation matrix
        self.transformation.transform(&world_point, scale)
    }

    /// Unprojects pixel coordinates back to LatLng using Leaflet's two-step approach
    /// Step 1: Untransform from pixels to world coordinates, Step 2: Unproject to LatLng
    pub fn unproject(&self, pixel: &Point, zoom: Option<f64>) -> LatLng {
        let z = zoom.unwrap_or(self.zoom);
        let scale = 256.0 * 2_f64.powf(z);

        // Step 1: Untransform from pixel coordinates to world coordinates
        let world_point = self.transformation.untransform(pixel, scale);

        // Step 2: Unproject from world coordinates to LatLng (SphericalMercator)
        self.unproject_from_world(&world_point)
    }

    /// Gets or calculates the pixel origin for this viewport
    /// This is used to keep pixel coordinates manageable and avoid precision issues
    pub fn get_pixel_origin(&self) -> Point {
        self.pixel_origin
            .unwrap_or_else(|| self.project(&self.center, None).floor())
    }

    /// Updates the pixel origin based on current center
    /// Only updates if no active transform is present to avoid sudden jumps during animations
    fn update_pixel_origin(&mut self) {
        // CRITICAL FIX: Don't update pixel origin during active transforms
        // This prevents sudden jumps when animations complete or during dragging
        if !self.current_transform.is_identity() || self.is_dragging {
            return;
        }

        self.pixel_origin = Some(self.project(&self.center, None).floor());
    }

    /// Force updates the pixel origin regardless of transform state
    /// Use this when you need to ensure pixel origin is up to date
    fn force_update_pixel_origin(&mut self) {
        self.pixel_origin = Some(self.project(&self.center, None).floor());
    }

    /// Converts a geographical coordinate to screen pixel coordinates (container relative)
    /// This is the main method for converting LatLng to screen coordinates
    /// During dragging, this accounts for the map pane position offset
    pub fn lat_lng_to_container_point(&self, lat_lng: &LatLng) -> Point {
        let layer_point = self.lat_lng_to_layer_point(lat_lng);
        self.layer_point_to_container_point(&layer_point)
    }

    /// Converts screen pixel coordinates back to geographical coordinates
    /// This is the main method for converting screen coordinates to LatLng
    pub fn container_point_to_lat_lng(&self, pixel: &Point) -> LatLng {
        let layer_point = self.container_point_to_layer_point(pixel);
        self.layer_point_to_lat_lng(&layer_point)
    }

    /// Converts LatLng to layer point (relative to pixel origin)
    /// Layer points are the intermediate coordinate system used for precise calculations
    pub fn lat_lng_to_layer_point(&self, lat_lng: &LatLng) -> Point {
        let projected_point = self.project(lat_lng, None);
        projected_point.subtract(&self.get_pixel_origin())
    }

    /// Converts layer point back to LatLng
    /// This is the inverse of lat_lng_to_layer_point
    pub fn layer_point_to_lat_lng(&self, point: &Point) -> LatLng {
        let projected_point = point.add(&self.get_pixel_origin());
        self.unproject(&projected_point, None)
    }

    /// Legacy method names for compatibility
    pub fn lat_lng_to_pixel(&self, lat_lng: &LatLng) -> Point {
        self.lat_lng_to_container_point(lat_lng)
    }

    /// Legacy method names for compatibility
    pub fn pixel_to_lat_lng(&self, pixel: &Point) -> LatLng {
        self.container_point_to_lat_lng(pixel)
    }

    /// Converts layer point to container point (screen coordinates)
    /// This method supports CSS-style transforms during animation and DOM-based dragging
    /// CRITICAL FIX: Properly handle map pane position during dragging to eliminate fish-eye distortion
    pub fn layer_point_to_container_point(&self, point: &Point) -> Point {
        // Start with layer point offset by viewport center (like Leaflet)
        let mut result = Point::new(point.x + self.size.x / 2.0, point.y + self.size.y / 2.0);

        // Apply map pane position offset for DOM-based dragging (key to Leaflet's approach)
        // This is what prevents fish-eye distortion during dragging
        result = result.add(&self.map_pane_position);

        // Apply current transform (CSS transforms during animation)
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
    /// This method supports CSS-style transforms during animation and DOM-based dragging
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

        // Remove map pane position offset (reverse of layer_point_to_container_point)
        result = result.subtract(&self.map_pane_position);

        // Remove viewport center offset to get layer point
        Point::new(result.x - self.size.x / 2.0, result.y - self.size.y / 2.0)
    }

    /// Pans the viewport by the given pixel offset with bounds checking
    /// This implements Leaflet-style DOM-based dragging behavior
    pub fn pan(&mut self, delta: Point) -> Point {
        if self.is_dragging {
            // During dragging, just move the map pane (DOM-based like Leaflet)
            self.raw_pan_by(delta)
        } else {
            // When not dragging, update the actual center coordinates
            let current_layer_point = self.lat_lng_to_layer_point(&self.center);
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
    }

    /// Start dragging mode (like Leaflet's drag start)
    pub fn start_drag(&mut self) {
        self.is_dragging = true;
    }

    /// End dragging mode and update center coordinates based on map pane position
    pub fn end_drag(&mut self) {
        if self.is_dragging {
            self.is_dragging = false;

            // On drag end, set the new center to whatever is at the screen center (after drag)
            if self.map_pane_position.x != 0.0 || self.map_pane_position.y != 0.0 {
                let screen_center = Point::new(self.size.x / 2.0, self.size.y / 2.0);
                let new_center = self.container_point_to_lat_lng(&screen_center);
                self.set_center(new_center);
                self.map_pane_position = Point::new(0.0, 0.0);
            }

            // Force update pixel origin when dragging ends to prevent jumps
            self.force_update_pixel_origin();
        }
    }

    /// Raw pan by moving the map pane position (DOM-based dragging like Leaflet)
    /// This doesn't update the center coordinates until dragging ends
    pub fn raw_pan_by(&mut self, offset: Point) -> Point {
        // Move the map pane position (like Leaflet's _rawPanBy)
        // When dragging right, we want the content to move right, so we ADD the offset
        self.map_pane_position = self.map_pane_position.add(&offset);
        offset
    }

    /// Get the current map pane position (like Leaflet's _getMapPanePos)
    pub fn get_map_pane_position(&self) -> Point {
        self.map_pane_position
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
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
            self.clear_transform(); // This now properly updates pixel origin
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

        self.current_transform = Transform::new(translation, scale_factor, origin);

        // Don't update the actual zoom or pixel origin until animation is complete
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
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 10.0, Point::new(800.0, 600.0));
        assert_eq!(viewport.center.lat, 0.0);
        assert_eq!(viewport.center.lng, 0.0);
        assert_eq!(viewport.zoom, 10.0);
        assert_eq!(viewport.size.x, 800.0);
        assert_eq!(viewport.size.y, 600.0);
    }

    #[test]
    fn test_leaflet_compatible_projection() {
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 0.0, Point::new(256.0, 256.0));

        // Test center point (0,0) at zoom 0 should project to (128, 128) in a 256x256 viewport
        let center_point = viewport.project(&LatLng::new(0.0, 0.0), Some(0.0));

        // At zoom 0, the world is 256 pixels wide, so (0,0) should be at (128,128)
        assert!(
            (center_point.x - 128.0).abs() < 0.1,
            "Center X projection mismatch: {}",
            center_point.x
        );
        assert!(
            (center_point.y - 128.0).abs() < 0.1,
            "Center Y projection mismatch: {}",
            center_point.y
        );

        // Test reverse projection
        let reverse_latlng = viewport.unproject(&center_point, Some(0.0));
        assert!(
            (reverse_latlng.lat - 0.0).abs() < 0.0001,
            "Reverse lat mismatch: {}",
            reverse_latlng.lat
        );
        assert!(
            (reverse_latlng.lng - 0.0).abs() < 0.0001,
            "Reverse lng mismatch: {}",
            reverse_latlng.lng
        );

        // Test a known location: San Francisco (37.7749, -122.4194)
        let sf = LatLng::new(37.7749, -122.4194);
        let sf_projected = viewport.project(&sf, Some(10.0));
        let sf_reverse = viewport.unproject(&sf_projected, Some(10.0));

        // Should be able to round-trip accurately
        assert!(
            (sf_reverse.lat - sf.lat).abs() < 0.0001,
            "SF lat round-trip error: {} vs {}",
            sf_reverse.lat,
            sf.lat
        );
        assert!(
            (sf_reverse.lng - sf.lng).abs() < 0.0001,
            "SF lng round-trip error: {} vs {}",
            sf_reverse.lng,
            sf.lng
        );
    }

    #[test]
    fn test_spherical_mercator_bounds() {
        let viewport = Viewport::new(LatLng::new(0.0, 0.0), 0.0, Point::new(256.0, 256.0));

        // Test maximum latitude (should be clamped to 85.0511287798 like Leaflet)
        let max_lat = LatLng::new(90.0, 0.0); // Input 90 degrees
        let projected = viewport.project_to_world(&max_lat);
        let unprojected = viewport.unproject_from_world(&projected);

        // Should be clamped to Leaflet's max latitude
        assert!(
            (unprojected.lat - 85.0511287798).abs() < 0.0001,
            "Max latitude not properly clamped: {}",
            unprojected.lat
        );

        // Test minimum latitude
        let min_lat = LatLng::new(-90.0, 0.0);
        let projected = viewport.project_to_world(&min_lat);
        let unprojected = viewport.unproject_from_world(&projected);

        // Should be clamped to Leaflet's min latitude
        assert!(
            (unprojected.lat - (-85.0511287798)).abs() < 0.0001,
            "Min latitude not properly clamped: {}",
            unprojected.lat
        );
    }

    #[test]
    fn test_coordinate_conversion() {
        let viewport = Viewport::new(LatLng::new(51.505, -0.09), 13.0, Point::new(800.0, 600.0));

        let latlng = LatLng::new(51.505, -0.09);
        let pixel = viewport.lat_lng_to_container_point(&latlng);
        let converted_back = viewport.container_point_to_lat_lng(&pixel);

        assert!((converted_back.lat - latlng.lat).abs() < 0.0001);
        assert!((converted_back.lng - latlng.lng).abs() < 0.0001);
    }

    #[test]
    fn test_zoom_limits() {
        let mut viewport = Viewport::new(LatLng::new(0.0, 0.0), 5.0, Point::new(800.0, 600.0));
        viewport.set_zoom_limits(2.0, 15.0);

        viewport.set_zoom(1.0); // Below minimum
        assert_eq!(viewport.zoom, 2.0);

        viewport.set_zoom(20.0); // Above maximum
        assert_eq!(viewport.zoom, 15.0);

        viewport.set_zoom(10.0); // Within range
        assert_eq!(viewport.zoom, 10.0);
    }

    #[test]
    fn test_projection_stability_during_animation() {
        let mut viewport = Viewport::new(LatLng::new(0.0, 0.0), 10.0, Point::new(800.0, 600.0));

        // Get initial projection
        let test_point = LatLng::new(37.7749, -122.4194);
        let initial_projection = viewport.project(&test_point, None);

        // Simulate animation by setting a transform
        viewport.set_transform(Transform::new(
            Point::new(10.0, 10.0),
            2.0,
            Point::new(400.0, 300.0),
        ));

        // Projection should remain stable (pixel origin should not update during transform)
        let during_animation = viewport.project(&test_point, None);

        // The basic projection should be the same (only visual transform differs)
        assert!(
            (initial_projection.x - during_animation.x).abs() < 0.1,
            "Projection unstable during animation"
        );
        assert!(
            (initial_projection.y - during_animation.y).abs() < 0.1,
            "Projection unstable during animation"
        );

        // Clear transform
        viewport.clear_transform();

        // After clearing, projection should still be consistent
        let after_animation = viewport.project(&test_point, None);
        assert!(
            (initial_projection.x - after_animation.x).abs() < 0.1,
            "Projection changed after animation"
        );
        assert!(
            (initial_projection.y - after_animation.y).abs() < 0.1,
            "Projection changed after animation"
        );
    }

    #[test]
    fn test_pan() {
        let mut viewport =
            Viewport::new(LatLng::new(51.505, -0.09), 10.0, Point::new(800.0, 600.0));
        let initial_center = viewport.center;

        viewport.pan(Point::new(100.0, 50.0));

        // Center should have changed
        assert_ne!(viewport.center.lat, initial_center.lat);
        assert_ne!(viewport.center.lng, initial_center.lng);
    }
}
