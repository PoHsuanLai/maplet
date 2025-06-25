use crate::core::geo::{LatLng, LatLngBounds};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GeoJSON feature types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoJsonGeometry {
    Point {
        coordinates: [f64; 2],
    },
    LineString {
        coordinates: Vec<[f64; 2]>,
    },
    Polygon {
        coordinates: Vec<Vec<[f64; 2]>>,
    },
    MultiPoint {
        coordinates: Vec<[f64; 2]>,
    },
    MultiLineString {
        coordinates: Vec<Vec<[f64; 2]>>,
    },
    MultiPolygon {
        coordinates: Vec<Vec<Vec<[f64; 2]>>>,
    },
    GeometryCollection {
        geometries: Vec<GeoJsonGeometry>,
    },
}

/// GeoJSON feature with geometry and properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonFeature {
    pub id: Option<serde_json::Value>,
    pub geometry: Option<GeoJsonGeometry>,
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

/// Root GeoJSON object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoJson {
    Feature(GeoJsonFeature),
    FeatureCollection { features: Vec<GeoJsonFeature> },
    Geometry(GeoJsonGeometry),
}

/// Style information for rendering GeoJSON features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureStyle {
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub stroke_opacity: Option<f64>,
    pub fill: Option<String>,
    pub fill_opacity: Option<f64>,
    pub marker_color: Option<String>,
    pub marker_size: Option<String>,
    pub marker_symbol: Option<String>,
}

impl Default for FeatureStyle {
    fn default() -> Self {
        Self {
            stroke: Some("#3388ff".to_string()),
            stroke_width: Some(3.0),
            stroke_opacity: Some(1.0),
            fill: Some("#3388ff".to_string()),
            fill_opacity: Some(0.2),
            marker_color: Some("#3388ff".to_string()),
            marker_size: Some("medium".to_string()),
            marker_symbol: None,
        }
    }
}

/// GeoJSON layer for displaying geographic data
pub struct GeoJsonLayer {
    data: GeoJson,
    style: FeatureStyle,
    style_function: Option<Box<dyn Fn(&GeoJsonFeature) -> FeatureStyle>>,
    filter: Option<Box<dyn Fn(&GeoJsonFeature) -> bool>>,
}

impl GeoJsonLayer {
    /// Creates a new GeoJSON layer from raw JSON string
    pub fn from_str(geojson_str: &str) -> crate::Result<Self> {
        let data: GeoJson = serde_json::from_str(geojson_str)
            .map_err(|e| crate::Error::ParseError(format!("Invalid GeoJSON: {}", e)))?;

        Ok(Self {
            data,
            style: FeatureStyle::default(),
            style_function: None,
            filter: None,
        })
    }

    /// Creates a new GeoJSON layer from parsed GeoJSON
    pub fn new(data: GeoJson) -> Self {
        Self {
            data,
            style: FeatureStyle::default(),
            style_function: None,
            filter: None,
        }
    }

    /// Sets the default style for all features
    pub fn set_style(mut self, style: FeatureStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets a function to style features based on their properties
    pub fn set_style_function<F>(mut self, style_fn: F) -> Self
    where
        F: Fn(&GeoJsonFeature) -> FeatureStyle + 'static,
    {
        self.style_function = Some(Box::new(style_fn));
        self
    }

    /// Sets a filter function to show/hide features
    pub fn set_filter<F>(mut self, filter_fn: F) -> Self
    where
        F: Fn(&GeoJsonFeature) -> bool + 'static,
    {
        self.filter = Some(Box::new(filter_fn));
        self
    }

    /// Gets all features in the layer
    pub fn features(&self) -> Vec<&GeoJsonFeature> {
        let mut features = Vec::new();
        self.collect_features(&self.data, &mut features);

        if let Some(filter) = &self.filter {
            features.retain(|f| filter(f));
        }

        features
    }

    /// Gets the bounding box of all features
    pub fn bounds(&self) -> Option<LatLngBounds> {
        let features = self.features();
        if features.is_empty() {
            return None;
        }

        let mut bounds: Option<LatLngBounds> = None;

        for feature in features {
            if let Some(geometry) = &feature.geometry {
                if let Some(geom_bounds) = Self::geometry_bounds(geometry) {
                    if let Some(ref mut b) = bounds {
                        b.extend(&geom_bounds.south_west);
                        b.extend(&geom_bounds.north_east);
                    } else {
                        bounds = Some(geom_bounds);
                    }
                }
            }
        }

        bounds
    }

    /// Gets the style for a specific feature
    pub fn feature_style(&self, feature: &GeoJsonFeature) -> FeatureStyle {
        if let Some(style_fn) = &self.style_function {
            style_fn(feature)
        } else {
            self.style.clone()
        }
    }

    fn collect_features<'a>(&self, geojson: &'a GeoJson, features: &mut Vec<&'a GeoJsonFeature>) {
        match geojson {
            GeoJson::Feature(feature) => features.push(feature),
            GeoJson::FeatureCollection { features: f } => {
                for feature in f {
                    features.push(feature);
                }
            }
            GeoJson::Geometry(_) => {
                // Create a temporary feature for bare geometries
                // This is a limitation - we'd need to store it somewhere
            }
        }
    }

