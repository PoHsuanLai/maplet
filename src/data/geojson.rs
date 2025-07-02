use crate::core::bounds::Bounds;
use crate::core::geo::{LatLng, LatLngBounds};
use crate::prelude::HashMap;
use crate::spatial::index::{SpatialIndex, SpatialItem};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;

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

impl GeoJsonFeature {
    /// Get the bounds of this feature
    pub fn bounds(&self) -> Option<LatLngBounds> {
        self.geometry
            .as_ref()
            .and_then(GeoJsonLayer::geometry_bounds)
    }
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

/// Type alias for style functions
pub type StyleFunction = Box<dyn Fn(&GeoJsonFeature) -> FeatureStyle + Send + Sync>;

/// Type alias for filter functions  
pub type FilterFunction = Box<dyn Fn(&GeoJsonFeature) -> bool + Send + Sync>;

/// Type alias for filter functions in streaming config
pub type FilterFunctionArc = Arc<dyn Fn(&GeoJsonFeature) -> bool + Send + Sync>;

/// GeoJSON layer for displaying geographic data
pub struct GeoJsonLayer {
    data: GeoJson,
    style: FeatureStyle,
    style_fn: Option<StyleFunction>,
    filter_fn: Option<FilterFunction>,
}

impl std::str::FromStr for GeoJsonLayer {
    type Err =
        std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>;

