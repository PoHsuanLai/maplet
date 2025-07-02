use crate::{
    core::{
        geo::{LatLng, LatLngBounds, Point},
        viewport::Viewport,
    },
    layers::base::{LayerProperties, LayerTrait, LayerType},
    Result,
};

use crate::rendering::context::RenderContext;

#[cfg(feature = "egui")]
use egui::Color32;

use crate::prelude::HashMap;
use serde::{Deserialize, Serialize};

/// Serializable color type that can convert to/from egui::Color32
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SerializableColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<Color32> for SerializableColor {
    fn from(color: Color32) -> Self {
        Self {
            r: color.r(),
            g: color.g(),
            b: color.b(),
            a: color.a(),
        }
    }
}

impl From<SerializableColor> for Color32 {
    fn from(color: SerializableColor) -> Self {
        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
    }
}

impl SerializableColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// Style for point features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointStyle {
    /// Fill color
    pub fill_color: SerializableColor,
    /// Border color
    pub stroke_color: SerializableColor,
    /// Border width
    pub stroke_width: f32,
    /// Point radius
    pub radius: f32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self {
            fill_color: SerializableColor::rgb(255, 0, 0),
            stroke_color: SerializableColor::rgb(255, 255, 255),
            stroke_width: 2.0,
            radius: 5.0,
            opacity: 1.0,
        }
    }
}

/// Style for line features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    /// Line color
    pub color: SerializableColor,
    /// Line width
    pub width: f32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Line dash pattern (empty for solid line)
    pub dash_pattern: Vec<f32>,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            color: SerializableColor::rgb(0, 0, 255),
            width: 2.0,
            opacity: 1.0,
            dash_pattern: Vec::new(),
        }
    }
}

/// Style for polygon features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolygonStyle {
    /// Fill color
    pub fill_color: SerializableColor,
    /// Border color
    pub stroke_color: SerializableColor,
    /// Border width
    pub stroke_width: f32,
    /// Fill opacity (0.0 to 1.0)
    pub fill_opacity: f32,
    /// Stroke opacity (0.0 to 1.0)
    pub stroke_opacity: f32,
}

impl Default for PolygonStyle {
    fn default() -> Self {
        Self {
            fill_color: SerializableColor::new(0, 255, 0, 100),
            stroke_color: SerializableColor::rgb(0, 200, 0),
            stroke_width: 2.0,
            fill_opacity: 0.4,
            stroke_opacity: 1.0,
        }
    }
}

/// Combined style for all vector feature types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorFeatureStyle {
    Point(PointStyle),
    Line(LineStyle),
    Polygon(PolygonStyle),
}

/// Different types of vector features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VectorFeature {
    /// A point marker
    Point { position: LatLng, style: PointStyle },
    /// A line string (path)
    LineString {
        points: Vec<LatLng>,
        style: LineStyle,
    },
    /// A polygon with optional holes
    Polygon {
        exterior: Vec<LatLng>,
        holes: Vec<Vec<LatLng>>,
        style: PolygonStyle,
    },
    /// A multi-point collection
    MultiPoint {
        points: Vec<LatLng>,
        style: PointStyle,
    },
    /// A multi-line collection
    MultiLineString {
        lines: Vec<Vec<LatLng>>,
        style: LineStyle,
    },
    /// A multi-polygon collection
    MultiPolygon {
        polygons: Vec<(Vec<LatLng>, Vec<Vec<LatLng>>)>, // (exterior, holes)
        style: PolygonStyle,
    },
}

