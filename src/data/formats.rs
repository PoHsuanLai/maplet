use crate::prelude::HashMap;
use crate::{
    core::geo::{LatLng, LatLngBounds},
    data::geojson::{GeoJson, GeoJsonFeature, GeoJsonGeometry},
};
use serde::{Deserialize, Serialize};

/// Supported data formats
#[derive(Debug, Clone, PartialEq)]
pub enum DataFormat {
    GeoJSON,
    KML,
    GPX,
    CSV,
    Shapefile,
    WKT, // Well-Known Text
    TopoJSON,
}

/// Generic feature representation that can be converted to/from various formats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: Option<String>,
    pub geometry: Option<Geometry>,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Generic geometry types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    Point(LatLng),
    LineString(Vec<LatLng>),
    Polygon(Vec<Vec<LatLng>>),
    MultiPoint(Vec<LatLng>),
    MultiLineString(Vec<Vec<LatLng>>),
    MultiPolygon(Vec<Vec<Vec<LatLng>>>),
}

/// Collection of features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureCollection {
    pub features: Vec<Feature>,
    pub bbox: Option<LatLngBounds>,
}

/// Main data format processor
pub struct DataProcessor;

impl DataProcessor {
    /// Detects the format of input data
    pub fn detect_format(data: &str) -> Option<DataFormat> {
        let trimmed = data.trim();

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            // Check if it's valid JSON
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                return Some(DataFormat::GeoJSON);
            }
        }

        if trimmed.starts_with("<?xml") && trimmed.contains("<kml") {
            return Some(DataFormat::KML);
        }

        if trimmed.starts_with("<?xml") && trimmed.contains("<gpx") {
            return Some(DataFormat::GPX);
        }

        // Check for CSV by looking for comma-separated values
        if trimmed.lines().any(|line| line.contains(',')) {
            return Some(DataFormat::CSV);
        }

        // Check for WKT
        if trimmed.to_uppercase().starts_with("POINT")
            || trimmed.to_uppercase().starts_with("LINESTRING")
            || trimmed.to_uppercase().starts_with("POLYGON")
        {
            return Some(DataFormat::WKT);
        }

        None
    }

    /// Parses data into a feature collection
    pub fn parse(data: &str, format: Option<DataFormat>) -> Result<FeatureCollection, ParseError> {
        let detected_format = format
            .or_else(|| Self::detect_format(data))
            .ok_or(ParseError::UnknownFormat)?;

        match detected_format {
            DataFormat::GeoJSON => Self::parse_geojson(data),
            DataFormat::KML => Self::parse_kml(data),
            DataFormat::GPX => Self::parse_gpx(data),
            DataFormat::CSV => Self::parse_csv(data),
            DataFormat::WKT => Self::parse_wkt(data),
            _ => Err(ParseError::UnsupportedFormat(detected_format)),
        }
    }

    /// Exports feature collection to specified format
    pub fn export(features: &FeatureCollection, format: DataFormat) -> Result<String, ParseError> {
        match format {
            DataFormat::GeoJSON => Self::export_geojson(features),
            DataFormat::KML => Self::export_kml(features),
            DataFormat::GPX => Self::export_gpx(features),
            DataFormat::CSV => Self::export_csv(features),
            DataFormat::WKT => Self::export_wkt(features),
            _ => Err(ParseError::UnsupportedFormat(format)),
        }
    }

    // GeoJSON parsing
    fn parse_geojson(data: &str) -> Result<FeatureCollection, ParseError> {
        let geojson: GeoJson = serde_json::from_str(data)
            .map_err(|e| ParseError::InvalidFormat(format!("Invalid GeoJSON: {}", e)))?;

        let features = match geojson {
            GeoJson::Feature(feature) => vec![Self::convert_geojson_feature(feature)],
            GeoJson::FeatureCollection { features } => features
                .into_iter()
                .map(Self::convert_geojson_feature)
                .collect(),
            GeoJson::Geometry(geometry) => {
                vec![Feature {
                    id: None,
                    geometry: Some(Self::convert_geojson_geometry(geometry)),
                    properties: HashMap::default(),
                }]
            }
        };

        let bbox = Self::calculate_bbox(&features);
        Ok(FeatureCollection { features, bbox })
    }

    // Simplified KML parsing (basic implementation)
    fn parse_kml(data: &str) -> Result<FeatureCollection, ParseError> {
        // This is a very basic KML parser - a full implementation would use an XML parser
        let mut features = Vec::new();

        // Look for Placemark elements
        for line in data.lines() {
            if line.trim().contains("<coordinates>") {
                if let Some(coords_str) = Self::extract_xml_content(line, "coordinates") {
                    if let Ok(coords) = Self::parse_kml_coordinates(&coords_str) {
                        features.push(Feature {
                            id: None,
                            geometry: Some(Geometry::Point(coords[0])),
                            properties: HashMap::default(),
                        });
                    }
                }
            }
        }

        Ok(FeatureCollection {
            features,
            bbox: None,
        })
    }

    // Simplified GPX parsing (basic implementation)
    fn parse_gpx(data: &str) -> Result<FeatureCollection, ParseError> {
        let mut features = Vec::new();

        // Look for waypoints
        for line in data.lines() {
            if line.trim().contains("<wpt") {
                if let Some((lat, lon)) = Self::extract_gpx_waypoint(line) {
                    features.push(Feature {
                        id: None,
                        geometry: Some(Geometry::Point(LatLng::new(lat, lon))),
                        properties: HashMap::default(),
                    });
                }
            }
        }

        Ok(FeatureCollection {
            features,
            bbox: None,
        })
    }

    // Basic CSV parsing
    fn parse_csv(data: &str) -> Result<FeatureCollection, ParseError> {
        let lines: Vec<&str> = data.lines().collect();
        if lines.is_empty() {
            return Ok(FeatureCollection {
                features: Vec::new(),
                bbox: None,
            });
        }

        // Parse header
        let headers: Vec<&str> = lines[0].split(',').map(|h| h.trim()).collect();
        let mut lat_col = None;
        let mut lng_col = None;

        // Find lat/lng columns
        for (i, header) in headers.iter().enumerate() {
            let header_lower = header.to_lowercase();
            if header_lower.contains("lat") {
                lat_col = Some(i);
            }
            if header_lower.contains("lng") || header_lower.contains("lon") {
                lng_col = Some(i);
            }
        }

        let lat_col =
            lat_col.ok_or(ParseError::InvalidFormat("No latitude column found".into()))?;
        let lng_col = lng_col.ok_or(ParseError::InvalidFormat(
            "No longitude column found".into(),
        ))?;

        let mut features = Vec::new();

        for line in lines.iter().skip(1) {
            let values: Vec<&str> = line.split(',').map(|v| v.trim()).collect();
            if values.len() > lat_col.max(lng_col) {
                if let (Ok(lat), Ok(lng)) = (
                    values[lat_col].parse::<f64>(),
                    values[lng_col].parse::<f64>(),
                ) {
                    let mut properties = HashMap::default();
                    for (i, &value) in values.iter().enumerate() {
                        if i != lat_col && i != lng_col && i < headers.len() {
                            properties.insert(
                                headers[i].to_string(),
                                serde_json::Value::String(value.to_string()),
                            );
                        }
                    }

                    features.push(Feature {
                        id: None,
                        geometry: Some(Geometry::Point(LatLng::new(lat, lng))),
                        properties,
                    });
                }
            }
        }

        Ok(FeatureCollection {
            features,
            bbox: None,
        })
    }

    // Basic WKT parsing
    fn parse_wkt(data: &str) -> Result<FeatureCollection, ParseError> {
        let trimmed = data.trim().to_uppercase();

        if trimmed.starts_with("POINT") {
            if let Some(coords_str) = trimmed
                .strip_prefix("POINT(")
                .and_then(|s| s.strip_suffix(")"))
            {
                let coords: Vec<&str> = coords_str.split_whitespace().collect();
                if coords.len() >= 2 {
                    if let (Ok(lng), Ok(lat)) = (coords[0].parse::<f64>(), coords[1].parse::<f64>())
                    {
                        return Ok(FeatureCollection {
                            features: vec![Feature {
                                id: None,
                                geometry: Some(Geometry::Point(LatLng::new(lat, lng))),
                                properties: HashMap::default(),
                            }],
                            bbox: None,
                        });
                    }
                }
            }
        }

        Err(ParseError::InvalidFormat("Unsupported WKT geometry".into()))
    }

    // Export functions
    fn export_geojson(features: &FeatureCollection) -> Result<String, ParseError> {
        let geojson_features: Vec<GeoJsonFeature> = features
            .features
            .iter()
            .map(Self::convert_to_geojson_feature)
            .collect();

        let geojson = GeoJson::FeatureCollection {
            features: geojson_features,
        };

        serde_json::to_string_pretty(&geojson)
            .map_err(|e| ParseError::ExportError(format!("Failed to serialize GeoJSON: {}", e)))
    }

    fn export_kml(features: &FeatureCollection) -> Result<String, ParseError> {
        let mut kml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        kml.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n");
        kml.push_str("  <Document>\n");

        for (i, feature) in features.features.iter().enumerate() {
            kml.push_str("    <Placemark>\n");
            kml.push_str(&format!("      <name>Feature {}</name>\n", i));

            if let Some(geometry) = &feature.geometry {
                match geometry {
                    Geometry::Point(point) => {
                        kml.push_str("      <Point>\n");
                        kml.push_str(&format!(
                            "        <coordinates>{},{}</coordinates>\n",
                            point.lng, point.lat
                        ));
                        kml.push_str("      </Point>\n");
                    }
                    Geometry::LineString(points) => {
                        kml.push_str("      <LineString>\n");
                        let coords: Vec<String> = points
                            .iter()
                            .map(|p| format!("{},{}", p.lng, p.lat))
                            .collect();
                        kml.push_str(&format!(
                            "        <coordinates>{}</coordinates>\n",
                            coords.join(" ")
                        ));
                        kml.push_str("      </LineString>\n");
                    }
                    Geometry::Polygon(rings) => {
                        kml.push_str("      <Polygon>\n");
                        if let Some(outer_ring) = rings.first() {
                            kml.push_str("        <outerBoundaryIs>\n");
                            kml.push_str("          <LinearRing>\n");
                            let coords: Vec<String> = outer_ring
                                .iter()
                                .map(|p| format!("{},{}", p.lng, p.lat))
                                .collect();
                            kml.push_str(&format!(
                                "            <coordinates>{}</coordinates>\n",
                                coords.join(" ")
                            ));
                            kml.push_str("          </LinearRing>\n");
                            kml.push_str("        </outerBoundaryIs>\n");

                            // Add inner rings (holes)
                            for hole in rings.iter().skip(1) {
                                kml.push_str("        <innerBoundaryIs>\n");
                                kml.push_str("          <LinearRing>\n");
                                let hole_coords: Vec<String> = hole
                                    .iter()
                                    .map(|p| format!("{},{}", p.lng, p.lat))
                                    .collect();
                                kml.push_str(&format!(
                                    "            <coordinates>{}</coordinates>\n",
                                    hole_coords.join(" ")
                                ));
                                kml.push_str("          </LinearRing>\n");
                                kml.push_str("        </innerBoundaryIs>\n");
                            }
                        }
                        kml.push_str("      </Polygon>\n");
                    }
                    _ => {
                        // For unsupported geometries, add as a comment
                        kml.push_str(&format!(
                            "      <!-- Unsupported geometry type: {:?} -->\n",
                            geometry
                        ));
                    }
                }
            }

            kml.push_str("    </Placemark>\n");
        }

        kml.push_str("  </Document>\n");
        kml.push_str("</kml>\n");

        Ok(kml)
    }

    fn export_gpx(_features: &FeatureCollection) -> Result<String, ParseError> {
        // Basic GPX export would go here
        Err(ParseError::UnsupportedFormat(DataFormat::GPX))
    }

    fn export_csv(features: &FeatureCollection) -> Result<String, ParseError> {
        let mut csv = String::from("lat,lng,id\n");

        for feature in &features.features {
            if let Some(Geometry::Point(point)) = &feature.geometry {
                csv.push_str(&format!(
                    "{},{},{}\n",
                    point.lat,
                    point.lng,
                    feature.id.as_deref().unwrap_or(""),
                ));
            }
        }

        Ok(csv)
    }

    fn export_wkt(_features: &FeatureCollection) -> Result<String, ParseError> {
        // Basic WKT export would go here
        Err(ParseError::UnsupportedFormat(DataFormat::WKT))
    }

    // Helper functions
    fn convert_geojson_feature(feature: GeoJsonFeature) -> Feature {
        Feature {
            id: feature.id.map(|id| id.to_string()),
            geometry: feature.geometry.map(Self::convert_geojson_geometry),
            properties: feature.properties.unwrap_or_default(),
        }
    }

    fn convert_geojson_geometry(geometry: GeoJsonGeometry) -> Geometry {
        match geometry {
            GeoJsonGeometry::Point { coordinates } => {
                Geometry::Point(LatLng::new(coordinates[1], coordinates[0]))
            }
            GeoJsonGeometry::LineString { coordinates } => {
                let points = coordinates
                    .iter()
                    .map(|c| LatLng::new(c[1], c[0]))
                    .collect();
                Geometry::LineString(points)
            }
            GeoJsonGeometry::Polygon { coordinates } => {
                let rings = coordinates
                    .iter()
                    .map(|ring| ring.iter().map(|c| LatLng::new(c[1], c[0])).collect())
                    .collect();
                Geometry::Polygon(rings)
            }
            _ => Geometry::Point(LatLng::new(0.0, 0.0)), // Fallback
        }
    }

    fn convert_to_geojson_feature(feature: &Feature) -> GeoJsonFeature {
        GeoJsonFeature {
            id: feature
                .id
                .as_ref()
                .map(|id| serde_json::Value::String(id.clone())),
            geometry: feature
                .geometry
                .as_ref()
                .map(Self::convert_to_geojson_geometry),
            properties: Some(feature.properties.clone()),
        }
    }

    fn convert_to_geojson_geometry(geometry: &Geometry) -> GeoJsonGeometry {
        match geometry {
            Geometry::Point(point) => GeoJsonGeometry::Point {
                coordinates: [point.lng, point.lat],
            },
            Geometry::LineString(points) => GeoJsonGeometry::LineString {
                coordinates: points.iter().map(|p| [p.lng, p.lat]).collect(),
            },
            Geometry::Polygon(rings) => GeoJsonGeometry::Polygon {
                coordinates: rings
                    .iter()
                    .map(|ring| ring.iter().map(|p| [p.lng, p.lat]).collect())
                    .collect(),
            },
            _ => GeoJsonGeometry::Point {
                coordinates: [0.0, 0.0],
            }, // Fallback
        }
    }

    fn extract_xml_content(line: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);

        if let Some(start) = line.find(&start_tag) {
            if let Some(end) = line.find(&end_tag) {
                let content_start = start + start_tag.len();
                if content_start < end {
                    return Some(line[content_start..end].to_string());
                }
            }
        }
        None
    }

    fn parse_kml_coordinates(coords_str: &str) -> Result<Vec<LatLng>, ParseError> {
        let mut points = Vec::new();

        for coord_set in coords_str.split_whitespace() {
            let parts: Vec<&str> = coord_set.split(',').collect();
            if parts.len() >= 2 {
                if let (Ok(lng), Ok(lat)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    points.push(LatLng::new(lat, lng));
                }
            }
        }

        if points.is_empty() {
            Err(ParseError::InvalidFormat(
                "No valid coordinates found".into(),
            ))
        } else {
            Ok(points)
        }
    }

    fn extract_gpx_waypoint(line: &str) -> Option<(f64, f64)> {
        // Very basic GPX waypoint parsing
        if let Some(lat_start) = line.find("lat=\"") {
            if let Some(lat_end) = line[lat_start + 5..].find('"') {
                let lat_str = &line[lat_start + 5..lat_start + 5 + lat_end];
                if let Ok(lat) = lat_str.parse::<f64>() {
                    if let Some(lon_start) = line.find("lon=\"") {
                        if let Some(lon_end) = line[lon_start + 5..].find('"') {
                            let lon_str = &line[lon_start + 5..lon_start + 5 + lon_end];
                            if let Ok(lon) = lon_str.parse::<f64>() {
                                return Some((lat, lon));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Calculate bounding box for a collection of features
    fn calculate_bbox(features: &[Feature]) -> Option<LatLngBounds> {
        if features.is_empty() {
            return None;
        }

        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lng = f64::INFINITY;
        let mut max_lng = f64::NEG_INFINITY;

        for feature in features {
            if let Some(geometry) = &feature.geometry {
                let points = Self::extract_points_from_geometry(geometry);
                for point in points {
                    min_lat = min_lat.min(point.lat);
                    max_lat = max_lat.max(point.lat);
                    min_lng = min_lng.min(point.lng);
                    max_lng = max_lng.max(point.lng);
                }
            }
        }

        if min_lat.is_infinite()
            || max_lat.is_infinite()
            || min_lng.is_infinite()
            || max_lng.is_infinite()
        {
            None
        } else {
            Some(LatLngBounds::new(
                LatLng::new(min_lat, min_lng),
                LatLng::new(max_lat, max_lng),
            ))
        }
    }

    /// Extract all points from a geometry for bbox calculation
    fn extract_points_from_geometry(geometry: &Geometry) -> Vec<LatLng> {
        match geometry {
            Geometry::Point(point) => vec![*point],
            Geometry::LineString(points) => points.clone(),
            Geometry::Polygon(rings) => {
                let mut all_points = Vec::new();
                for ring in rings {
                    all_points.extend(ring.iter().copied());
                }
                all_points
            }
            Geometry::MultiPoint(points) => points.clone(),
            Geometry::MultiLineString(lines) => {
                let mut all_points = Vec::new();
                for line in lines {
                    all_points.extend(line.iter().copied());
                }
                all_points
            }
            Geometry::MultiPolygon(polygons) => {
                let mut all_points = Vec::new();
                for polygon in polygons {
                    for ring in polygon {
                        all_points.extend(ring.iter().copied());
                    }
                }
                all_points
            }
        }
    }
}

/// Errors that can occur during data parsing
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Unknown data format")]
    UnknownFormat,
    #[error("Unsupported format: {0:?}")]
    UnsupportedFormat(DataFormat),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Export error: {0}")]
    ExportError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        let geojson = r#"{"type": "Point", "coordinates": [0, 0]}"#;
        assert_eq!(
            DataProcessor::detect_format(geojson),
            Some(DataFormat::GeoJSON)
        );

        let csv = "lat,lng\n40.7128,-74.0060";
        assert_eq!(DataProcessor::detect_format(csv), Some(DataFormat::CSV));

        let wkt = "POINT(-74.0060 40.7128)";
        assert_eq!(DataProcessor::detect_format(wkt), Some(DataFormat::WKT));
    }

    #[test]
    fn test_csv_parsing() {
        let csv_data = "lat,lng,name\n40.7128,-74.0060,NYC\n34.0522,-118.2437,LA";
        let result = DataProcessor::parse(csv_data, Some(DataFormat::CSV)).unwrap();

        assert_eq!(result.features.len(), 2);
        assert_eq!(
            result.features[0].properties.get("name"),
            Some(&serde_json::Value::String("NYC".to_string()))
        );
    }

    #[test]
    fn test_wkt_parsing() {
        let wkt_data = "POINT(-74.0060 40.7128)";
        let result = DataProcessor::parse(wkt_data, Some(DataFormat::WKT)).unwrap();

        assert_eq!(result.features.len(), 1);
        if let Some(Geometry::Point(point)) = &result.features[0].geometry {
            assert_eq!(point.lat, 40.7128);
            assert_eq!(point.lng, -74.0060);
        }
    }
}
