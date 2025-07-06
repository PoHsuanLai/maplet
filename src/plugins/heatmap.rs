use crate::{
    core::{
        bounds::Bounds,
        geo::{LatLng, Point},
        map::Map,
        viewport::Viewport,
    },
    input::events::InputEvent,
    plugins::base::PluginTrait,
    Result,
};

use crate::rendering::context::RenderContext;

use crate::prelude::HashMap;
use crate::spatial::index::{SpatialIndex, SpatialItem};
#[cfg(feature = "egui")]
use egui::Color32;

#[derive(Debug, Clone)]
pub struct HeatmapPoint {
    pub position: LatLng,
    pub intensity: f64,
    pub metadata: HashMap<String, String>,
}

impl HeatmapPoint {
    pub fn new(position: LatLng, intensity: f64) -> Self {
        Self {
            position,
            intensity,
            metadata: HashMap::default(),
        }
    }

    pub fn with_metadata(
        position: LatLng,
        intensity: f64,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            position,
            intensity,
            metadata,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeatmapConfig {
    pub radius: f64,
    pub max_intensity: f64,
    pub min_intensity: f64,
    pub blur: f64,
    pub gradient: Vec<(f64, Color32)>,
    pub opacity: f32,
    pub visible: bool,
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self {
            radius: 25.0,
            max_intensity: 1.0,
            min_intensity: 0.0,
            blur: 15.0,
            gradient: vec![
                (0.0, Color32::from_rgba_premultiplied(0, 0, 255, 0)), // Transparent blue
                (0.2, Color32::from_rgba_premultiplied(0, 0, 255, 128)), // Blue
                (0.4, Color32::from_rgba_premultiplied(0, 255, 255, 128)), // Cyan
                (0.6, Color32::from_rgba_premultiplied(0, 255, 0, 128)), // Green
                (0.8, Color32::from_rgba_premultiplied(255, 255, 0, 128)), // Yellow
                (1.0, Color32::from_rgba_premultiplied(255, 0, 0, 255)), // Red
            ],
            opacity: 0.8,
            visible: true,
        }
    }
}

pub struct HeatmapPlugin {
    config: HeatmapConfig,
    data_points: Vec<HeatmapPoint>,
    spatial_index: SpatialIndex<HeatmapPoint>,
    cached_heatmap: Option<HeatmapCache>,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct HeatmapCache {
    viewport_bounds: Bounds,
    zoom_level: f64,
    grid_data: Vec<Vec<f64>>,
    grid_width: usize,
    grid_height: usize,
    cell_size: f64,
}

impl HeatmapPlugin {
    pub fn new() -> Self {
        Self {
            config: HeatmapConfig::default(),
            data_points: Vec::new(),
            spatial_index: SpatialIndex::new(),
            cached_heatmap: None,
            dirty: true,
        }
    }

    pub fn with_config(config: HeatmapConfig) -> Self {
        Self {
            config,
            data_points: Vec::new(),
            spatial_index: SpatialIndex::new(),
            cached_heatmap: None,
            dirty: true,
        }
    }

    pub fn add_point(&mut self, point: HeatmapPoint) -> Result<()> {
        let item = SpatialItem {
            id: format!("heatmap_point_{}", self.data_points.len()),
            bounds: crate::core::bounds::Bounds::from_center_and_size(
                crate::core::geo::Point::new(point.position.lng, point.position.lat),
                self.config.radius * 2.0,
                self.config.radius * 2.0,
            ),
            data: point.clone(),
        };

        self.spatial_index.insert(item)?;
        self.data_points.push(point);
        self.cached_heatmap = None;
        self.dirty = true;
        Ok(())
    }

    pub fn add_points(&mut self, points: Vec<HeatmapPoint>) -> Result<()> {
        let initial_len = self.data_points.len();
        self.data_points.reserve(points.len());

        for (i, point) in points.into_iter().enumerate() {
            let item = SpatialItem {
                id: format!("heatmap_point_{}", initial_len + i),
                bounds: crate::core::bounds::Bounds::from_center_and_size(
                    crate::core::geo::Point::new(point.position.lng, point.position.lat),
                    self.config.radius * 2.0,
                    self.config.radius * 2.0,
                ),
                data: point.clone(),
            };

            self.spatial_index.insert(item)?;
            self.data_points.push(point);
        }

        self.cached_heatmap = None;
        self.dirty = true;
        Ok(())
    }