    fn geometry_bounds(geometry: &GeoJsonGeometry) -> Option<LatLngBounds> {
        match geometry {
            GeoJsonGeometry::Point { coordinates } => {
                let point = LatLng::new(coordinates[1], coordinates[0]);
                Some(LatLngBounds::new(point, point))
            }
            GeoJsonGeometry::LineString { coordinates } => Self::coords_bounds(coordinates),
            GeoJsonGeometry::Polygon { coordinates } => {
                if let Some(exterior) = coordinates.first() {
                    Self::coords_bounds(exterior)
                } else {
                    None
                }
            }
            GeoJsonGeometry::MultiPoint { coordinates } => Self::coords_bounds(coordinates),
            GeoJsonGeometry::MultiLineString { coordinates } => {
                let mut all_coords = Vec::new();
                for line in coordinates {
                    all_coords.extend(line);
                }
                Self::coords_bounds(&all_coords)
            }
            GeoJsonGeometry::MultiPolygon { coordinates } => {
                let mut all_coords = Vec::new();
                for polygon in coordinates {
                    if let Some(exterior) = polygon.first() {
                        all_coords.extend(exterior);
                    }
                }
                Self::coords_bounds(&all_coords)
            }
            GeoJsonGeometry::GeometryCollection { geometries } => {
                let mut bounds: Option<LatLngBounds> = None;
                for geom in geometries {
                    if let Some(geom_bounds) = Self::geometry_bounds(geom) {
                        if let Some(ref mut b) = bounds {
                            b.extend(&geom_bounds.south_west);
                            b.extend(&geom_bounds.north_east);
                        } else {
                            bounds = Some(geom_bounds);
                        }
                    }
                }
                bounds
            }
        }
    }

    fn coords_bounds(coordinates: &[[f64; 2]]) -> Option<LatLngBounds> {
        if coordinates.is_empty() {
            return None;
        }

        let first = LatLng::new(coordinates[0][1], coordinates[0][0]);
        let mut bounds = LatLngBounds::new(first, first);

        for coord in coordinates.iter().skip(1) {
            let point = LatLng::new(coord[1], coord[0]);
            bounds.extend(&point);
        }

        Some(bounds)
    }
}

impl GeoJsonGeometry {
    /// Converts coordinates to LatLng points
    pub fn to_lat_lng_points(&self) -> Vec<LatLng> {
        match self {
            GeoJsonGeometry::Point { coordinates } => {
                vec![LatLng::new(coordinates[1], coordinates[0])]
            }
            GeoJsonGeometry::LineString { coordinates } => coordinates
                .iter()
                .map(|c| LatLng::new(c[1], c[0]))
                .collect(),
            GeoJsonGeometry::Polygon { coordinates } => {
                if let Some(exterior) = coordinates.first() {
                    exterior.iter().map(|c| LatLng::new(c[1], c[0])).collect()
                } else {
                    Vec::new()
                }
            }
            GeoJsonGeometry::MultiPoint { coordinates } => coordinates
                .iter()
                .map(|c| LatLng::new(c[1], c[0]))
                .collect(),
            GeoJsonGeometry::MultiLineString { coordinates } => {
                let mut points = Vec::new();
                for line in coordinates {
                    for c in line {
                        points.push(LatLng::new(c[1], c[0]));
                    }
                }
                points
            }
            GeoJsonGeometry::MultiPolygon { coordinates } => {
                let mut points = Vec::new();
                for polygon in coordinates {
                    if let Some(exterior) = polygon.first() {
                        for c in exterior {
                            points.push(LatLng::new(c[1], c[0]));
                        }
                    }
                }
                points
            }
            GeoJsonGeometry::GeometryCollection { geometries } => {
                let mut points = Vec::new();
                for geom in geometries {
                    points.extend(geom.to_lat_lng_points());
                }
                points
            }
        }
    }

