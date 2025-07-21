use crate::core::{geo::Point, viewport::Viewport};
use nalgebra::Matrix4;

/// 2D camera for map rendering - uses Leaflet-style coordinate transformations
/// instead of 3D projection matrices to avoid fish-eye distortion
pub struct Camera {
    /// Current position in world coordinates (projected LatLng)
    pub position: Point,
    /// Current zoom level
    pub zoom: f64,
    /// Viewport size in pixels
    pub viewport_size: Point,
    /// Whether matrices need updating
    dirty: bool,
    /// Identity matrix for compatibility with existing shader code
    identity_matrix: Matrix4<f32>,
}

impl Camera {
    /// Create a new camera
    pub fn new(position: Point, zoom: f64, viewport_size: Point) -> Self {
        Self {
            position,
            zoom,
            viewport_size,
            dirty: false,
            identity_matrix: Matrix4::identity(),
        }
    }

    /// Create camera from viewport
    pub fn from_viewport(viewport: &Viewport) -> Self {
        // Use the viewport's projected center position instead of raw LatLng
        let projected_center = viewport.project(&viewport.center, None);
        Self::new(projected_center, viewport.zoom, viewport.size)
    }

    /// Update camera position
    pub fn set_position(&mut self, position: Point) {
        if self.position.x != position.x || self.position.y != position.y {
            self.position = position;
            self.dirty = true;
        }
    }

    /// Update camera zoom
    pub fn set_zoom(&mut self, zoom: f64) {
        if (self.zoom - zoom).abs() > f64::EPSILON {
            self.zoom = zoom;
            self.dirty = true;
        }
    }

    /// Update viewport size
    pub fn set_viewport_size(&mut self, size: Point) {
        if self.viewport_size.x != size.x || self.viewport_size.y != size.y {
            self.viewport_size = size;
            self.dirty = true;
        }
    }

    /// Update from viewport
    pub fn update_from_viewport(&mut self, viewport: &Viewport) {
        // Use the viewport's projected center position
        let projected_center = viewport.project(&viewport.center, None);
        let mut changed = false;

        if self.position.x != projected_center.x || self.position.y != projected_center.y {
            self.position = projected_center;
            changed = true;
        }

        if (self.zoom - viewport.zoom).abs() > f64::EPSILON {
            self.zoom = viewport.zoom;
            changed = true;
        }

        if self.viewport_size.x != viewport.size.x || self.viewport_size.y != viewport.size.y {
            self.viewport_size = viewport.size;
            changed = true;
        }

        if changed {
            self.dirty = true;
        }
    }

    /// Get the current view-projection matrix (identity for 2D rendering)
    /// This maintains compatibility with existing shader code while using 2D coordinates
    pub fn view_projection_matrix(&mut self) -> &Matrix4<f32> {
        &self.identity_matrix
    }

    /// Get the view matrix (identity for 2D rendering)
    pub fn view_matrix(&mut self) -> &Matrix4<f32> {
        &self.identity_matrix
    }

    /// Get the projection matrix (identity for 2D rendering)
    pub fn projection_matrix(&mut self) -> &Matrix4<f32> {
        &self.identity_matrix
    }

    /// Get view-projection matrix as array for GPU upload (identity matrix)
    pub fn view_projection_array(&mut self) -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// Convert world coordinates to screen coordinates using Leaflet-style 2D transformation
    /// This eliminates fish-eye distortion by using direct coordinate mapping
    pub fn world_to_screen(&mut self, world_pos: Point) -> Point {
        // Calculate the offset from camera position
        let offset_x = world_pos.x - self.position.x;
        let offset_y = world_pos.y - self.position.y;

        // Convert to screen coordinates (center of viewport)
        let screen_x = self.viewport_size.x / 2.0 + offset_x;
        let screen_y = self.viewport_size.y / 2.0 - offset_y; // Flip Y for screen coordinates

        Point::new(screen_x, screen_y)
    }

    /// Convert screen coordinates to world coordinates using Leaflet-style 2D transformation
    pub fn screen_to_world(&mut self, screen_pos: Point) -> Point {
        // Convert from screen coordinates to world offset
        let offset_x = screen_pos.x - self.viewport_size.x / 2.0;
        let offset_y = self.viewport_size.y / 2.0 - screen_pos.y; // Flip Y for screen coordinates

        // Add offset to camera position
        Point::new(
            self.position.x + offset_x,
            self.position.y + offset_y,
        )
    }

    /// Get the scale factor for the current zoom level
    pub fn scale(&self) -> f64 {
        2_f64.powf(self.zoom)
    }

    /// Get camera frustum bounds in world coordinates
    pub fn get_frustum_bounds(&mut self) -> (Point, Point) {
        let half_width = self.viewport_size.x / 2.0;
        let half_height = self.viewport_size.y / 2.0;

        let min = Point::new(self.position.x - half_width, self.position.y - half_height);
        let max = Point::new(self.position.x + half_width, self.position.y + half_height);

        (min, max)
    }

    /// Check if a point is visible in the camera frustum using unified geometry operations
    pub fn is_point_visible(&mut self, point: Point) -> bool {
        use crate::traits::GeometryOps;
        let (min, max) = self.get_frustum_bounds();
        let frustum_bounds = crate::core::bounds::Bounds::new(min, max);
        frustum_bounds.contains_point(&point)
    }

    /// Pan the camera by a screen space offset
    pub fn pan(&mut self, screen_delta: Point) {
        self.set_position(Point::new(
            self.position.x + screen_delta.x,
            self.position.y - screen_delta.y, // Flip Y for screen coordinates
        ));
    }

    /// Zoom the camera while keeping a screen point fixed
    pub fn zoom_to_point(&mut self, new_zoom: f64, screen_point: Point) {
        let old_world_point = self.screen_to_world(screen_point);
        self.set_zoom(new_zoom);
        let new_world_point = self.screen_to_world(screen_point);

        let world_delta = Point::new(
            old_world_point.x - new_world_point.x,
            old_world_point.y - new_world_point.y,
        );

        self.set_position(Point::new(
            self.position.x + world_delta.x,
            self.position.y + world_delta.y,
        ));
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(Point::new(0.0, 0.0), 1.0, Point::new(800.0, 600.0))
    }
}