impl VectorFeature {
    /// Get the bounding box of this feature
    pub fn bounds(&self) -> LatLngBounds {
        match self {
            VectorFeature::Point { position, .. } => LatLngBounds::new(*position, *position),
            VectorFeature::LineString { points, .. } => Self::bounds_from_points(points),
            VectorFeature::Polygon {
                exterior, holes, ..
            } => {
                let mut bounds = Self::bounds_from_points(exterior);
                for hole in holes {
                    bounds = Self::union_bounds(&bounds, &Self::bounds_from_points(hole));
                }
                bounds
            }
            VectorFeature::MultiPoint { points, .. } => Self::bounds_from_points(points),
            VectorFeature::MultiLineString { lines, .. } => {
                let mut bounds: Option<LatLngBounds> = None;
                for line in lines {
                    let line_bounds = Self::bounds_from_points(line);
                    bounds = Some(match bounds {
                        Some(b) => b.union(&line_bounds),
                        None => line_bounds,
                    });
                }
                bounds.unwrap_or_else(|| {
                    LatLngBounds::new(LatLng::new(0.0, 0.0), LatLng::new(0.0, 0.0))
                })
            }
            VectorFeature::MultiPolygon { polygons, .. } => {
                let mut bounds: Option<LatLngBounds> = None;
                for (exterior, holes) in polygons {
                    let mut poly_bounds = Self::bounds_from_points(exterior);
                    for hole in holes {
                        poly_bounds =
                            Self::union_bounds(&poly_bounds, &Self::bounds_from_points(hole));
                    }
                    bounds = Some(match bounds {
                        Some(b) => b.union(&poly_bounds),
                        None => poly_bounds,
                    });
                }
                bounds.unwrap_or_else(|| {
                    LatLngBounds::new(LatLng::new(0.0, 0.0), LatLng::new(0.0, 0.0))
                })
            }
        }
    }

    fn bounds_from_points(points: &[LatLng]) -> LatLngBounds {
        LatLngBounds::from_points(points)
            .unwrap_or_else(|| LatLngBounds::new(LatLng::new(0.0, 0.0), LatLng::new(0.0, 0.0)))
    }

    fn union_bounds(bounds1: &LatLngBounds, bounds2: &LatLngBounds) -> LatLngBounds {
        bounds1.union(bounds2)
    }

    /// Check if this feature intersects with the given bounds
    pub fn intersects_bounds(&self, bounds: &LatLngBounds) -> bool {
        self.bounds().intersects(bounds)
    }
}

/// A feature with associated data and unique ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorFeatureData {
    /// Unique identifier
    pub id: String,
    /// The geometric feature
    pub feature: VectorFeature,
    /// Associated properties/data
    pub properties: HashMap<String, serde_json::Value>,
    /// Whether this feature is selected
    pub selected: bool,
    /// Whether this feature is highlighted (hover)
    pub highlighted: bool,
    /// Whether this feature is visible in the current viewport
    pub visible: bool,
    /// Style to use when feature is selected
    pub selected_style: Option<VectorFeatureStyle>,
    /// Style to use when feature is hovered
    pub hover_style: Option<VectorFeatureStyle>,
    /// Whether this feature is currently being hovered
    pub hovered: bool,
    /// Base style for the feature
    pub style: VectorFeatureStyle,
}

impl VectorFeatureData {
    /// Create a new feature with the given ID and geometry
    pub fn new(id: String, feature: VectorFeature) -> Self {
        let style = match &feature {
            VectorFeature::Point { style, .. } => VectorFeatureStyle::Point(style.clone()),
            VectorFeature::LineString { style, .. } => VectorFeatureStyle::Line(style.clone()),
            VectorFeature::Polygon { style, .. } => VectorFeatureStyle::Polygon(style.clone()),
            VectorFeature::MultiPoint { style, .. } => VectorFeatureStyle::Point(style.clone()),
            VectorFeature::MultiLineString { style, .. } => VectorFeatureStyle::Line(style.clone()),
            VectorFeature::MultiPolygon { style, .. } => VectorFeatureStyle::Polygon(style.clone()),
        };

        Self {
            id,
            feature,
            properties: HashMap::default(),
            selected: false,
            highlighted: false,
            visible: true,
            selected_style: None,
            hover_style: None,
            hovered: false,
            style,
        }
    }

