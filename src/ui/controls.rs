use crate::core::geo::LatLng;
use crate::core::viewport::Viewport;
use crate::prelude::HashMap;
use crate::Result;
use serde::{Deserialize, Serialize};

/// Different types of map controls
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControlType {
    /// Zoom in/out buttons
    Zoom,
    /// Layer visibility toggles
    Layers,
    /// Full screen toggle
    Fullscreen,
    /// Map type selector
    MapType,
    /// Navigation compass
    Compass,
    /// Scale bar
    ScaleBar,
    /// Search box
    Search,
    /// Drawing tools
    Drawing,
    /// Measurement tools
    Measurement,
    /// Location/GPS controls
    Location,
    /// Attribution display
    Attribution,
    /// Custom control
    Custom(String),
}

/// Position of controls on the map
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControlPosition {
    TopLeft,
    TopRight,
    TopCenter,
    BottomLeft,
    BottomRight,
    BottomCenter,
    LeftCenter,
    RightCenter,
    Custom { x: f32, y: f32 },
}

/// Base configuration for all controls
#[derive(Debug, Clone)]
pub struct ControlConfig {
    /// Whether the control is visible
    pub visible: bool,
    /// Position on the map
    pub position: ControlPosition,
    /// Margin from the edge
    pub margin: f32,
    /// Whether the control can be dragged
    pub draggable: bool,
    /// Custom CSS class for styling
    pub css_class: Option<String>,
    /// Z-index for layering
    pub z_index: i32,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            visible: true,
            position: ControlPosition::TopRight,
            margin: 10.0,
            draggable: false,
            css_class: None,
            z_index: 1000,
        }
    }
}

/// Zoom control configuration
#[derive(Debug, Clone)]
pub struct ZoomControlConfig {
    pub base: ControlConfig,
    /// Show zoom in button
    pub show_zoom_in: bool,
    /// Show zoom out button
    pub show_zoom_out: bool,
    /// Show current zoom level
    pub show_zoom_level: bool,
    /// Button size
    pub button_size: f32,
    /// Text for zoom in button
    pub zoom_in_text: String,
    /// Text for zoom out button
    pub zoom_out_text: String,
}

impl Default for ZoomControlConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig::default(),
            show_zoom_in: true,
            show_zoom_out: true,
            show_zoom_level: false,
            button_size: 30.0,
            zoom_in_text: "+".to_string(),
            zoom_out_text: "−".to_string(),
        }
    }
}

/// Layer control configuration
#[derive(Debug, Clone)]
pub struct LayerControlConfig {
    pub base: ControlConfig,
    /// Whether to show base layer options
    pub show_base_layers: bool,
    /// Whether to show overlay layers
    pub show_overlays: bool,
    /// Whether layers are collapsible
    pub collapsible: bool,
    /// Initially collapsed state
    pub collapsed: bool,
    /// Maximum height before scrolling
    pub max_height: f32,
}

impl Default for LayerControlConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopRight,
                margin: 50.0,
                ..ControlConfig::default()
            },
            show_base_layers: true,
            show_overlays: true,
            collapsible: true,
            collapsed: true,
            max_height: 300.0,
        }
    }
}

/// Navigation compass configuration
#[derive(Debug, Clone)]
pub struct CompassConfig {
    pub base: ControlConfig,
    /// Size of the compass
    pub size: f32,
    /// Whether to show north indicator
    pub show_north: bool,
    /// Whether compass is interactive (clickable to reset)
    pub interactive: bool,
}

impl Default for CompassConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopLeft,
                ..ControlConfig::default()
            },
            size: 50.0,
            show_north: true,
            interactive: true,
        }
    }
}

/// Scale bar configuration
#[derive(Debug, Clone)]
pub struct ScaleBarConfig {
    pub base: ControlConfig,
    /// Maximum width of the scale bar
    pub max_width: f32,
    /// Whether to show metric units
    pub metric: bool,
    /// Whether to show imperial units
    pub imperial: bool,
    /// Update threshold for scale changes
    pub update_when_idle: bool,
}

impl Default for ScaleBarConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::BottomLeft,
                ..ControlConfig::default()
            },
            max_width: 100.0,
            metric: true,
            imperial: false,
            update_when_idle: true,
        }
    }
}

