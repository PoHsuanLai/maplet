use crate::{
    core::{
        bounds::Bounds,
        geo::{LatLng, Point},
        map::Map,
        viewport::Viewport,
    },
    input::events::InputEvent,
    layers::vector::VectorLayer,
    plugins::base::PluginTrait,
    Result,
};

#[cfg(feature = "render")]
use crate::rendering::context::RenderContext;

#[cfg(feature = "egui")]
use egui::Color32;

use std::collections::HashMap;

/// Measurement tool types
#[derive(Debug, Clone, PartialEq)]
pub enum MeasureTool {
    /// Distance measurement (line)
    Distance,
    /// Area measurement (polygon)
    Area,
    /// Angle measurement
    Angle,
    /// Clear all measurements
    Clear,
}

/// Measurement state
#[derive(Debug, Clone)]
pub enum MeasureState {
    /// Not measuring
    Idle,
    /// Measuring in progress
    Measuring {
        tool: MeasureTool,
        points: Vec<Point>,
        start_point: Point,
    },
}

/// Measurement result
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Unique identifier
    pub id: String,
    /// Measurement type
    pub tool: MeasureTool,
    /// Points used for measurement
    pub points: Vec<Point>,
    /// Calculated value
    pub value: f64,
    /// Unit of measurement
    pub unit: String,
    /// Display text
    pub display_text: String,
    /// Style for rendering
    pub style: MeasureStyle,
    /// Whether the measurement is visible
    pub visible: bool,
}

impl Measurement {
    /// Create a new measurement
    pub fn new(
        id: String,
        tool: MeasureTool,
        points: Vec<Point>,
        value: f64,
        unit: String,
    ) -> Self {
        let display_text = format!("{:.2} {}", value, unit);
        Self {
            id,
            tool,
            points,
            value,
            unit,
            display_text,
            style: MeasureStyle::default(),
            visible: true,
        }
    }

    /// Update the measurement value and display text
    pub fn update_value(&mut self, value: f64) {
        self.value = value;
        self.display_text = format!("{:.2} {}", value, self.unit);
    }
}

/// Style for measurements
#[derive(Debug, Clone)]
pub struct MeasureStyle {
    /// Line color
    pub line_color: Color32,
    /// Point color
    pub point_color: Color32,
    /// Text color
    pub text_color: Color32,
    /// Line width
    pub line_width: f32,
    /// Point radius
    pub point_radius: f32,
    /// Text size
    pub text_size: f32,
    /// Opacity
    pub opacity: f32,
    /// Whether to show points
    pub show_points: bool,
    /// Whether to show text
    pub show_text: bool,
}

impl Default for MeasureStyle {
    fn default() -> Self {
        Self {
            line_color: Color32::from_rgb(0, 150, 255),
            point_color: Color32::from_rgb(255, 255, 255),
            text_color: Color32::from_rgb(0, 0, 0),
            line_width: 2.0,
            point_radius: 4.0,
            text_size: 12.0,
            opacity: 1.0,
            show_points: true,
            show_text: true,
        }
    }
}

/// Configuration for the measure plugin
#[derive(Debug, Clone)]
pub struct MeasureConfig {
    /// Default style for measurements
    pub default_style: MeasureStyle,
    /// Whether measurement is enabled
    pub enabled: bool,
    /// Whether to show measurement controls
    pub show_controls: bool,
    /// Maximum number of measurements
    pub max_measurements: Option<usize>,
    /// Whether to snap to existing measurements
    pub snap_to_measurements: bool,
    /// Snap tolerance in pixels
    pub snap_tolerance: f64,
    /// Whether to show intermediate measurements
    pub show_intermediate: bool,
    /// Unit system to use
    pub unit_system: UnitSystem,
}

/// Unit system for measurements
#[derive(Debug, Clone, PartialEq)]
pub enum UnitSystem {
    /// Metric system (meters, kilometers)
    Metric,
    /// Imperial system (feet, miles)
    Imperial,
    /// Nautical system (nautical miles)
    Nautical,
}

impl Default for MeasureConfig {
    fn default() -> Self {
        Self {
            default_style: MeasureStyle::default(),
            enabled: true,
            show_controls: true,
            max_measurements: None,
            snap_to_measurements: false,
            snap_tolerance: 10.0,
            show_intermediate: true,
            unit_system: UnitSystem::Metric,
        }
    }
}