    /// Add a property to this feature
    pub fn with_property<V: Into<serde_json::Value>>(mut self, key: String, value: V) -> Self {
        self.properties.insert(key, value.into());
        self
    }

    /// Get a property value
    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }

    /// Set a property value
    pub fn set_property<V: Into<serde_json::Value>>(&mut self, key: String, value: V) {
        self.properties.insert(key, value.into());
    }
}

/// Vector layer for displaying geometric features
pub struct VectorLayer {
    /// Base layer properties
    properties: LayerProperties,
    /// All features in this layer
    features: HashMap<String, VectorFeatureData>,
    /// Layer-wide style overrides
    default_point_style: PointStyle,
    default_line_style: LineStyle,
    default_polygon_style: PolygonStyle,
    /// Whether features can be selected
    selectable: bool,
    /// Currently selected feature IDs
    selected_features: Vec<String>,
    /// Spatial index for efficient querying (simplified)
    spatial_index: Vec<(String, LatLngBounds)>,
}

impl VectorLayer {
    /// Create a new vector layer
    pub fn new(id: String, name: String) -> Self {
        let properties = LayerProperties::new(id, name, LayerType::Vector);

        Self {
            properties,
            features: HashMap::default(),
            default_point_style: PointStyle::default(),
            default_line_style: LineStyle::default(),
            default_polygon_style: PolygonStyle::default(),
            selectable: true,
            selected_features: Vec::new(),
            spatial_index: Vec::new(),
        }
    }

    /// Add a feature to the layer
    pub fn add_feature(&mut self, feature: VectorFeatureData) {
        let bounds = feature.feature.bounds();
        self.spatial_index.push((feature.id.clone(), bounds));
        self.features.insert(feature.id.clone(), feature);
    }

    /// Remove a feature by ID
    pub fn remove_feature(&mut self, id: &str) -> Option<VectorFeatureData> {
        self.spatial_index.retain(|(fid, _)| fid != id);
        self.selected_features.retain(|fid| fid != id);
        self.features.remove(id)
    }

    /// Get a feature by ID
    pub fn get_feature(&self, id: &str) -> Option<&VectorFeatureData> {
        self.features.get(id)
    }

    /// Get a mutable reference to a feature by ID
    pub fn get_feature_mut(&mut self, id: &str) -> Option<&mut VectorFeatureData> {
        self.features.get_mut(id)
    }

    /// Get all features
    pub fn features(&self) -> &HashMap<String, VectorFeatureData> {
        &self.features
    }

    /// Get features that intersect with the given bounds
    pub fn features_in_bounds(&self, bounds: &LatLngBounds) -> Vec<&VectorFeatureData> {
        let mut result = Vec::new();

        for (id, feature_bounds) in &self.spatial_index {
            if bounds.intersects(feature_bounds) {
                if let Some(feature) = self.features.get(id) {
                    result.push(feature);
                }
            }
        }

        result
    }

    /// Find features at a specific point (with tolerance)
    pub fn features_at_point(&self, point: &LatLng, tolerance: f64) -> Vec<&VectorFeatureData> {
        let tolerance_bounds = LatLngBounds::new(
            LatLng::new(point.lat - tolerance, point.lng - tolerance),
            LatLng::new(point.lat + tolerance, point.lng + tolerance),
        );

        self.features_in_bounds(&tolerance_bounds)
    }

    /// Select a feature by ID
    pub fn select_feature(&mut self, id: &str, multi_select: bool) {
        if !multi_select {
            // Clear previous selections
            for feature_id in &self.selected_features {
                if let Some(feature) = self.features.get_mut(feature_id) {
                    feature.selected = false;
                }
            }
            self.selected_features.clear();
        }

        if let Some(feature) = self.features.get_mut(id) {
            feature.selected = true;
            if !self.selected_features.contains(&id.to_string()) {
                self.selected_features.push(id.to_string());
            }
        }
    }