    /// Checks if the geometry contains a point
    pub fn contains_point(&self, point: &LatLng) -> bool {
        match self {
            GeoJsonGeometry::Point { coordinates } => {
                let geom_point = LatLng::new(coordinates[1], coordinates[0]);
                (geom_point.lat - point.lat).abs() < 1e-10
                    && (geom_point.lng - point.lng).abs() < 1e-10
            }
            GeoJsonGeometry::Polygon { coordinates } => {
                if let Some(exterior) = coordinates.first() {
                    Self::point_in_polygon(point, exterior)
                } else {
                    false
                }
            }
            GeoJsonGeometry::LineString { coordinates } => {
                // Check if point is on the line (simplified check)
                Self::point_on_line(point, coordinates)
            }
            GeoJsonGeometry::MultiPoint { coordinates } => coordinates.iter().any(|c| {
                let geom_point = LatLng::new(c[1], c[0]);
                (geom_point.lat - point.lat).abs() < 1e-10
                    && (geom_point.lng - point.lng).abs() < 1e-10
            }),
            GeoJsonGeometry::MultiLineString { coordinates } => coordinates
                .iter()
                .any(|line| Self::point_on_line(point, line)),
            GeoJsonGeometry::MultiPolygon { coordinates } => coordinates.iter().any(|polygon| {
                if let Some(exterior) = polygon.first() {
                    Self::point_in_polygon(point, exterior)
                } else {
                    false
                }
            }),
            GeoJsonGeometry::GeometryCollection { geometries } => {
                geometries.iter().any(|geom| geom.contains_point(point))
            }
        }
    }

    fn point_in_polygon(point: &LatLng, polygon: &[[f64; 2]]) -> bool {
        let mut inside = false;
        let mut j = polygon.len() - 1;

        for i in 0..polygon.len() {
            let xi = polygon[i][0]; // longitude
            let yi = polygon[i][1]; // latitude
            let xj = polygon[j][0];
            let yj = polygon[j][1];

            if ((yi > point.lat) != (yj > point.lat))
                && (point.lng < (xj - xi) * (point.lat - yi) / (yj - yi) + xi)
            {
                inside = !inside;
            }
            j = i;
        }

        inside
    }

    fn point_on_line(point: &LatLng, line: &[[f64; 2]]) -> bool {
        const TOLERANCE: f64 = 1e-10;

        for i in 0..line.len().saturating_sub(1) {
            let p1_lng = line[i][0];
            let p1_lat = line[i][1];
            let p2_lng = line[i + 1][0];
            let p2_lat = line[i + 1][1];

            // Check if point is close to the line segment
            let distance =
                Self::point_to_line_distance(point.lat, point.lng, p1_lat, p1_lng, p2_lat, p2_lng);

            if distance < TOLERANCE {
                return true;
            }
        }

        false
    }

    fn point_to_line_distance(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;

        if dx == 0.0 && dy == 0.0 {
            // Degenerate line segment, return distance to point
            return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
        }

        let t = ((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy);
        let t = t.clamp(0.0, 1.0);

        let closest_x = x1 + t * dx;
        let closest_y = y1 + t * dy;

        ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geojson_parsing() {
        let geojson_str = r#"
        {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "properties": {"name": "Test Point"},
                    "geometry": {
                        "type": "Point",
                        "coordinates": [-74.0060, 40.7128]
                    }
                }
            ]
        }
        "#;

        let layer = GeoJsonLayer::from_str(geojson_str).unwrap();
        let features = layer.features();
        assert_eq!(features.len(), 1);
    }

    #[test]
    fn test_point_geometry() {
        let geometry = GeoJsonGeometry::Point {
            coordinates: [-74.0060, 40.7128],
        };

        let points = geometry.to_lat_lng_points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], LatLng::new(40.7128, -74.0060));
    }

    #[test]
    fn test_bounds_calculation() {
        let geojson = GeoJson::FeatureCollection {
            features: vec![
                GeoJsonFeature {
                    id: None,
                    properties: None,
                    geometry: Some(GeoJsonGeometry::Point {
                        coordinates: [-74.0060, 40.7128],
                    }),
                },
                GeoJsonFeature {
                    id: None,
                    properties: None,
                    geometry: Some(GeoJsonGeometry::Point {
                        coordinates: [-73.9857, 40.7489],
                    }),
                },
            ],
        };

        let layer = GeoJsonLayer::new(geojson);
        let bounds = layer.bounds().unwrap();

        assert_eq!(bounds.south_west.lat, 40.7128);
        assert_eq!(bounds.north_east.lat, 40.7489);
    }
}