/// Search control configuration
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub base: ControlConfig,
    /// Placeholder text
    pub placeholder: String,
    /// Maximum number of results to show
    pub max_results: usize,
    /// Minimum characters before searching
    pub min_chars: usize,
    /// Search delay in milliseconds
    pub search_delay: u32,
    /// Whether to search as user types
    pub live_search: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopLeft,
                margin: 60.0,
                ..ControlConfig::default()
            },
            placeholder: "Search...".to_string(),
            max_results: 10,
            min_chars: 3,
            search_delay: 300,
            live_search: true,
        }
    }
}

/// Drawing tools configuration
#[derive(Debug, Clone)]
pub struct DrawingToolsConfig {
    pub base: ControlConfig,
    /// Available drawing tools
    pub available_tools: Vec<DrawingTool>,
    /// Initially selected tool
    pub default_tool: Option<DrawingTool>,
    /// Whether tools are collapsible
    pub collapsible: bool,
}

/// Available drawing tools
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingTool {
    Marker,
    Line,
    Polygon,
    Rectangle,
    Circle,
    Text,
    Eraser,
}

impl Default for DrawingToolsConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopLeft,
                margin: 110.0,
                ..ControlConfig::default()
            },
            available_tools: vec![
                DrawingTool::Marker,
                DrawingTool::Line,
                DrawingTool::Polygon,
                DrawingTool::Rectangle,
                DrawingTool::Circle,
            ],
            default_tool: None,
            collapsible: true,
        }
    }
}

/// Measurement tools configuration
#[derive(Debug, Clone)]
pub struct MeasurementConfig {
    pub base: ControlConfig,
    /// Available measurement tools
    pub available_tools: Vec<MeasurementTool>,
    /// Units to display
    pub units: MeasurementUnits,
    /// Show area measurements
    pub show_area: bool,
    /// Show perimeter measurements
    pub show_perimeter: bool,
}

/// Available measurement tools
#[derive(Debug, Clone, PartialEq)]
pub enum MeasurementTool {
    Distance,
    Area,
    Bearing,
    Elevation,
}

/// Units for measurements
#[derive(Debug, Clone, PartialEq)]
pub enum MeasurementUnits {
    Metric,
    Imperial,
    Nautical,
    Auto,
}

impl Default for MeasurementConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopLeft,
                margin: 160.0,
                ..ControlConfig::default()
            },
            available_tools: vec![MeasurementTool::Distance, MeasurementTool::Area],
            units: MeasurementUnits::Auto,
            show_area: true,
            show_perimeter: true,
        }
    }
}

/// Location/GPS control configuration
#[derive(Debug, Clone)]
pub struct LocationConfig {
    pub base: ControlConfig,
    /// Whether to automatically track location
    pub auto_track: bool,
    /// Zoom level when location is found
    pub zoom_to_location: Option<f64>,
    /// Whether to show accuracy circle
    pub show_accuracy: bool,
    /// Timeout for location requests (ms)
    pub timeout: u32,
}

impl Default for LocationConfig {
    fn default() -> Self {
        Self {
            base: ControlConfig {
                position: ControlPosition::TopRight,
                margin: 60.0,
                ..ControlConfig::default()
            },
            auto_track: false,
            zoom_to_location: Some(15.0),
            show_accuracy: true,
            timeout: 10000,
        }
    }
}

/// Main map controls manager
pub struct MapControls {
    /// Zoom control
    pub zoom_control: Option<ZoomControl>,
    /// Layer control
    pub layer_control: Option<LayerControl>,
    /// Compass
    pub compass: Option<Compass>,
    /// Scale bar
    pub scale_bar: Option<ScaleBar>,
    /// Search control
    pub search: Option<SearchControl>,
    /// Drawing tools
    pub drawing_tools: Option<DrawingTools>,
    /// Measurement tools
    pub measurement: Option<Measurement>,
    /// Location control
    pub location: Option<LocationControl>,
    /// Custom controls
    pub custom_controls: HashMap<String, Box<dyn Control>>,
}

impl MapControls {
    pub fn new() -> Self {
        Self {
            zoom_control: Some(ZoomControl::new(ZoomControlConfig::default())),
            layer_control: None,
            compass: None,
            scale_bar: Some(ScaleBar::new(ScaleBarConfig::default())),
            search: None,
            drawing_tools: None,
            measurement: None,
            location: None,
            custom_controls: HashMap::default(),
        }
    }