/// Measure plugin implementation
pub struct MeasurePlugin {
    /// Configuration
    config: MeasureConfig,
    /// Current measurement state
    state: MeasureState,
    /// Current measurement tool
    current_tool: MeasureTool,
    /// Measurements
    measurements: HashMap<String, Measurement>,
    /// Whether the plugin is active
    active: bool,
    /// Measurement counter for generating unique IDs
    measurement_counter: usize,
}

impl MeasurePlugin {
    /// Create a new measure plugin
    pub fn new() -> Self {
        Self {
            config: MeasureConfig::default(),
            state: MeasureState::Idle,
            current_tool: MeasureTool::Distance,
            measurements: HashMap::new(),
            active: false,
            measurement_counter: 0,
        }
    }

    /// Create a new measure plugin with configuration
    pub fn with_config(config: MeasureConfig) -> Self {
        Self {
            config,
            state: MeasureState::Idle,
            current_tool: MeasureTool::Distance,
            measurements: HashMap::new(),
            active: false,
            measurement_counter: 0,
        }
    }

    /// Set the current measurement tool
    pub fn set_tool(&mut self, tool: MeasureTool) {
        self.current_tool = tool.clone();
        self.state = MeasureState::Idle;

        if tool == MeasureTool::Clear {
            self.clear_measurements();
        }
    }

    /// Get the current measurement tool
    pub fn current_tool(&self) -> &MeasureTool {
        &self.current_tool
    }

    /// Start measuring
    pub fn start_measuring(&mut self, start_point: Point) -> Result<()> {
        if !self.config.enabled || !self.active {
            return Ok(());
        }

        self.state = MeasureState::Measuring {
            tool: self.current_tool.clone(),
            points: vec![start_point],
            start_point,
        };

        Ok(())
    }

    /// Continue measuring
    pub fn continue_measuring(&mut self, point: Point) -> Result<()> {
        if let MeasureState::Measuring { points, .. } = &mut self.state {
            points.push(point);
        }

        Ok(())
    }

    /// Finish measuring
    pub fn finish_measuring(&mut self) -> Result<()> {
        let should_create_measurement = if let MeasureState::Measuring { points, .. } = &self.state
        {
            points.len() >= 2
        } else {
            false
        };

        if should_create_measurement {
            if let MeasureState::Measuring { tool, points, .. } = &self.state {
                let measurement_id = format!("measurement_{}", self.measurement_counter);
                self.measurement_counter += 1;

                let tool_clone = tool.clone();
                let points_clone = points.clone();
                let (value, unit) = self.calculate_measurement(&tool_clone, &points_clone);
                let measurement = Measurement::new(
                    measurement_id.clone(),
                    tool_clone,
                    points_clone,
                    value,
                    unit,
                );

                self.measurements.insert(measurement_id, measurement);
            }
        }

        self.state = MeasureState::Idle;
        Ok(())
    }

    /// Cancel measuring
    pub fn cancel_measuring(&mut self) {
        self.state = MeasureState::Idle;
    }

    /// Calculate measurement value and unit
    fn calculate_measurement(&self, tool: &MeasureTool, points: &[Point]) -> (f64, String) {
        match tool {
            MeasureTool::Distance => {
                let distance = self.calculate_distance(points);
                match self.config.unit_system {
                    UnitSystem::Metric => {
                        if distance >= 1000.0 {
                            (distance / 1000.0, "km".to_string())
                        } else {
                            (distance, "m".to_string())
                        }
                    }
                    UnitSystem::Imperial => {
                        let feet = distance * 3.28084;
                        if feet >= 5280.0 {
                            (feet / 5280.0, "mi".to_string())
                        } else {
                            (feet, "ft".to_string())
                        }
                    }
                    UnitSystem::Nautical => {
                        let nautical_miles = distance / 1852.0;
                        (nautical_miles, "nm".to_string())
                    }
                }
            }
            MeasureTool::Area => {
                let area = self.calculate_area(points);
                match self.config.unit_system {
                    UnitSystem::Metric => {
                        if area >= 1_000_000.0 {
                            (area / 1_000_000.0, "km²".to_string())
                        } else {
                            (area, "m²".to_string())
                        }
                    }
                    UnitSystem::Imperial => {
                        let sq_feet = area * 10.7639;
                        if sq_feet >= 27_878_400.0 {
                            (sq_feet / 27_878_400.0, "mi²".to_string())
                        } else {
                            (sq_feet, "ft²".to_string())
                        }
                    }
                    UnitSystem::Nautical => {
                        let sq_nautical_miles = area / (1852.0 * 1852.0);
                        (sq_nautical_miles, "nm²".to_string())
                    }
                }
            }
            MeasureTool::Angle => {
                if points.len() >= 3 {
                    let angle = self.calculate_angle(&points[0], &points[1], &points[2]);
                    (angle, "°".to_string())
                } else {
                    (0.0, "°".to_string())
                }
            }
            _ => (0.0, "".to_string()),
        }
    }

