use crate::{
    core::{bounds::Bounds, geo::Point, map::Map, viewport::Viewport},
    input::events::InputEvent,
    plugins::base::PluginTrait,
    prelude::{HashMap, HashSet},
    rendering::context::RenderContext,
    Result,
};

#[cfg(feature = "egui")]
use egui::Color32;

#[derive(Debug, Clone, PartialEq)]
pub enum DrawTool {
    Freehand,
    Line,
    Rectangle,
    Circle,
    Polygon,
    Marker,
    Text,
    Select,
    Delete,
}

#[derive(Debug, Clone)]
pub enum DrawState {
    Idle,
    Drawing {
        tool: DrawTool,
        points: Vec<Point>,
        start_point: Point,
    },
    Selecting {
        selection_rect: Bounds,
        start_point: Point,
    },
    Editing {
        shape_id: String,
        point_index: usize,
        original_point: Point,
    },
}

#[derive(Debug, Clone)]
pub struct DrawStyle {
    pub stroke_color: Color32,
    pub fill_color: Color32,
    pub stroke_width: f32,
    pub opacity: f32,
    pub filled: bool,
    pub dash_pattern: Option<Vec<f32>>,
}

impl Default for DrawStyle {
    fn default() -> Self {
        Self {
            stroke_color: Color32::from_rgb(255, 0, 0),
            fill_color: Color32::from_rgba_premultiplied(255, 0, 0, 64),
            stroke_width: 2.0,
            opacity: 1.0,
            filled: true,
            dash_pattern: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawnShape {
    pub id: String,
    pub tool: DrawTool,
    pub points: Vec<Point>,
    pub style: DrawStyle,
    pub metadata: HashMap<String, String>,
    pub selected: bool,
    pub visible: bool,
}

impl DrawnShape {
    pub fn new(id: String, tool: DrawTool, points: Vec<Point>, style: DrawStyle) -> Self {
        Self {
            id,
            tool,
            points,
            style,
            metadata: HashMap::default(),
            selected: false,
            visible: true,
        }
    }

    pub fn bounds(&self) -> Option<Bounds> {
        if self.points.is_empty() {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for point in &self.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Some(Bounds::new(
            Point::new(min_x, min_y),
            Point::new(max_x, max_y),
        ))
    }

    pub fn contains_point(&self, point: &Point) -> bool {
        match self.tool {
            DrawTool::Rectangle => {
                if let Some(bounds) = self.bounds() {
                    bounds.contains(point)
                } else {
                    false
                }
            }
            DrawTool::Circle => {
                if self.points.len() >= 2 {
                    let center = &self.points[0];
                    let radius_point = &self.points[1];
                    let radius = ((center.x - radius_point.x).powi(2)
                        + (center.y - radius_point.y).powi(2))
                    .sqrt();
                    let distance =
                        ((center.x - point.x).powi(2) + (center.y - point.y).powi(2)).sqrt();
                    distance <= radius
                } else {
                    false
                }
            }
            DrawTool::Polygon => {
                if self.points.len() < 3 {
                    return false;
                }

                let mut inside = false;
                let mut j = self.points.len() - 1;

                for i in 0..self.points.len() {
                    let pi = &self.points[i];
                    let pj = &self.points[j];

                    if ((pi.y > point.y) != (pj.y > point.y))
                        && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
                    {
                        inside = !inside;
                    }
                    j = i;
                }

                inside
            }
            _ => false,
        }
    }

    pub fn add_point(&mut self, point: Point) {
        self.points.push(point);
    }

    pub fn update_point(&mut self, index: usize, point: Point) -> Result<()> {
        if index < self.points.len() {
            self.points[index] = point;
            Ok(())
        } else {
            Err("Point index out of bounds".into())
        }
    }

    pub fn remove_point(&mut self, index: usize) -> Result<()> {
        if index < self.points.len() {
            self.points.remove(index);
            Ok(())
        } else {
            Err("Point index out of bounds".into())
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawConfig {
    pub default_style: DrawStyle,
    pub enabled: bool,
    pub snap_to_grid: bool,
    pub grid_size: f64,
    pub show_controls: bool,
    pub max_shapes: Option<usize>,
    pub allow_editing: bool,
    pub allow_deleting: bool,
}

impl Default for DrawConfig {
    fn default() -> Self {
        Self {
            default_style: DrawStyle::default(),
            enabled: true,
            snap_to_grid: false,
            grid_size: 10.0,
            show_controls: true,
            max_shapes: None,
            allow_editing: true,
            allow_deleting: true,
        }
    }
}

pub struct DrawPlugin {
    config: DrawConfig,
    state: DrawState,
    current_tool: DrawTool,
    shapes: HashMap<String, DrawnShape>,
    selected_shapes: HashSet<String>,
    active: bool,
    shape_counter: usize,
}

impl DrawPlugin {
    pub fn new() -> Self {
        Self {
            config: DrawConfig::default(),
            state: DrawState::Idle,
            current_tool: DrawTool::Freehand,
            shapes: HashMap::default(),
            selected_shapes: HashSet::default(),
            active: false,
            shape_counter: 0,
        }
    }

    pub fn with_config(config: DrawConfig) -> Self {
        Self {
            config,
            state: DrawState::Idle,
            current_tool: DrawTool::Freehand,
            shapes: HashMap::default(),
            selected_shapes: HashSet::default(),
            active: false,
            shape_counter: 0,
        }
    }

    pub fn set_tool(&mut self, tool: DrawTool) {
        self.current_tool = tool;
        self.state = DrawState::Idle;
    }

    pub fn current_tool(&self) -> &DrawTool {
        &self.current_tool
    }

    pub fn start_drawing(&mut self, start_point: Point) -> Result<()> {
        if !self.config.enabled || !self.active {
            return Ok(());
        }

        let snapped_point = if self.config.snap_to_grid {
            self.snap_to_grid(start_point)
        } else {
            start_point
        };

        self.state = DrawState::Drawing {
            tool: self.current_tool.clone(),
            points: vec![snapped_point],
            start_point: snapped_point,
        };

        Ok(())
    }

    /// Continue drawing
    pub fn continue_drawing(&mut self, point: Point) -> Result<()> {
        let snap_to_grid = self.config.snap_to_grid;
        let grid_size = self.config.grid_size;

        if let DrawState::Drawing { points, .. } = &mut self.state {
            let snapped_point = if snap_to_grid {
                Self::snap_to_grid_with_size(point, grid_size)
            } else {
                point
            };

            points.push(snapped_point);
        }

        Ok(())
    }

    /// Finish drawing
    pub fn finish_drawing(&mut self) -> Result<()> {
        if let DrawState::Drawing { tool, points, .. } = &self.state {
            if points.len() >= 2 || *tool == DrawTool::Marker {
                let shape_id = format!("shape_{}", self.shape_counter);
                self.shape_counter += 1;

                let shape = DrawnShape::new(
                    shape_id.clone(),
                    tool.clone(),
                    points.clone(),
                    self.config.default_style.clone(),
                );

                self.shapes.insert(shape_id, shape);
            }
        }

        self.state = DrawState::Idle;
        Ok(())
    }

    /// Cancel drawing
    pub fn cancel_drawing(&mut self) {
        self.state = DrawState::Idle;
    }

    /// Add a shape
    pub fn add_shape(&mut self, shape: DrawnShape) -> Result<()> {
        if let Some(max_shapes) = self.config.max_shapes {
            if self.shapes.len() >= max_shapes {
                return Err("Maximum number of shapes reached".into());
            }
        }

        self.shapes.insert(shape.id.clone(), shape);
        Ok(())
    }

    /// Remove a shape
    pub fn remove_shape(&mut self, shape_id: &str) -> Result<()> {
        if !self.config.allow_deleting {
            return Err("Deleting shapes is not allowed".into());
        }

        self.shapes.remove(shape_id);
        self.selected_shapes.remove(shape_id);
        Ok(())
    }

    /// Get a shape by ID
    pub fn get_shape(&self, shape_id: &str) -> Option<&DrawnShape> {
        self.shapes.get(shape_id)
    }

    /// Get a mutable reference to a shape by ID
    pub fn get_shape_mut(&mut self, shape_id: &str) -> Option<&mut DrawnShape> {
        self.shapes.get_mut(shape_id)
    }

    /// Select a shape
    pub fn select_shape(&mut self, shape_id: &str) -> Result<()> {
        self.selected_shapes.insert(shape_id.to_string());
        Ok(())
    }

    /// Deselect a shape
    pub fn deselect_shape(&mut self, shape_id: &str) -> Result<()> {
        self.selected_shapes.remove(shape_id);
        Ok(())
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_shapes.clear();
    }

    /// Get all shapes
    pub fn shapes(&self) -> &HashMap<String, DrawnShape> {
        &self.shapes
    }

    /// Get selected shapes
    pub fn selected_shapes(&self) -> Vec<&DrawnShape> {
        self.selected_shapes
            .iter()
            .filter_map(|id| self.shapes.get(id))
            .collect()
    }

    /// Clear all shapes
    pub fn clear_shapes(&mut self) {
        self.shapes.clear();
        self.selected_shapes.clear();
    }

    /// Set the configuration
    pub fn set_config(&mut self, config: DrawConfig) {
        self.config = config;
    }

    /// Get the configuration
    pub fn config(&self) -> &DrawConfig {
        &self.config
    }

    /// Set whether the plugin is active
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.state = DrawState::Idle;
        }
    }

    /// Check if the plugin is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Snap a point to the grid
    fn snap_to_grid(&self, point: Point) -> Point {
        if !self.config.snap_to_grid {
            return point;
        }

        let grid_size = self.config.grid_size;
        Point::new(
            (point.x / grid_size).round() * grid_size,
            (point.y / grid_size).round() * grid_size,
        )
    }

    /// Snap a point to the grid with a specific grid size
    fn snap_to_grid_with_size(point: Point, grid_size: f64) -> Point {
        Point::new(
            (point.x / grid_size).round() * grid_size,
            (point.y / grid_size).round() * grid_size,
        )
    }
}

impl PluginTrait for DrawPlugin {
    fn name(&self) -> &str {
        "Draw"
    }

    fn on_add(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn on_remove(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn handle_input(&mut self, _input: &InputEvent) -> Result<()> {
        // This will be called by the map, but we need the viewport
        // So we'll handle input in the render method instead
        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, context: &mut RenderContext, _viewport: &Viewport) -> Result<()> {
        if !self.active || !self.config.enabled {
            return Ok(());
        }

        // Render all shapes
        for shape in self.shapes.values() {
            if !shape.visible {
                continue;
            }

            match shape.tool {
                DrawTool::Line => {
                    if shape.points.len() >= 2 {
                        let line_style = crate::rendering::context::LineRenderStyle {
                            color: shape.style.stroke_color,
                            width: shape.style.stroke_width,
                            opacity: shape.style.opacity,
                            dash_pattern: shape.style.dash_pattern.clone().unwrap_or_default(),
                        };
                        context.render_line(&shape.points, &line_style)?;
                    }
                }
                DrawTool::Rectangle => {
                    if let Some(bounds) = shape.bounds() {
                        let exterior = vec![
                            bounds.min,
                            Point::new(bounds.max.x, bounds.min.y),
                            bounds.max,
                            Point::new(bounds.min.x, bounds.max.y),
                        ];
                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: shape.style.fill_color,
                            stroke_color: shape.style.stroke_color,
                            stroke_width: shape.style.stroke_width,
                            fill_opacity: if shape.style.filled {
                                shape.style.opacity
                            } else {
                                0.0
                            },
                            stroke_opacity: shape.style.opacity,
                        };
                        context.render_polygon(&exterior, &[], &polygon_style)?;
                    }
                }
                DrawTool::Circle => {
                    if shape.points.len() >= 2 {
                        let center = &shape.points[0];
                        let radius_point = &shape.points[1];
                        let radius = ((center.x - radius_point.x).powi(2)
                            + (center.y - radius_point.y).powi(2))
                        .sqrt();

                        // Approximate circle with polygon
                        let mut points = Vec::new();
                        let segments = 32;
                        for i in 0..segments {
                            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
                            points.push(Point::new(
                                center.x + radius * angle.cos(),
                                center.y + radius * angle.sin(),
                            ));
                        }

                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: shape.style.fill_color,
                            stroke_color: shape.style.stroke_color,
                            stroke_width: shape.style.stroke_width,
                            fill_opacity: if shape.style.filled {
                                shape.style.opacity
                            } else {
                                0.0
                            },
                            stroke_opacity: shape.style.opacity,
                        };
                        context.render_polygon(&points, &[], &polygon_style)?;
                    }
                }
                DrawTool::Polygon => {
                    if shape.points.len() >= 3 {
                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: shape.style.fill_color,
                            stroke_color: shape.style.stroke_color,
                            stroke_width: shape.style.stroke_width,
                            fill_opacity: if shape.style.filled {
                                shape.style.opacity
                            } else {
                                0.0
                            },
                            stroke_opacity: shape.style.opacity,
                        };
                        context.render_polygon(&shape.points, &[], &polygon_style)?;
                    }
                }
                DrawTool::Freehand => {
                    if shape.points.len() >= 2 {
                        let line_style = crate::rendering::context::LineRenderStyle {
                            color: shape.style.stroke_color,
                            width: shape.style.stroke_width,
                            opacity: shape.style.opacity,
                            dash_pattern: shape.style.dash_pattern.clone().unwrap_or_default(),
                        };
                        context.render_line(&shape.points, &line_style)?;
                    }
                }
                DrawTool::Marker => {
                    if !shape.points.is_empty() {
                        let point_style = crate::rendering::context::PointRenderStyle {
                            fill_color: shape.style.fill_color,
                            stroke_color: shape.style.stroke_color,
                            stroke_width: shape.style.stroke_width,
                            radius: 5.0,
                            opacity: shape.style.opacity,
                        };
                        context.render_point(&shape.points[0], &point_style)?;
                    }
                }
                _ => {}
            }

            // Render selection indicator
            if shape.selected {
                if let Some(bounds) = shape.bounds() {
                    let exterior = vec![
                        bounds.min,
                        Point::new(bounds.max.x, bounds.min.y),
                        bounds.max,
                        Point::new(bounds.min.x, bounds.max.y),
                    ];
                    let selection_style = crate::rendering::context::PolygonRenderStyle {
                        fill_color: Color32::from_rgba_premultiplied(0, 255, 255, 32),
                        stroke_color: Color32::from_rgb(0, 255, 255),
                        stroke_width: 2.0,
                        fill_opacity: 0.0,
                        stroke_opacity: 1.0,
                    };
                    context.render_polygon(&exterior, &[], &selection_style)?;
                }
            }
        }

        // Render current drawing
        if let DrawState::Drawing { tool, points, .. } = &self.state {
            match tool {
                DrawTool::Line => {
                    if points.len() >= 2 {
                        let line_style = crate::rendering::context::LineRenderStyle {
                            color: self.config.default_style.stroke_color,
                            width: self.config.default_style.stroke_width,
                            opacity: self.config.default_style.opacity,
                            dash_pattern: vec![5.0, 5.0], // Dashed for preview
                        };
                        context.render_line(points, &line_style)?;
                    }
                }
                DrawTool::Rectangle => {
                    if points.len() >= 2 {
                        let bounds = Bounds::new(points[0], points[1]);
                        let exterior = vec![
                            bounds.min,
                            Point::new(bounds.max.x, bounds.min.y),
                            bounds.max,
                            Point::new(bounds.min.x, bounds.max.y),
                        ];
                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: self.config.default_style.fill_color,
                            stroke_color: self.config.default_style.stroke_color,
                            stroke_width: self.config.default_style.stroke_width,
                            fill_opacity: if self.config.default_style.filled {
                                self.config.default_style.opacity
                            } else {
                                0.0
                            },
                            stroke_opacity: self.config.default_style.opacity,
                        };
                        context.render_polygon(&exterior, &[], &polygon_style)?;
                    }
                }
                DrawTool::Circle => {
                    if points.len() >= 2 {
                        let center = &points[0];
                        let radius_point = &points[1];
                        let radius = ((center.x - radius_point.x).powi(2)
                            + (center.y - radius_point.y).powi(2))
                        .sqrt();

                        // Approximate circle with polygon
                        let mut circle_points = Vec::new();
                        let segments = 32;
                        for i in 0..segments {
                            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
                            circle_points.push(Point::new(
                                center.x + radius * angle.cos(),
                                center.y + radius * angle.sin(),
                            ));
                        }

                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: self.config.default_style.fill_color,
                            stroke_color: self.config.default_style.stroke_color,
                            stroke_width: self.config.default_style.stroke_width,
                            fill_opacity: if self.config.default_style.filled {
                                self.config.default_style.opacity
                            } else {
                                0.0
                            },
                            stroke_opacity: self.config.default_style.opacity,
                        };
                        context.render_polygon(&circle_points, &[], &polygon_style)?;
                    }
                }
                DrawTool::Freehand => {
                    if points.len() >= 2 {
                        let line_style = crate::rendering::context::LineRenderStyle {
                            color: self.config.default_style.stroke_color,
                            width: self.config.default_style.stroke_width,
                            opacity: self.config.default_style.opacity,
                            dash_pattern: vec![5.0, 5.0], // Dashed for preview
                        };
                        context.render_line(points, &line_style)?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Default for DrawPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_plugin_creation() {
        let plugin = DrawPlugin::new();
        assert_eq!(plugin.name(), "Draw");
        assert!(!plugin.is_active());
    }

    #[test]
    fn test_shape_operations() {
        let mut plugin = DrawPlugin::new();
        plugin.set_active(true);

        let shape = DrawnShape::new(
            "test_shape".to_string(),
            DrawTool::Line,
            vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
            DrawStyle::default(),
        );

        plugin.add_shape(shape).unwrap();
        assert_eq!(plugin.shapes().len(), 1);

        plugin.remove_shape("test_shape").unwrap();
        assert_eq!(plugin.shapes().len(), 0);
    }

    #[test]
    fn test_shape_bounds() {
        let shape = DrawnShape::new(
            "test".to_string(),
            DrawTool::Rectangle,
            vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            DrawStyle::default(),
        );

        let bounds = shape.bounds().unwrap();
        assert_eq!(bounds.min.x, 0.0);
        assert_eq!(bounds.min.y, 0.0);
        assert_eq!(bounds.max.x, 10.0);
        assert_eq!(bounds.max.y, 10.0);
    }
}
