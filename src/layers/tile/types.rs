//! Core data types for tile layer functionality

use crate::{
    core::geo::{LatLng, Point, TileCoord},
    traits::{RetryLogic, should_retry_with_backoff},
    prelude::{HashMap, Arc},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TileLayerOptions {
    pub tile_size: u32, 
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub attribution: Option<String>,
    pub opacity: f32,
    pub z_index: i32,
    pub keep_buffer: u32,
    pub subdomains: Vec<String>,
    pub error_tile_url: Option<String>,
    pub cross_origin: bool,
    pub tms: bool,
    pub detect_retina: bool,
    pub reference_system: String,
    pub bounds: Option<crate::core::geo::LatLngBounds>,
}

impl Default for TileLayerOptions {
    fn default() -> Self {
        Self {
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 18,
            attribution: None,
            opacity: 1.0,
            z_index: 1,
            keep_buffer: 8, // Increased from 2 for much more aggressive prefetching
            subdomains: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            error_tile_url: None,
            cross_origin: false,
            tms: false,
            detect_retina: false,
            reference_system: "EPSG:3857".to_string(),
            bounds: None,
        }
    }
}

/// Represents a collection of tiles at a specific zoom level
/// Enhanced with CSS-style animation transform support (like Leaflet's level system)
#[derive(Debug)]
pub struct TileLevel {
    pub zoom: u8,
    pub tiles: HashMap<TileCoord, TileState>,
    pub origin: Point,
    pub scale: f64,
    pub translation: Point,
    
    /// CSS-style animation support (like Leaflet's transform animations)
    pub animating: bool,
    /// Transform matrix for smooth zoom animations
    pub transform_matrix: Option<[f64; 6]>, // CSS-style 2D transform matrix
    /// Animation start time for interpolation
    pub animation_start: Option<std::time::Instant>,
    /// Animation duration
    pub animation_duration: std::time::Duration,
    /// Target transform for ongoing animations
    pub target_transform: Option<(f64, Point)>, // (scale, translation)
    
    /// Leaflet-style level management
    /// Z-index for layering during animations (like Leaflet's CSS z-index)
    pub z_index: i32,
    /// Whether this level should be retained during zoom transitions
    pub retain: bool,
    /// Opacity for fade transitions (0.0 to 1.0)
    pub opacity: f32,
    /// Whether this level is actively being used for rendering
    pub active: bool,
}

impl TileLevel {
    pub fn new(zoom: u8) -> Self {
        Self {
            zoom,
            tiles: HashMap::default(),
            origin: Point::new(0.0, 0.0),
            scale: 1.0,
            translation: Point::new(0.0, 0.0),
            animating: false,
            transform_matrix: None,
            animation_start: None,
            animation_duration: std::time::Duration::from_millis(350),
            target_transform: None,
            z_index: 1,
            retain: false,
            opacity: 1.0,
            active: false,
        }
    }

    /// Set zoom transform for this level (Leaflet-style)
    /// This method applies CSS-style transforms to the entire tile level
    pub fn set_zoom_transform(
        &mut self,
        center: LatLng,
        _zoom: f64,
        viewport_center: LatLng,
        viewport_zoom: f64,
        viewport: &crate::core::viewport::Viewport,
    ) {
        // Calculate scale factor based on zoom difference
        let zoom_diff = viewport_zoom - self.zoom as f64;
        self.scale = 2_f64.powf(zoom_diff);
        
        // Calculate translation for centering
        let level_center = viewport.project(&center, Some(self.zoom as f64));
        let viewport_center_px = viewport.project(&viewport_center, Some(self.zoom as f64));
        
        self.translation = Point::new(
            (viewport_center_px.x - level_center.x) * self.scale,
            (viewport_center_px.y - level_center.y) * self.scale,
        );

        // Update transform matrix for CSS-style rendering
        self.update_transform_matrix();
        
        // Mark as animating if scale is significantly different from 1.0
        self.animating = (self.scale - 1.0).abs() > 0.01;
    }

    /// Update the CSS-style transform matrix
    fn update_transform_matrix(&mut self) {
        // Create a 2D transformation matrix: [a, b, c, d, e, f]
        // where the transform is: [a c e; b d f; 0 0 1]
        // This represents: translate(e, f) scale(a, d) with rotation support via b,c
        self.transform_matrix = Some([
            self.scale,      // a: x-scale
            0.0,             // b: y-skew 
            0.0,             // c: x-skew
            self.scale,      // d: y-scale
            self.translation.x, // e: x-translation
            self.translation.y, // f: y-translation
        ]);
    }

    /// Start a smooth animation to a target transform
    pub fn animate_to_transform(&mut self, target_scale: f64, target_translation: Point, duration: std::time::Duration) {
        self.target_transform = Some((target_scale, target_translation));
        self.animation_start = Some(std::time::Instant::now());
        self.animation_duration = duration;
        self.animating = true;
    }

    /// Update animation state and interpolate transforms
    pub fn update_animation(&mut self) -> bool {
        if let (Some(start_time), Some((target_scale, target_translation))) = 
            (self.animation_start, self.target_transform) {
            
            let elapsed = start_time.elapsed();
            if elapsed >= self.animation_duration {
                // Animation complete
                self.scale = target_scale;
                self.translation = target_translation;
                self.animating = false;
                self.animation_start = None;
                self.target_transform = None;
                self.update_transform_matrix();
                return false;
            }
            
            // Use unified easing function (ease-out cubic)
            let t = elapsed.as_secs_f64() / self.animation_duration.as_secs_f64();
            let eased_t = crate::layers::animation::ease_out_cubic(t);
            
            // Use unified interpolation functions
            let current_scale = self.scale;
            let current_translation = self.translation;
            
            self.scale = crate::layers::animation::lerp(current_scale, target_scale, eased_t);
            self.translation = Point::new(
                crate::layers::animation::lerp(current_translation.x, target_translation.x, eased_t),
                crate::layers::animation::lerp(current_translation.y, target_translation.y, eased_t),
            );
            
            self.update_transform_matrix();
            return true;
        }
        false
    }

    /// Transform screen bounds using the current animation state
    /// This enables CSS-style tile transformation during zoom animations
    pub fn transform_bounds(&self, bounds: (Point, Point)) -> (Point, Point) {
        if !self.animating || self.transform_matrix.is_none() {
            return bounds;
        }
        
        let matrix = self.transform_matrix.unwrap();
        let (min, max) = bounds;
        
        // Apply 2D transformation matrix to corner points
        let transformed_min = self.apply_transform_matrix(&matrix, min);
        let transformed_max = self.apply_transform_matrix(&matrix, max);
        
        // Calculate actual bounds from transformed corners
        let all_corners = [
            transformed_min,
            transformed_max,
            self.apply_transform_matrix(&matrix, Point::new(min.x, max.y)),
            self.apply_transform_matrix(&matrix, Point::new(max.x, min.y)),
        ];
        
        let final_min = Point::new(
            all_corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
            all_corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
        );
        
        let final_max = Point::new(
            all_corners.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
            all_corners.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
        );
        
        (final_min, final_max)
    }

    /// Apply 2D transformation matrix to a point using unified operations
    fn apply_transform_matrix(&self, matrix: &[f64; 6], point: Point) -> Point {
        use crate::traits::MatrixTransform;
        point.apply_transform(matrix)
    }

    /// Check if this level has any active animations
    pub fn is_animating(&self) -> bool {
        self.animating
    }

    /// Get the current transform matrix for CSS-style rendering
    pub fn get_transform_matrix(&self) -> Option<[f64; 6]> {
        self.transform_matrix
    }

    /// Reset all transforms to identity
    pub fn reset_transform(&mut self) {
        self.scale = 1.0;
        self.translation = Point::new(0.0, 0.0);
        self.animating = false;
        self.animation_start = None;
        self.target_transform = None;
        self.transform_matrix = None;
    }
    
    /// Leaflet-style level management methods
    
    /// Set the z-index for this level (higher values render on top)
    pub fn set_z_index(&mut self, z_index: i32) {
        self.z_index = z_index;
    }
    
    /// Mark this level as active (currently being rendered)
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    
    /// Set whether this level should be retained during zoom transitions
    pub fn set_retain(&mut self, retain: bool) {
        self.retain = retain;
    }
    
    /// Set the opacity for fade transitions
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    
    /// Check if this level should be retained
    pub fn should_retain(&self) -> bool {
        self.retain
    }
    
    /// Check if this level is active
    pub fn is_active(&self) -> bool {
        self.active
    }
    
    /// Get the current opacity
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }
    
    /// Get the z-index
    pub fn get_z_index(&self) -> i32 {
        self.z_index
    }
}