    pub fn clear_points(&mut self) {
        self.data_points.clear();
        self.spatial_index.clear();
        self.cached_heatmap = None;
        self.dirty = true;
    }

    pub fn set_config(&mut self, config: HeatmapConfig) {
        self.config = config;
        self.cached_heatmap = None; // Invalidate cache
        self.dirty = true;
    }

    pub fn config(&self) -> &HeatmapConfig {
        &self.config
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.config.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.config.visible
    }

    pub fn point_count(&self) -> usize {
        self.data_points.len()
    }

    /// Optimized heatmap generation with spatial indexing and early termination
    fn generate_heatmap(&mut self, viewport: &Viewport) -> Result<()> {
        let viewport_latlng_bounds = viewport.bounds();
        let north_west = LatLng::new(
            viewport_latlng_bounds.north_east.lat,
            viewport_latlng_bounds.south_west.lng,
        );
        let south_east = LatLng::new(
            viewport_latlng_bounds.south_west.lat,
            viewport_latlng_bounds.north_east.lng,
        );
        let nw_pixel = viewport.lat_lng_to_pixel(&north_west);
        let se_pixel = viewport.lat_lng_to_pixel(&south_east);
        let viewport_bounds = crate::core::bounds::Bounds::new(nw_pixel, se_pixel);

        if let Some(ref cache) = self.cached_heatmap {
            if !self.dirty
                && cache.viewport_bounds.intersects(&viewport_bounds)
                && (cache.zoom_level - viewport.zoom).abs() < 0.1
            {
                return Ok(());
            }
        }

        let cell_size = self.config.radius / 2.0;
        let grid_width = (viewport_bounds.width() / cell_size).ceil() as usize;
        let grid_height = (viewport_bounds.height() / cell_size).ceil() as usize;

        // Pre-allocate with known capacity to avoid reallocations
        let mut grid_data = Vec::with_capacity(grid_height);
        for _ in 0..grid_height {
            grid_data.push(vec![0.0; grid_width]);
        }

        // OPTIMIZATION: Expand query bounds to include radius for accurate calculations
        let expanded_bounds = crate::core::bounds::Bounds::new(
            crate::core::geo::Point::new(
                viewport_bounds.min.x - self.config.radius,
                viewport_bounds.min.y - self.config.radius,
            ),
            crate::core::geo::Point::new(
                viewport_bounds.max.x + self.config.radius,
                viewport_bounds.max.y + self.config.radius,
            ),
        );

        let points = self.spatial_index.query(&expanded_bounds);

        // OPTIMIZATION: Pre-calculate blur factor to avoid repeated calculations
        let blur_factor = 2.0 * self.config.blur * self.config.blur;
        let radius_squared = self.config.radius * self.config.radius;

        // OPTIMIZATION: Process cells in batches and use early termination
        for row in 0..grid_height {
            for col in 0..grid_width {
                let cell_x = viewport_bounds.min.x + (col as f64 * cell_size);
                let cell_y = viewport_bounds.min.y + (row as f64 * cell_size);
                let cell_center = Point::new(cell_x + cell_size / 2.0, cell_y + cell_size / 2.0);

                let mut total_intensity = 0.0;

                // OPTIMIZATION: Early termination if we've found enough intensity
                for point_item in &points {
                    let point_pos =
                        Point::new(point_item.data.position.lng, point_item.data.position.lat);

                    // OPTIMIZATION: Use squared distance to avoid sqrt
                    let dx = cell_center.x - point_pos.x;
                    let dy = cell_center.y - point_pos.y;
                    let distance_squared = dx * dx + dy * dy;

                    if distance_squared <= radius_squared {
                        // OPTIMIZATION: Use pre-calculated blur factor
                        let influence = (-distance_squared / blur_factor).exp();
                        total_intensity += point_item.data.intensity * influence;

                        // OPTIMIZATION: Early termination if we've reached maximum intensity
                        if total_intensity >= self.config.max_intensity {
                            total_intensity = self.config.max_intensity;
                            break;
                        }
                    }
                }

                grid_data[row][col] = total_intensity;
            }
        }

        self.cached_heatmap = Some(HeatmapCache {
            viewport_bounds,
            zoom_level: viewport.zoom,
            grid_data,
            grid_width,
            grid_height,
            cell_size,
        });

        self.dirty = false;
        Ok(())
    }