    /// Enable zoom control
    pub fn with_zoom_control(mut self, config: ZoomControlConfig) -> Self {
        self.zoom_control = Some(ZoomControl::new(config));
        self
    }

    /// Enable layer control
    pub fn with_layer_control(mut self, config: LayerControlConfig) -> Self {
        self.layer_control = Some(LayerControl::new(config));
        self
    }

    /// Enable compass
    pub fn with_compass(mut self, config: CompassConfig) -> Self {
        self.compass = Some(Compass::new(config));
        self
    }

    /// Enable scale bar
    pub fn with_scale_bar(mut self, config: ScaleBarConfig) -> Self {
        self.scale_bar = Some(ScaleBar::new(config));
        self
    }

    /// Enable search
    pub fn with_search(mut self, config: SearchConfig) -> Self {
        self.search = Some(SearchControl::new(config));
        self
    }

    /// Enable drawing tools
    pub fn with_drawing_tools(mut self, config: DrawingToolsConfig) -> Self {
        self.drawing_tools = Some(DrawingTools::new(config));
        self
    }

    /// Enable measurement tools
    pub fn with_measurement(mut self, config: MeasurementConfig) -> Self {
        self.measurement = Some(Measurement::new(config));
        self
    }

    /// Enable location control
    pub fn with_location(mut self, config: LocationConfig) -> Self {
        self.location = Some(LocationControl::new(config));
        self
    }

    /// Add a custom control
    pub fn add_custom_control(&mut self, id: String, control: Box<dyn Control>) {
        self.custom_controls.insert(id, control);
    }

    /// Update all controls
    pub fn update(&mut self, viewport: &Viewport, delta_time: f32) -> Result<()> {
        if let Some(ref mut zoom_control) = self.zoom_control {
            zoom_control.update(viewport, delta_time)?;
        }
        if let Some(ref mut layer_control) = self.layer_control {
            layer_control.update(viewport, delta_time)?;
        }
        if let Some(ref mut compass) = self.compass {
            compass.update(viewport, delta_time)?;
        }
        if let Some(ref mut scale_bar) = self.scale_bar {
            scale_bar.update(viewport, delta_time)?;
        }
        if let Some(ref mut search) = self.search {
            search.update(viewport, delta_time)?;
        }
        if let Some(ref mut drawing_tools) = self.drawing_tools {
            drawing_tools.update(viewport, delta_time)?;
        }
        if let Some(ref mut measurement) = self.measurement {
            measurement.update(viewport, delta_time)?;
        }
        if let Some(ref mut location) = self.location {
            location.update(viewport, delta_time)?;
        }

        for control in self.custom_controls.values_mut() {
            control.update(viewport, delta_time)?;
        }

        Ok(())
    }
}

/// Trait for all map controls
pub trait Control {
    fn update(&mut self, viewport: &Viewport, delta_time: f32) -> Result<()>;
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn get_position(&self) -> &ControlPosition;
    fn set_position(&mut self, position: ControlPosition);
}

/// Zoom control implementation
pub struct ZoomControl {
    config: ZoomControlConfig,
    current_zoom: f64,
}

impl ZoomControl {
    pub fn new(config: ZoomControlConfig) -> Self {
        Self {
            config,
            current_zoom: 1.0,
        }
    }

    pub fn zoom_in_clicked(&self) -> bool {
        // This would be implemented based on UI framework
        false
    }

    pub fn zoom_out_clicked(&self) -> bool {
        // This would be implemented based on UI framework
        false
    }
}