    /// Calculate distance between points (in meters)
    fn calculate_distance(&self, points: &[Point]) -> f64 {
        if points.len() < 2 {
            return 0.0;
        }

        let mut total_distance = 0.0;
        for i in 0..points.len() - 1 {
            let p1 = &points[i];
            let p2 = &points[i + 1];

            // Convert lat/lng to meters using Haversine formula
            let lat1 = p1.y.to_radians();
            let lng1 = p1.x.to_radians();
            let lat2 = p2.y.to_radians();
            let lng2 = p2.x.to_radians();

            let dlat = lat2 - lat1;
            let dlng = lng2 - lng1;

            let a =
                (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
            let c = 2.0 * a.sqrt().asin();

            // Earth radius in meters
            let r = 6_371_000.0;
            total_distance += r * c;
        }

        total_distance
    }

    /// Calculate area of a polygon (in square meters)
    fn calculate_area(&self, points: &[Point]) -> f64 {
        if points.len() < 3 {
            return 0.0;
        }

        // Use the shoelace formula for polygon area
        let mut area = 0.0;
        let n = points.len();

        for i in 0..n {
            let j = (i + 1) % n;
            area += points[i].x * points[j].y;
            area -= points[j].x * points[i].y;
        }

        area.abs() / 2.0
    }

    /// Calculate angle between three points (in degrees)
    fn calculate_angle(&self, p1: &Point, p2: &Point, p3: &Point) -> f64 {
        let v1x = p1.x - p2.x;
        let v1y = p1.y - p2.y;
        let v2x = p3.x - p2.x;
        let v2y = p3.y - p2.y;

        let dot = v1x * v2x + v1y * v2y;
        let det = v1x * v2y - v1y * v2x;

        let angle = det.atan2(dot);
        angle.to_degrees()
    }

    /// Add a measurement
    pub fn add_measurement(&mut self, measurement: Measurement) -> Result<()> {
        if let Some(max_measurements) = self.config.max_measurements {
            if self.measurements.len() >= max_measurements {
                return Err("Maximum number of measurements reached".into());
            }
        }

        self.measurements
            .insert(measurement.id.clone(), measurement);
        Ok(())
    }

    /// Remove a measurement
    pub fn remove_measurement(&mut self, measurement_id: &str) -> Result<()> {
        self.measurements.remove(measurement_id);
        Ok(())
    }

    /// Get a measurement by ID
    pub fn get_measurement(&self, measurement_id: &str) -> Option<&Measurement> {
        self.measurements.get(measurement_id)
    }

    /// Get all measurements
    pub fn measurements(&self) -> &HashMap<String, Measurement> {
        &self.measurements
    }

    /// Clear all measurements
    pub fn clear_measurements(&mut self) {
        self.measurements.clear();
    }

    /// Set the configuration
    pub fn set_config(&mut self, config: MeasureConfig) {
        self.config = config;
    }

    /// Get the configuration
    pub fn config(&self) -> &MeasureConfig {
        &self.config
    }

    /// Set whether the plugin is active
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.state = MeasureState::Idle;
        }
    }

    /// Check if the plugin is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Handle input events
    fn handle_input(&mut self, event: &InputEvent, viewport: &Viewport) -> Result<()> {
        if !self.active || !self.config.enabled {
            return Ok(());
        }

        match event {
            InputEvent::Click { position } => {
                let lat_lng = viewport.pixel_to_lat_lng(position);
                let point = Point::new(lat_lng.lng, lat_lng.lat);

                match &self.state {
                    MeasureState::Idle => {
                        self.start_measuring(point)?;
                    }
                    MeasureState::Measuring { .. } => {
                        self.continue_measuring(point)?;
                        self.finish_measuring()?;
                    }
                }
            }
            InputEvent::MouseMove { position } => {
                if let MeasureState::Measuring { .. } = &self.state {
                    let lat_lng = viewport.pixel_to_lat_lng(position);
                    let point = Point::new(lat_lng.lng, lat_lng.lat);
                    self.continue_measuring(point)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl PluginTrait for MeasurePlugin {
    fn name(&self) -> &str {
        "Measure"
    }

    fn on_add(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn on_remove(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }

    fn handle_input(&mut self, input: &InputEvent) -> Result<()> {
        // This will be called by the map, but we need the viewport
        // So we'll handle input in the render method instead
        Ok(())
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.active || !self.config.enabled {
            return Ok(());
        }

        // Render all measurements
        for measurement in self.measurements.values() {
            if !measurement.visible {
                continue;
            }

            let line_style = crate::rendering::context::LineRenderStyle {
                color: measurement.style.line_color,
                width: measurement.style.line_width,
                opacity: measurement.style.opacity,
                dash_pattern: vec![],
            };

            match measurement.tool {
                MeasureTool::Distance | MeasureTool::Angle => {
                    if measurement.points.len() >= 2 {
                        context.render_line(&measurement.points, &line_style)?;
                    }
                }
                MeasureTool::Area => {
                    if measurement.points.len() >= 3 {
                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: measurement.style.line_color,
                            stroke_color: measurement.style.line_color,
                            stroke_width: measurement.style.line_width,
                            fill_opacity: 0.2,
                            stroke_opacity: measurement.style.opacity,
                        };
                        context.render_polygon(&measurement.points, &[], &polygon_style)?;
                    }
                }
                _ => {}
            }

            // Render points
            if measurement.style.show_points {
                for point in &measurement.points {
                    let point_style = crate::rendering::context::PointRenderStyle {
                        fill_color: measurement.style.point_color,
                        stroke_color: measurement.style.line_color,
                        stroke_width: 1.0,
                        radius: measurement.style.point_radius,
                        opacity: measurement.style.opacity,
                    };
                    context.render_point(point, &point_style)?;
                }
            }
        }

        // Render current measurement
        if let MeasureState::Measuring { tool, points, .. } = &self.state {
            let line_style = crate::rendering::context::LineRenderStyle {
                color: self.config.default_style.line_color,
                width: self.config.default_style.line_width,
                opacity: self.config.default_style.opacity,
                dash_pattern: vec![5.0, 5.0], // Dashed for preview
            };

            match tool {
                MeasureTool::Distance | MeasureTool::Angle => {
                    if points.len() >= 2 {
                        context.render_line(points, &line_style)?;
                    }
                }
                MeasureTool::Area => {
                    if points.len() >= 3 {
                        let polygon_style = crate::rendering::context::PolygonRenderStyle {
                            fill_color: self.config.default_style.line_color,
                            stroke_color: self.config.default_style.line_color,
                            stroke_width: self.config.default_style.line_width,
                            fill_opacity: 0.1,
                            stroke_opacity: self.config.default_style.opacity,
                        };
                        context.render_polygon(points, &[], &polygon_style)?;
                    }
                }
                _ => {}
            }

            // Render preview points
            if self.config.default_style.show_points {
                for point in points {
                    let point_style = crate::rendering::context::PointRenderStyle {
                        fill_color: self.config.default_style.point_color,
                        stroke_color: self.config.default_style.line_color,
                        stroke_width: 1.0,
                        radius: self.config.default_style.point_radius,
                        opacity: self.config.default_style.opacity,
                    };
                    context.render_point(point, &point_style)?;
                }
            }
        }

        Ok(())
    }
}

impl Default for MeasurePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_plugin_creation() {
        let plugin = MeasurePlugin::new();
        assert_eq!(plugin.name(), "Measure");
        assert!(!plugin.is_active());
    }

    #[test]
    fn test_distance_calculation() {
        let plugin = MeasurePlugin::new();
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(0.001, 0.0), // ~111 meters
        ];

        let (distance, unit) = plugin.calculate_measurement(&MeasureTool::Distance, &points);
        assert!(distance > 0.0);
        assert_eq!(unit, "m");
    }

    #[test]
    fn test_area_calculation() {
        let plugin = MeasurePlugin::new();
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(0.001, 0.0),
            Point::new(0.001, 0.001),
            Point::new(0.0, 0.001),
        ];

        let (area, unit) = plugin.calculate_measurement(&MeasureTool::Area, &points);
        assert!(area > 0.0);
        assert_eq!(unit, "m²");
    }
}