    /// Deselect a feature by ID
    pub fn deselect_feature(&mut self, id: &str) {
        if let Some(feature) = self.features.get_mut(id) {
            feature.selected = false;
        }
        self.selected_features.retain(|fid| fid != id);
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        for feature_id in &self.selected_features {
            if let Some(feature) = self.features.get_mut(feature_id) {
                feature.selected = false;
            }
        }
        self.selected_features.clear();
    }

    /// Get selected feature IDs
    pub fn selected_features(&self) -> &[String] {
        &self.selected_features
    }

    /// Set default point style
    pub fn set_default_point_style(&mut self, style: PointStyle) {
        self.default_point_style = style;
    }

    /// Set default line style
    pub fn set_default_line_style(&mut self, style: LineStyle) {
        self.default_line_style = style;
    }

    /// Set default polygon style
    pub fn set_default_polygon_style(&mut self, style: PolygonStyle) {
        self.default_polygon_style = style;
    }

    /// Get feature count
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    /// Clear all features
    pub fn clear(&mut self) {
        self.features.clear();
        self.selected_features.clear();
        self.spatial_index.clear();
    }

    /// Create a point feature
    pub fn create_point_feature(
        id: String,
        position: LatLng,
        style: Option<PointStyle>,
    ) -> VectorFeatureData {
        VectorFeatureData::new(
            id,
            VectorFeature::Point {
                position,
                style: style.unwrap_or_default(),
            },
        )
    }

    /// Create a line feature
    pub fn create_line_feature(
        id: String,
        points: Vec<LatLng>,
        style: Option<LineStyle>,
    ) -> VectorFeatureData {
        VectorFeatureData::new(
            id,
            VectorFeature::LineString {
                points,
                style: style.unwrap_or_default(),
            },
        )
    }

    /// Create a polygon feature
    pub fn create_polygon_feature(
        id: String,
        exterior: Vec<LatLng>,
        style: Option<PolygonStyle>,
    ) -> VectorFeatureData {
        VectorFeatureData::new(
            id,
            VectorFeature::Polygon {
                exterior,
                holes: Vec::new(),
                style: style.unwrap_or_default(),
            },
        )
    }

    /// Get the bounds of all features in this layer
    pub fn get_layer_bounds(&self) -> Option<LatLngBounds> {
        let mut bounds: Option<LatLngBounds> = None;

        for feature in self.features.values() {
            let feature_bounds = feature.feature.bounds();
            bounds = Some(match bounds {
                None => feature_bounds,
                Some(b) => b.union(&feature_bounds),
            });
        }

        bounds
    }

    /// Update features based on current viewport
    pub async fn update_features(&mut self, viewport: &Viewport) -> Result<()> {
        // Get viewport bounds for culling
        let viewport_bounds = viewport.bounds();

        // Update feature visibility based on viewport bounds
        for (_, feature) in self.features.iter_mut() {
            let feature_bounds = feature.feature.bounds();
            // Simple bounds intersection check
            feature.visible = viewport_bounds.south_west.lat <= feature_bounds.north_east.lat
                && viewport_bounds.north_east.lat >= feature_bounds.south_west.lat
                && viewport_bounds.south_west.lng <= feature_bounds.north_east.lng
                && viewport_bounds.north_east.lng >= feature_bounds.south_west.lng;
        }

        Ok(())
    }

