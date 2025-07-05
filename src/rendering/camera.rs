use crate::core::{geo::Point, viewport::Viewport};
use nalgebra::{Matrix4, Point3, Vector3, Vector4};

/// 2D camera for map rendering
pub struct Camera {
    /// Current position in world coordinates
    pub position: Point,
    /// Current zoom level
    pub zoom: f64,
    /// Viewport size in pixels
    pub viewport_size: Point,
    /// Projection matrix
    projection_matrix: Matrix4<f32>,
    /// View matrix
    view_matrix: Matrix4<f32>,
    /// Combined view-projection matrix
    view_projection_matrix: Matrix4<f32>,
    /// Whether matrices need updating
    dirty: bool,
}

impl Camera {
    /// Create a new camera
    pub fn new(position: Point, zoom: f64, viewport_size: Point) -> Self {
        let mut camera = Self {
            position,
            zoom,
            viewport_size,
            projection_matrix: Matrix4::identity(),
            view_matrix: Matrix4::identity(),
            view_projection_matrix: Matrix4::identity(),
            dirty: true,
        };

        camera.update_matrices();
        camera
    }

    /// Create camera from viewport
    pub fn from_viewport(viewport: &Viewport) -> Self {
        let position = Point::new(viewport.center.lng, viewport.center.lat);
        Self::new(position, viewport.zoom, viewport.size)
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
        let position = Point::new(viewport.center.lng, viewport.center.lat);
        let mut changed = false;

        if self.position.x != position.x || self.position.y != position.y {
            self.position = position;
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

    /// Get the current view-projection matrix
    pub fn view_projection_matrix(&mut self) -> &Matrix4<f32> {
        if self.dirty {
            self.update_matrices();
        }
        &self.view_projection_matrix
    }

    /// Get the view matrix
    pub fn view_matrix(&mut self) -> &Matrix4<f32> {
        if self.dirty {
            self.update_matrices();
        }
        &self.view_matrix
    }

    /// Get the projection matrix
    pub fn projection_matrix(&mut self) -> &Matrix4<f32> {
        if self.dirty {
            self.update_matrices();
        }
        &self.projection_matrix
    }

    /// Get view-projection matrix as array for GPU upload
    pub fn view_projection_array(&mut self) -> [[f32; 4]; 4] {
        let matrix = self.view_projection_matrix();
        [
            [
                matrix[(0, 0)],
                matrix[(0, 1)],
                matrix[(0, 2)],
                matrix[(0, 3)],
            ],
            [
                matrix[(1, 0)],
                matrix[(1, 1)],
                matrix[(1, 2)],
                matrix[(1, 3)],
            ],
            [
                matrix[(2, 0)],
                matrix[(2, 1)],
                matrix[(2, 2)],
                matrix[(2, 3)],
            ],
            [
                matrix[(3, 0)],
                matrix[(3, 1)],
                matrix[(3, 2)],
                matrix[(3, 3)],
            ],
        ]
    }

    /// Convert world coordinates to screen coordinates
    pub fn world_to_screen(&mut self, world_pos: Point) -> Point {
        let view_proj = self.view_projection_matrix();
        let world_vec = Vector4::new(world_pos.x as f32, world_pos.y as f32, 0.0, 1.0);
        let clip_space = view_proj * world_vec;

        // Convert from clip space (-1 to 1) to screen space (0 to viewport size)
        let ndc_x = clip_space.x / clip_space.w;
        let ndc_y = clip_space.y / clip_space.w;

        let screen_x = (ndc_x + 1.0) * 0.5 * self.viewport_size.x as f32;
        let screen_y = (1.0 - ndc_y) * 0.5 * self.viewport_size.y as f32; // Flip Y

        Point::new(screen_x as f64, screen_y as f64)
    }

    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&mut self, screen_pos: Point) -> Point {
        // Convert screen space to normalized device coordinates
        let ndc_x = (screen_pos.x as f32 / self.viewport_size.x as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y as f32 / self.viewport_size.y as f32) * 2.0; // Flip Y

        // Get inverse view-projection matrix
        let view_proj = self.view_projection_matrix();
        if let Some(inv_view_proj) = view_proj.try_inverse() {
            let clip_space = Vector4::new(ndc_x, ndc_y, 0.0, 1.0);
            let world_space = inv_view_proj * clip_space;

            Point::new(
                world_space.x as f64 / world_space.w as f64,
                world_space.y as f64 / world_space.w as f64,
            )
        } else {
            // Fallback if matrix is not invertible
            self.position
        }
    }

    /// Get the scale factor for the current zoom level
    pub fn scale(&self) -> f64 {
        2_f64.powf(self.zoom)
    }

    /// Update internal matrices
    fn update_matrices(&mut self) {
        // Create orthographic projection matrix
        let scale = self.scale() as f32;
        let half_width = (self.viewport_size.x as f32) / (2.0 * scale);
        let half_height = (self.viewport_size.y as f32) / (2.0 * scale);

        let left = -half_width;
        let right = half_width;
        let bottom = -half_height;
        let top = half_height;
        let near = -1000.0;
        let far = 1000.0;

        self.projection_matrix = Matrix4::new_orthographic(left, right, bottom, top, near, far);

        // Create view matrix (translation to camera position)
        let eye = Point3::new(self.position.x as f32, self.position.y as f32, 0.0);
        let target = Point3::new(self.position.x as f32, self.position.y as f32, -1.0);
        let up = Vector3::new(0.0, 1.0, 0.0);

        self.view_matrix = Matrix4::look_at_rh(&eye, &target, &up);

        // Combine view and projection matrices
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;

        self.dirty = false;
    }

    /// Get camera frustum bounds in world coordinates
    pub fn get_frustum_bounds(&mut self) -> (Point, Point) {
        let scale = self.scale();
        let half_width = (self.viewport_size.x) / (2.0 * scale);
        let half_height = (self.viewport_size.y) / (2.0 * scale);

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
        let scale = self.scale();
        let world_delta = Point::new(
            screen_delta.x / scale,
            -screen_delta.y / scale, // Flip Y for screen coordinates
        );

        self.set_position(Point::new(
            self.position.x + world_delta.x,
            self.position.y + world_delta.y,
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