#[derive(Debug, Clone)]
pub struct TileState {
    pub coord: TileCoord,
    pub data: Option<Arc<Vec<u8>>>,
    pub loading: bool,
    pub error: Option<String>,
    pub current: bool,
    pub retain: bool,
    pub opacity: f32,
    pub loaded_time: Option<std::time::Instant>,
    pub retry_count: u32,
    pub last_retry_time: Option<std::time::Instant>,
    pub parent_data: Option<Arc<Vec<u8>>>,
    pub show_parent: bool,
}

impl TileState {
    pub fn new(coord: TileCoord) -> Self {
        Self {
            coord,
            data: None,
            loading: false,
            error: None,
            current: false,
            retain: false,
            opacity: 0.0,
            loaded_time: None,
            retry_count: 0,
            last_retry_time: None,
            parent_data: None,
            show_parent: false,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn has_display_data(&self) -> bool {
        self.data.is_some() || (self.show_parent && self.parent_data.is_some())
    }

    pub fn get_display_data(&self) -> Option<&Arc<Vec<u8>>> {
        if let Some(ref data) = self.data {
            Some(data)
        } else if self.show_parent {
            self.parent_data.as_ref()
        } else {
            None
        }
    }

    pub fn mark_loaded(&mut self, data: Arc<Vec<u8>>) {
        self.data = Some(data);
        self.loading = false;
        self.error = None;
        self.retry_count = 0;
        self.loaded_time = Some(std::time::Instant::now());
    }

    pub fn mark_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.retry_count += 1;
        self.last_retry_time = Some(std::time::Instant::now());
    }



    pub fn set_parent_data(&mut self, parent_data: Option<Arc<Vec<u8>>>) {
        self.show_parent = parent_data.is_some() && self.data.is_none();
        self.parent_data = parent_data;
    }
}

impl RetryLogic for TileState {
    fn should_retry(&self, max_retries: u32, retry_delay_ms: u64, exponential_backoff: bool) -> bool {
        should_retry_with_backoff(
            self.retry_count,
            self.last_retry_time,
            max_retries,
            retry_delay_ms,
            exponential_backoff,
        )
    }

    fn get_retry_count(&self) -> u32 {
        self.retry_count
    }

    fn get_last_retry_time(&self) -> Option<std::time::Instant> {
        self.last_retry_time
    }
}