    /// Creates a new GeoJSON layer from raw JSON string
    fn from_str(geojson_str: &str) -> crate::Result<Self> {
        let data: GeoJson = serde_json::from_str(geojson_str)
            .map_err(|e| crate::Error::ParseError(format!("Invalid GeoJSON: {}", e)))?;

        Ok(Self {
            data,
            style: FeatureStyle::default(),
            style_fn: None,
            filter_fn: None,
        })
    }
}

impl GeoJsonLayer {
    /// Creates a new GeoJSON layer from parsed GeoJSON
    pub fn new(data: GeoJson) -> Self {
        Self {
            data,
            style: FeatureStyle::default(),
            style_fn: None,
            filter_fn: None,
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
        F: Fn(&GeoJsonFeature) -> FeatureStyle + Send + Sync + 'static,
    {
        self.style_fn = Some(Box::new(style_fn));
        self
    }

    /// Sets a filter function to show/hide features
    pub fn set_filter<F>(mut self, filter_fn: F) -> Self
    where
        F: Fn(&GeoJsonFeature) -> bool + Send + Sync + 'static,
    {
        self.filter_fn = Some(Box::new(filter_fn));
        self
    }

    /// Gets all features in the layer
    pub fn features(&self) -> Vec<&GeoJsonFeature> {
        let mut features = Vec::new();
        self.collect_features(&self.data, &mut features);

        if let Some(filter) = &self.filter_fn {
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
        if let Some(style_fn) = &self.style_fn {
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

    pub fn geometry_bounds(geometry: &GeoJsonGeometry) -> Option<LatLngBounds> {
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
        LatLngBounds::from_coordinates(coordinates)
    }

    /// Create a GeoJsonLayer from a parsed GeoJson object
    pub fn from_geojson(data: GeoJson) -> Self {
        Self::new(data)
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
    use std::str::FromStr;

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

/// Configuration for streaming GeoJSON processing
pub struct StreamingConfig {
    /// Maximum number of features to process in one chunk
    pub chunk_size: usize,
    /// Maximum memory usage before flushing (in bytes)
    pub memory_limit: usize,
    /// Enable progressive loading
    pub progressive: bool,
    /// Spatial index configuration
    pub spatial_index: bool,
    /// Feature filtering predicate
    pub filter: Option<FilterFunctionArc>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            memory_limit: 50 * 1024 * 1024, // 50MB
            progressive: true,
            spatial_index: true,
            filter: None,
        }
    }
}

/// A chunk of processed GeoJSON features
#[derive(Clone)]
pub struct FeatureChunk {
    /// Features in this chunk
    pub features: Vec<GeoJsonFeature>,
    /// Spatial index for this chunk (if enabled)
    pub spatial_index: Option<SpatialIndex<GeoJsonFeature>>,
    /// Bounds of all features in this chunk
    pub bounds: Option<LatLngBounds>,
    /// Chunk metadata
    pub metadata: ChunkMetadata,
}

/// Metadata for a feature chunk
#[derive(Debug, Clone)]
pub struct ChunkMetadata {
    /// Chunk index
    pub index: usize,
    /// Total number of features in chunk
    pub feature_count: usize,
    /// Estimated memory usage (bytes)
    pub memory_usage: usize,
    /// Processing time
    pub processing_time: std::time::Duration,
}

/// Streaming GeoJSON processor for large datasets
pub struct StreamingGeoJsonProcessor {
    /// Processing configuration
    config: StreamingConfig,
    /// Current chunk being processed
    current_chunk: Vec<GeoJsonFeature>,
    /// Completed chunks
    completed_chunks: Vec<FeatureChunk>,
    /// Current memory usage estimate
    current_memory: usize,
    /// Total features processed
    total_features: usize,
    /// Processing statistics
    stats: ProcessingStats,
}

/// Processing statistics
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    /// Total processing time
    pub total_time: std::time::Duration,
    /// Number of chunks processed
    pub chunks_processed: usize,
    /// Total features processed
    pub features_processed: usize,
    /// Total bytes processed
    pub bytes_processed: usize,
    /// Average processing speed (features per second)
    pub processing_speed: f64,
}

impl StreamingGeoJsonProcessor {
    /// Create a new streaming processor
    pub fn new(config: StreamingConfig) -> Self {
        Self {
            config,
            current_chunk: Vec::new(),
            completed_chunks: Vec::new(),
            current_memory: 0,
            total_features: 0,
            stats: ProcessingStats::default(),
        }
    }

    /// Process GeoJSON from a reader in chunks
    pub fn process_stream<R: Read>(&mut self, reader: R) -> Result<()> {
        let start_time = std::time::Instant::now();
        let mut buf_reader = BufReader::new(reader);
        let mut buffer = String::new();
        let mut in_feature_collection = false;
        let mut brace_count = 0;
        let mut current_feature = String::new();

        while buf_reader.read_line(&mut buffer)? > 0 {
            for ch in buffer.chars() {
                current_feature.push(ch);

                match ch {
                    '{' => {
                        brace_count += 1;
                        if !in_feature_collection && current_feature.contains("\"features\"") {
                            in_feature_collection = true;
                        }
                    }
                    '}' => {
                        brace_count -= 1;
                        if in_feature_collection && brace_count == 1 {
                            // End of a feature
                            if let Ok(feature) = self.parse_feature(&current_feature) {
                                self.add_feature(feature)?;
                            }
                            current_feature.clear();
                        }
                    }
                    ',' => {
                        if in_feature_collection && brace_count == 1 {
                            // End of a feature (with comma)
                            if let Ok(feature) =
                                self.parse_feature(current_feature.trim_end_matches(','))
                            {
                                self.add_feature(feature)?;
                            }
                            current_feature.clear();
                        }
                    }
                    _ => {}
                }
            }
            buffer.clear();
        }

        // Process any remaining features
        if !self.current_chunk.is_empty() {
            self.flush_chunk()?;
        }

        // Update statistics
        self.stats.total_time = start_time.elapsed();
        if self.stats.total_time.as_secs_f64() > 0.0 {
            self.stats.processing_speed =
                self.stats.features_processed as f64 / self.stats.total_time.as_secs_f64();
        }

        Ok(())
    }

    /// Add a feature to the current chunk
    fn add_feature(&mut self, feature: GeoJsonFeature) -> Result<()> {
        // Apply filter if configured
        if let Some(filter) = &self.config.filter {
            if !filter(&feature) {
                return Ok(());
            }
        }

        // Estimate memory usage
        let feature_memory = self.estimate_feature_memory(&feature);

        // Check if we need to flush the current chunk
        if self.current_chunk.len() >= self.config.chunk_size
            || self.current_memory + feature_memory > self.config.memory_limit
        {
            self.flush_chunk()?;
        }

        self.current_chunk.push(feature);
        self.current_memory += feature_memory;
        self.total_features += 1;

        Ok(())
    }

    /// Flush the current chunk
    fn flush_chunk(&mut self) -> Result<()> {
        if self.current_chunk.is_empty() {
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        // Calculate bounds
        let bounds = self.calculate_chunk_bounds(&self.current_chunk);

        // Build spatial index if enabled
        let spatial_index = if self.config.spatial_index {
            let mut index = SpatialIndex::new();
            for (i, feature) in self.current_chunk.iter().enumerate() {
                if let Some(lat_lng_bounds) = feature.bounds() {
                    // Convert LatLngBounds to Bounds for spatial index
                    let bounds = Bounds::from_coords(
                        lat_lng_bounds.south_west.lng,
                        lat_lng_bounds.south_west.lat,
                        lat_lng_bounds.north_east.lng,
                        lat_lng_bounds.north_east.lat,
                    );
                    let item = SpatialItem {
                        id: format!("feature_{}", i),
                        bounds,
                        data: feature.clone(),
                    };
                    let _ = index.insert(item);
                }
            }
            Some(index)
        } else {
            None
        };

        let processing_time = start_time.elapsed();

        // Create chunk
        let chunk = FeatureChunk {
            features: std::mem::take(&mut self.current_chunk),
            spatial_index,
            bounds,
            metadata: ChunkMetadata {
                index: self.completed_chunks.len(),
                feature_count: self.current_chunk.len(),
                memory_usage: self.current_memory,
                processing_time,
            },
        };

        self.completed_chunks.push(chunk);
        self.current_memory = 0;
        self.stats.chunks_processed += 1;
        self.stats.features_processed += self.current_chunk.len();

        Ok(())
    }

    /// Parse a feature from JSON string
    fn parse_feature(&self, json_str: &str) -> Result<GeoJsonFeature> {
        let cleaned = json_str.trim().trim_start_matches(',').trim();
        serde_json::from_str(cleaned).map_err(|e| format!("Failed to parse feature: {}", e).into())
    }

    /// Estimate memory usage of a feature
    fn estimate_feature_memory(&self, feature: &GeoJsonFeature) -> usize {
        // Rough estimation based on JSON serialization size
        match serde_json::to_string(feature) {
            Ok(json) => json.len() * 2, // Account for overhead
            Err(_) => 1024,             // Default estimate
        }
    }

    /// Calculate bounds for a chunk of features
    fn calculate_chunk_bounds(&self, features: &[GeoJsonFeature]) -> Option<LatLngBounds> {
        let mut bounds: Option<LatLngBounds> = None;

        for feature in features {
            if let Some(feature_bounds) = feature.bounds() {
                if let Some(ref mut b) = bounds {
                    b.extend(&feature_bounds.south_west);
                    b.extend(&feature_bounds.north_east);
                } else {
                    bounds = Some(feature_bounds);
                }
            }
        }

        bounds
    }

    /// Get all completed chunks
    pub fn chunks(&self) -> &[FeatureChunk] {
        &self.completed_chunks
    }

    /// Get processing statistics
    pub fn stats(&self) -> &ProcessingStats {
        &self.stats
    }

    /// Get chunks within a specific bounds (spatial query)
    pub fn chunks_in_bounds(&self, bounds: &LatLngBounds) -> Vec<&FeatureChunk> {
        self.completed_chunks
            .iter()
            .filter(|chunk| {
                chunk
                    .bounds
                    .as_ref()
                    .map(|cb| cb.intersects(bounds))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get features within bounds from all chunks
    pub fn features_in_bounds(&self, bounds: &LatLngBounds) -> Vec<&GeoJsonFeature> {
        let mut features = Vec::new();

        for chunk in self.chunks_in_bounds(bounds) {
            if let Some(spatial_index) = &chunk.spatial_index {
                // Use spatial index for efficient querying
                // Convert LatLngBounds to Bounds for spatial query
                let query_bounds = Bounds::from_coords(
                    bounds.south_west.lng,
                    bounds.south_west.lat,
                    bounds.north_east.lng,
                    bounds.north_east.lat,
                );
                let items = spatial_index.query(&query_bounds);
                features.extend(items.iter().map(|item| &item.data));
            } else {
                // Fallback to linear search
                for feature in &chunk.features {
                    if let Some(feature_bounds) = feature.bounds() {
                        if feature_bounds.intersects(bounds) {
                            features.push(feature);
                        }
                    }
                }
            }
        }

        features
    }

    /// Clear all processed data to free memory
    pub fn clear(&mut self) {
        self.current_chunk.clear();
        self.completed_chunks.clear();
        self.current_memory = 0;
        self.total_features = 0;
        self.stats = ProcessingStats::default();
    }
}

/// Progressive GeoJSON loader for UI integration
pub struct ProgressiveGeoJsonLoader {
    /// Streaming processor
    processor: StreamingGeoJsonProcessor,
    /// Loading state
    state: LoadingState,
    /// Progress callback
    progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadingState {
    Idle,
    Loading { progress: f64 },
    Completed,
    Error(String),
}

impl ProgressiveGeoJsonLoader {
    /// Create a new progressive loader
    pub fn new(config: StreamingConfig) -> Self {
        Self {
            processor: StreamingGeoJsonProcessor::new(config),
            state: LoadingState::Idle,
            progress_callback: None,
        }
    }

    /// Set progress callback
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Load GeoJSON progressively from bytes
    pub async fn load_progressive(&mut self, data: Vec<u8>) -> Result<()> {
        self.state = LoadingState::Loading { progress: 0.0 };

        // Simulate progressive loading by processing in chunks
        let _chunk_size = 8192; // 8KB chunks
        let _total_size = data.len();

        let mut cursor = std::io::Cursor::new(data);
        let _processed = 0;

        // Process the data
        match self.processor.process_stream(&mut cursor) {
            Ok(_) => {
                self.state = LoadingState::Completed;
                if let Some(callback) = &self.progress_callback {
                    callback(1.0);
                }
            }
            Err(e) => {
                self.state = LoadingState::Error(e.to_string());
            }
        }

        Ok(())
    }

    /// Get current loading state
    pub fn state(&self) -> &LoadingState {
        &self.state
    }

    /// Get the processor
    pub fn processor(&self) -> &StreamingGeoJsonProcessor {
        &self.processor
    }

    /// Get mutable processor
    pub fn processor_mut(&mut self) -> &mut StreamingGeoJsonProcessor {
        &mut self.processor
    }
}