impl Control for ZoomControl {
    fn update(&mut self, viewport: &Viewport, _delta_time: f32) -> Result<()> {
        self.current_zoom = viewport.zoom;
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Layer control implementation
pub struct LayerControl {
    config: LayerControlConfig,
    available_layers: Vec<LayerInfo>,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub is_base_layer: bool,
}

impl LayerControl {
    pub fn new(config: LayerControlConfig) -> Self {
        Self {
            config,
            available_layers: Vec::new(),
        }
    }

    pub fn add_layer(&mut self, layer: LayerInfo) {
        self.available_layers.push(layer);
    }

    pub fn toggle_layer(&mut self, layer_id: &str) {
        if let Some(layer) = self.available_layers.iter_mut().find(|l| l.id == layer_id) {
            layer.visible = !layer.visible;
        }
    }
}

impl Control for LayerControl {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Compass implementation
pub struct Compass {
    config: CompassConfig,
}

impl Compass {
    pub fn new(config: CompassConfig) -> Self {
        Self { config }
    }
}

impl Control for Compass {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        // Update bearing based on map rotation if supported
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Scale bar implementation
pub struct ScaleBar {
    config: ScaleBarConfig,
    scale_text: String,
}

impl ScaleBar {
    pub fn new(config: ScaleBarConfig) -> Self {
        Self {
            config,
            scale_text: "1 km".to_string(),
        }
    }

    fn calculate_scale(&mut self, viewport: &Viewport) {
        let resolution = viewport.resolution();

        // Calculate scale for different units
        if self.config.metric {
            let meters = self.config.max_width as f64 * resolution;
            if meters < 1000.0 {
                self.scale_text = format!("{:.0} m", meters);
            } else {
                self.scale_text = format!("{:.1} km", meters / 1000.0);
            }
        }
    }
}

impl Control for ScaleBar {
    fn update(&mut self, viewport: &Viewport, _delta_time: f32) -> Result<()> {
        self.calculate_scale(viewport);
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Search control implementation
pub struct SearchControl {
    config: SearchConfig,
    query: String,
    results: Vec<SearchResult>,
    searching: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub position: LatLng,
    pub description: Option<String>,
    pub category: Option<String>,
}

impl SearchControl {
    pub fn new(config: SearchConfig) -> Self {
        Self {
            config,
            query: String::new(),
            results: Vec::new(),
            searching: false,
        }
    }

    pub fn set_query(&mut self, query: String) {
        if query.len() >= self.config.min_chars {
            self.query = query;
            self.search();
        } else {
            self.query = query;
        }
    }

    fn search(&mut self) {
        self.searching = true;
        // Implement actual search logic here
        self.results.clear();
        self.searching = false;
    }
}

impl Control for SearchControl {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Drawing tools implementation
pub struct DrawingTools {
    config: DrawingToolsConfig,
    active_tool: Option<DrawingTool>,
}

impl DrawingTools {
    pub fn new(config: DrawingToolsConfig) -> Self {
        Self {
            active_tool: config.default_tool.clone(),
            config,
        }
    }

    pub fn set_active_tool(&mut self, tool: DrawingTool) {
        self.active_tool = Some(tool);
    }

    pub fn clear_active_tool(&mut self) {
        self.active_tool = None;
    }
}

impl Control for DrawingTools {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Measurement tools implementation
pub struct Measurement {
    config: MeasurementConfig,
    active_tool: Option<MeasurementTool>,
    current_measurement: Option<f64>,
}

impl Measurement {
    pub fn new(config: MeasurementConfig) -> Self {
        Self {
            config,
            active_tool: None,
            current_measurement: None,
        }
    }

    pub fn start_measurement(&mut self, tool: MeasurementTool) {
        self.active_tool = Some(tool);
        self.current_measurement = None;
    }

    pub fn end_measurement(&mut self) {
        self.active_tool = None;
        self.current_measurement = None;
    }
}

impl Control for Measurement {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

/// Location control implementation
pub struct LocationControl {
    config: LocationConfig,
    current_location: Option<LatLng>,
    tracking: bool,
}

impl LocationControl {
    pub fn new(config: LocationConfig) -> Self {
        Self {
            config,
            current_location: None,
            tracking: false,
        }
    }

    pub fn start_tracking(&mut self) {
        self.tracking = true;
        // Implement geolocation API calls
    }

    pub fn stop_tracking(&mut self) {
        self.tracking = false;
    }

    pub fn get_current_location(&self) -> Option<&LatLng> {
        self.current_location.as_ref()
    }
}

impl Control for LocationControl {
    fn update(&mut self, _viewport: &Viewport, _delta_time: f32) -> Result<()> {
        if self.tracking {
            // Update location if tracking is enabled
        }
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.config.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.config.base.visible = visible;
    }

    fn get_position(&self) -> &ControlPosition {
        &self.config.base.position
    }

    fn set_position(&mut self, position: ControlPosition) {
        self.config.base.position = position;
    }
}

impl Default for MapControls {
    fn default() -> Self {
        Self::new()
    }
}