    /// Optimized color calculation with pre-computed gradient lookup
    fn intensity_to_color(&self, intensity: f64) -> Color32 {
        if intensity <= self.config.min_intensity {
            return self.config.gradient[0].1;
        }
        if intensity >= self.config.max_intensity {
            return self.config.gradient.last().unwrap().1;
        }

        let normalized = (intensity - self.config.min_intensity)
            / (self.config.max_intensity - self.config.min_intensity);

        // OPTIMIZATION: Use binary search for large gradients
        if self.config.gradient.len() > 10 {
            let mut left = 0;
            let mut right = self.config.gradient.len() - 1;

            while left < right {
                let mid = (left + right) / 2;
                if self.config.gradient[mid].0 <= normalized {
                    left = mid + 1;
                } else {
                    right = mid;
                }
            }

            if left > 0 {
                let i = left - 1;
                let (t1, color1) = self.config.gradient[i];
                let (t2, color2) = self.config.gradient[i.min(self.config.gradient.len() - 1)];

                if t2 != t1 {
                    let t = (normalized - t1) / (t2 - t1);
                    return self.interpolate_color(color1, color2, t);
                }
            }
        } else {
            // OPTIMIZATION: Linear search for small gradients
            for i in 0..self.config.gradient.len() - 1 {
                let (t1, color1) = self.config.gradient[i];
                let (t2, color2) = self.config.gradient[i + 1];

                if normalized >= t1 && normalized <= t2 {
                    let t = if t2 != t1 {
                        (normalized - t1) / (t2 - t1)
                    } else {
                        0.0
                    };
                    return self.interpolate_color(color1, color2, t);
                }
            }
        }

        self.config.gradient.last().unwrap().1
    }

    /// Optimized color interpolation
    #[inline]
    fn interpolate_color(&self, color1: Color32, color2: Color32, t: f64) -> Color32 {
        let inv_t = 1.0 - t;
        Color32::from_rgba_premultiplied(
            (color1.r() as f64 * inv_t + color2.r() as f64 * t) as u8,
            (color1.g() as f64 * inv_t + color2.g() as f64 * t) as u8,
            (color1.b() as f64 * inv_t + color2.b() as f64 * t) as u8,
            ((color1.a() as f64 * inv_t + color2.a() as f64 * t) * self.config.opacity as f64)
                as u8,
        )
    }
}

impl PluginTrait for HeatmapPlugin {
    fn name(&self) -> &str {
        "Heatmap"
    }

    fn on_add(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn on_remove(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn handle_input(&mut self, _input: &InputEvent) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.config.visible || self.data_points.is_empty() {
            return Ok(());
        }

        self.generate_heatmap(viewport)?;

        if let Some(ref cache) = self.cached_heatmap {
            for row in 0..cache.grid_height {
                for col in 0..cache.grid_width {
                    let intensity = cache.grid_data[row][col];

                    if intensity > self.config.min_intensity {
                        let color = self.intensity_to_color(intensity);

                        let cell_x = cache.viewport_bounds.min.x + (col as f64 * cache.cell_size);
                        let cell_y = cache.viewport_bounds.min.y + (row as f64 * cache.cell_size);
                        let cell_center = Point::new(
                            cell_x + cache.cell_size / 2.0,
                            cell_y + cache.cell_size / 2.0,
                        );

                        let style = crate::rendering::context::PointRenderStyle {
                            fill_color: color,
                            stroke_color: color,
                            stroke_width: 0.0,
                            radius: (cache.cell_size / 2.0) as f32,
                            opacity: color.a() as f32 / 255.0,
                        };

                        context.render_point(&cell_center, &style)?;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for HeatmapPlugin {
    fn default() -> Self {
        Self::new()
    }
}