    /// Get the effective style for a feature
    fn get_effective_style<'a>(&self, feature: &'a VectorFeatureData) -> &'a VectorFeatureStyle {
        if feature.selected {
            feature.selected_style.as_ref().unwrap_or(&feature.style)
        } else if feature.hovered {
            feature.hover_style.as_ref().unwrap_or(&feature.style)
        } else {
            &feature.style
        }
    }

    /// Render a single feature
    fn render_feature(
        &self,
        context: &mut RenderContext,
        viewport: &Viewport,
        feature_data: &VectorFeatureData,
    ) -> Result<()> {
        use crate::rendering::context::{LineRenderStyle, PointRenderStyle, PolygonRenderStyle};

        let opacity_multiplier = self.opacity();
        let effective_style = self.get_effective_style(feature_data);

        match &feature_data.feature {
            VectorFeature::Point { position, .. } => {
                let screen_pos = viewport.lat_lng_to_pixel(position);
                if let VectorFeatureStyle::Point(style) = effective_style {
                    let render_style = PointRenderStyle {
                        fill_color: style.fill_color.into(),
                        stroke_color: style.stroke_color.into(),
                        stroke_width: style.stroke_width,
                        radius: style.radius,
                        opacity: style.opacity * opacity_multiplier,
                    };
                    context.render_point(&screen_pos, &render_style)?;
                }
            }
            VectorFeature::LineString { points, .. } => {
                let screen_points: Vec<Point> = points
                    .iter()
                    .map(|p| viewport.lat_lng_to_pixel(p))
                    .collect();
                if let VectorFeatureStyle::Line(style) = effective_style {
                    let render_style = LineRenderStyle {
                        color: style.color.into(),
                        width: style.width,
                        opacity: style.opacity * opacity_multiplier,
                        dash_pattern: style.dash_pattern.clone(),
                    };
                    context.render_line(&screen_points, &render_style)?;
                }
            }
            VectorFeature::Polygon {
                exterior, holes, ..
            } => {
                let screen_exterior: Vec<Point> = exterior
                    .iter()
                    .map(|p| viewport.lat_lng_to_pixel(p))
                    .collect();
                let screen_holes: Vec<Vec<Point>> = holes
                    .iter()
                    .map(|hole| hole.iter().map(|p| viewport.lat_lng_to_pixel(p)).collect())
                    .collect();
                if let VectorFeatureStyle::Polygon(style) = effective_style {
                    let render_style = PolygonRenderStyle {
                        fill_color: style.fill_color.into(),
                        stroke_color: style.stroke_color.into(),
                        stroke_width: style.stroke_width,
                        fill_opacity: style.fill_opacity * opacity_multiplier,
                        stroke_opacity: style.stroke_opacity * opacity_multiplier,
                    };
                    context.render_polygon(&screen_exterior, &screen_holes, &render_style)?;
                }
            }
            VectorFeature::MultiPoint { points, .. } => {
                if let VectorFeatureStyle::Point(style) = effective_style {
                    let render_style = PointRenderStyle {
                        fill_color: style.fill_color.into(),
                        stroke_color: style.stroke_color.into(),
                        stroke_width: style.stroke_width,
                        radius: style.radius,
                        opacity: style.opacity * opacity_multiplier,
                    };
                    for position in points {
                        let screen_pos = viewport.lat_lng_to_pixel(position);
                        context.render_point(&screen_pos, &render_style)?;
                    }
                }
            }
            VectorFeature::MultiLineString { lines, .. } => {
                if let VectorFeatureStyle::Line(style) = effective_style {
                    let render_style = LineRenderStyle {
                        color: style.color.into(),
                        width: style.width,
                        opacity: style.opacity * opacity_multiplier,
                        dash_pattern: style.dash_pattern.clone(),
                    };
                    for line in lines {
                        let screen_points: Vec<Point> =
                            line.iter().map(|p| viewport.lat_lng_to_pixel(p)).collect();
                        context.render_line(&screen_points, &render_style)?;
                    }
                }
            }
            VectorFeature::MultiPolygon { polygons, .. } => {
                if let VectorFeatureStyle::Polygon(style) = effective_style {
                    let render_style = PolygonRenderStyle {
                        fill_color: style.fill_color.into(),
                        stroke_color: style.stroke_color.into(),
                        stroke_width: style.stroke_width,
                        fill_opacity: style.fill_opacity * opacity_multiplier,
                        stroke_opacity: style.stroke_opacity * opacity_multiplier,
                    };
                    for (exterior, holes) in polygons {
                        let screen_exterior: Vec<Point> = exterior
                            .iter()
                            .map(|p| viewport.lat_lng_to_pixel(p))
                            .collect();
                        let screen_holes: Vec<Vec<Point>> = holes
                            .iter()
                            .map(|hole| hole.iter().map(|p| viewport.lat_lng_to_pixel(p)).collect())
                            .collect();
                        context.render_polygon(&screen_exterior, &screen_holes, &render_style)?;
                    }
                }
            }
        }

        Ok(())
    }
}


impl LayerTrait for VectorLayer {
    crate::impl_layer_trait!(VectorLayer, properties);

    fn bounds(&self) -> Option<LatLngBounds> {
        self.get_layer_bounds()
    }

    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.visible() {
            return Ok(());
        }

        // Update feature visibility based on viewport
        let _ = futures::executor::block_on(self.update_features(viewport));

        // Render visible features
        for feature_data in self.features.values() {
            if feature_data.visible {
                self.render_feature(context, viewport, feature_data)?;
            }
        }

        Ok(())
    }

    fn options(&self) -> serde_json::Value {
        serde_json::json!({
            "selectable": self.selectable,
            "feature_count": self.features.len()
        })
    }

    fn set_options(&mut self, _options: serde_json::Value) -> Result<()> {
        // TODO: Implement option setting for vector layer
        Ok(())
    }
}
mod tests {
    use super::*;

    #[test]
    fn test_vector_layer_creation() {
        let layer = VectorLayer::new("test".to_string(), "Test Vector Layer".to_string());
        assert_eq!(layer.id(), "test");
        assert_eq!(layer.name(), "Test Vector Layer");
        assert_eq!(layer.layer_type(), LayerType::Vector);
        assert_eq!(layer.feature_count(), 0);
    }

    #[test]
    fn test_feature_operations() {
        let mut layer = VectorLayer::new("test".to_string(), "Test".to_string());

        let point_feature = VectorLayer::create_point_feature(
            "point1".to_string(),
            LatLng::new(40.7128, -74.0060),
            None,
        );

        layer.add_feature(point_feature);
        assert_eq!(layer.feature_count(), 1);

        let feature = layer.get_feature("point1");
        assert!(feature.is_some());

        layer.remove_feature("point1");
        assert_eq!(layer.feature_count(), 0);
    }

    #[test]
    fn test_feature_selection() {
        let mut layer = VectorLayer::new("test".to_string(), "Test".to_string());

        let point_feature = VectorLayer::create_point_feature(
            "point1".to_string(),
            LatLng::new(40.7128, -74.0060),
            None,
        );

        layer.add_feature(point_feature);
        layer.select_feature("point1", false);

        assert_eq!(layer.selected_features().len(), 1);
        assert_eq!(layer.selected_features()[0], "point1");

        layer.clear_selection();
        assert_eq!(layer.selected_features().len(), 0);
    }

    #[test]
    fn test_feature_bounds() {
        let points = vec![
            LatLng::new(40.0, -74.0),
            LatLng::new(41.0, -73.0),
            LatLng::new(40.5, -73.5),
        ];

        let line_feature = VectorFeature::LineString {
            points,
            style: LineStyle::default(),
        };

        let bounds = line_feature.bounds();
        assert_eq!(bounds.south_west.lat, 40.0);
        assert_eq!(bounds.north_east.lat, 41.0);
        assert_eq!(bounds.south_west.lng, -74.0);
        assert_eq!(bounds.north_east.lng, -73.0);
    }

    #[test]
    fn test_serializable_color() {
        let color = SerializableColor::rgb(255, 128, 64);
        let egui_color: Color32 = color.into();
        let back_to_serializable: SerializableColor = egui_color.into();

        assert_eq!(color, back_to_serializable);
    }
}
