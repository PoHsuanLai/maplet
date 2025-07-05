use crate::core::geo::{LatLng, Point};
use crate::prelude::HashMap;

/// Coordinate reference system definitions
#[derive(Debug)]
pub enum CoordinateSystem {
    /// World Geodetic System 1984 (WGS84) - standard lat/lng
    WGS84,
    /// Web Mercator (EPSG:3857) - used by most web maps
    WebMercator,
    /// Universal Transverse Mercator
    UTM { zone: u8, northern: bool },
    /// Custom coordinate system with transformation parameters
    Custom {
        name: String,
        // Note: function types don't implement Clone/PartialEq, so we can't derive them
    },
}

/// Data conversion utilities
pub struct Converter {
    transformations: HashMap<String, Box<dyn Fn(Point) -> Point>>,
}

impl Converter {
    pub fn new() -> Self {
        Self {
            transformations: HashMap::default(),
        }
    }

    /// Converts coordinates between different systems
    pub fn convert_coordinates(
        &self,
        point: Point,
        from: &CoordinateSystem,
        to: &CoordinateSystem,
    ) -> Result<Point, ConversionError> {
        match (from, to) {
            (CoordinateSystem::WGS84, CoordinateSystem::WebMercator) => {
                let lat_lng = LatLng::new(point.y, point.x);
                let mercator = lat_lng.to_mercator();
                Ok(mercator)
            }
            (CoordinateSystem::WebMercator, CoordinateSystem::WGS84) => {
                let lat_lng = LatLng::from_mercator(point);
                Ok(Point::new(lat_lng.lng, lat_lng.lat))
            }
            (CoordinateSystem::WGS84, CoordinateSystem::UTM { zone, northern }) => {
                Self::wgs84_to_utm(point, *zone, *northern)
            }
            (CoordinateSystem::UTM { zone, northern }, CoordinateSystem::WGS84) => {
                Self::utm_to_wgs84(point, *zone, *northern)
            }
            _ => Err(ConversionError::UnsupportedTransformation),
        }
    }

    /// Converts a batch of coordinates
    pub fn convert_coordinates_batch(
        &self,
        points: &[Point],
        from: &CoordinateSystem,
        to: &CoordinateSystem,
    ) -> Result<Vec<Point>, ConversionError> {
        points
            .iter()
            .map(|p| self.convert_coordinates(*p, from, to))
            .collect()
    }

    /// WGS84 to UTM conversion (simplified)
    fn wgs84_to_utm(point: Point, zone: u8, northern: bool) -> Result<Point, ConversionError> {
        if !(1..=60).contains(&zone) {
            return Err(ConversionError::InvalidParameters(
                "UTM zone must be 1-60".into(),
            ));
        }

        let lat = point.y.to_radians();
        let lng = point.x.to_radians();

        // UTM zone central meridian
        let central_meridian = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
        let lng_diff = lng - central_meridian;

        // WGS84 ellipsoid parameters
        const A: f64 = 6378137.0; // Semi-major axis
        const E2: f64 = 0.00669437999014; // First eccentricity squared
        const K0: f64 = 0.9996; // Scale factor

        let n = A / (1.0 - E2 * lat.sin().powi(2)).sqrt();
        let t = lat.tan();
        let c = (E2 / (1.0 - E2)) * lat.cos().powi(2);
        let a_var = lat.cos() * lng_diff;

        // UTM projection formulas (accurate implementation)
        let easting = K0
            * n
            * (a_var
                + (1.0 - t.powi(2) + c) * a_var.powi(3) / 6.0
                + (5.0 - 18.0 * t.powi(2) + t.powi(4) + 72.0 * c - 58.0 * (E2 / (1.0 - E2)))
                    * a_var.powi(5)
                    / 120.0)
            + 500000.0;

        let m = A
            * ((1.0 - E2 / 4.0 - 3.0 * E2 * E2 / 64.0 - 5.0 * E2 * E2 * E2 / 256.0) * lat
                - (3.0 * E2 / 8.0 + 3.0 * E2 * E2 / 32.0 + 45.0 * E2 * E2 * E2 / 1024.0)
                    * (2.0 * lat).sin()
                + (15.0 * E2 * E2 / 256.0 + 45.0 * E2 * E2 * E2 / 1024.0) * (4.0 * lat).sin()
                - (35.0 * E2 * E2 * E2 / 3072.0) * (6.0 * lat).sin());

        let mut northing = K0
            * (m + n
                * t
                * (a_var.powi(2) / 2.0
                    + (5.0 - t.powi(2) + 9.0 * c + 4.0 * c.powi(2)) * a_var.powi(4) / 24.0
                    + (61.0 - 58.0 * t.powi(2) + t.powi(4) + 600.0 * c
                        - 330.0 * (E2 / (1.0 - E2)))
                        * a_var.powi(6)
                        / 720.0));

        if !northern {
            northing += 10000000.0; // False northing for southern hemisphere
        }

        Ok(Point::new(easting, northing))
    }

    /// UTM to WGS84 conversion (simplified)
    fn utm_to_wgs84(point: Point, zone: u8, northern: bool) -> Result<Point, ConversionError> {
        if !(1..=60).contains(&zone) {
            return Err(ConversionError::InvalidParameters(
                "UTM zone must be 1-60".into(),
            ));
        }

        let easting = point.x;
        let mut northing = point.y;

        if !northern {
            northing -= 10000000.0;
        }

        // WGS84 ellipsoid parameters
        const A: f64 = 6378137.0;
        const E2: f64 = 0.00669437999014;
        const K0: f64 = 0.9996;

        let central_meridian = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();

        // UTM inverse projection formulas
        let m = northing / K0;
        let mu = m / (A * (1.0 - E2 / 4.0 - 3.0 * E2 * E2 / 64.0 - 5.0 * E2 * E2 * E2 / 256.0));

        // Iterative calculation for latitude
        let e1 = (1.0 - (1.0 - E2).sqrt()) / (1.0 + (1.0 - E2).sqrt());
        let lat_rad = mu
            + (3.0 * e1 / 2.0 - 27.0 * e1 * e1 * e1 / 32.0) * (2.0 * mu).sin()
            + (21.0 * e1 * e1 / 16.0 - 55.0 * e1 * e1 * e1 * e1 / 32.0) * (4.0 * mu).sin()
            + (151.0 * e1 * e1 * e1 / 96.0) * (6.0 * mu).sin()
            + (1097.0 * e1 * e1 * e1 * e1 / 512.0) * (8.0 * mu).sin();

        let n1 = A / (1.0 - E2 * lat_rad.sin().powi(2)).sqrt();
        let t1 = lat_rad.tan();
        let c1 = (E2 / (1.0 - E2)) * lat_rad.cos().powi(2);
        let r1 = A * (1.0 - E2) / (1.0 - E2 * lat_rad.sin().powi(2)).powf(1.5);
        let d = (easting - 500000.0) / (n1 * K0);

        let lat_final = lat_rad
            - (n1 * t1 / r1)
                * (d.powi(2) / 2.0
                    - (5.0 + 3.0 * t1.powi(2) + 10.0 * c1
                        - 4.0 * c1.powi(2)
                        - 9.0 * (E2 / (1.0 - E2)))
                        * d.powi(4)
                        / 24.0
                    + (61.0 + 90.0 * t1.powi(2) + 298.0 * c1 + 45.0 * t1.powi(4)
                        - 252.0 * (E2 / (1.0 - E2))
                        - 3.0 * c1.powi(2))
                        * d.powi(6)
                        / 720.0);

        let lng_rad = central_meridian
            + (d - (1.0 + 2.0 * t1.powi(2) + c1) * d.powi(3) / 6.0
                + (5.0 - 2.0 * c1 + 28.0 * t1.powi(2) - 3.0 * c1.powi(2)
                    + 8.0 * (E2 / (1.0 - E2))
                    + 24.0 * t1.powi(4))
                    * d.powi(5)
                    / 120.0)
                / lat_rad.cos();

        Ok(Point::new(lng_rad.to_degrees(), lat_final.to_degrees()))
    }

    /// Adds a custom transformation
    pub fn add_transformation<F>(&mut self, name: String, transform: F)
    where
        F: Fn(Point) -> Point + 'static,
    {
        self.transformations.insert(name, Box::new(transform));
    }

    /// Applies a custom transformation by name
    pub fn apply_transformation(&self, name: &str, point: Point) -> Option<Point> {
        self.transformations
            .get(name)
            .map(|transform| transform(point))
    }
}

impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during coordinate conversion
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Unsupported coordinate transformation")]
    UnsupportedTransformation,
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
}

/// Pixel/meter conversion utilities (specialized functions not in LatLng)
pub struct PixelMetrics;

impl PixelMetrics {
    /// Converts pixels to meters at a given zoom level and latitude
    pub fn pixels_to_meters(pixels: f64, zoom: f64, latitude: f64) -> f64 {
        // Meters per pixel at given zoom and latitude
        let resolution = 156543.03392804097 / 2_f64.powf(zoom);
        let meters_per_pixel = resolution * latitude.to_radians().cos();
        pixels * meters_per_pixel
    }

    /// Converts meters to pixels at given zoom and latitude
    pub fn meters_to_pixels(meters: f64, zoom: f64, latitude: f64) -> f64 {
        let resolution = 156543.03392804097 / 2_f64.powf(zoom);
        let meters_per_pixel = resolution * latitude.to_radians().cos();
        meters / meters_per_pixel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_conversion() {
        let converter = Converter::new();
        let wgs84_point = Point::new(-74.0060, 40.7128); // NYC in WGS84

        let mercator_result = converter
            .convert_coordinates(
                wgs84_point,
                &CoordinateSystem::WGS84,
                &CoordinateSystem::WebMercator,
            )
            .unwrap();

        // Convert back
        let wgs84_result = converter
            .convert_coordinates(
                mercator_result,
                &CoordinateSystem::WebMercator,
                &CoordinateSystem::WGS84,
            )
            .unwrap();

        assert!((wgs84_result.x - wgs84_point.x).abs() < 1e-10);
        assert!((wgs84_result.y - wgs84_point.y).abs() < 1e-10);
    }

    #[test]
    fn test_tile_conversion() {
        // First create a tile coordinate from NYC location
        let nyc = LatLng::new(40.7128, -74.0060);
        let tile = crate::core::geo::TileCoord::from_lat_lng(&nyc, 10);
        let bounds = tile.bounds(); // Use TileCoord method directly

        // Should contain NYC approximately
        assert!(bounds.contains(&nyc));
    }

    #[test]
    fn test_coordinate_utilities() {
        // Test degree/radian conversion using built-in Rust methods
        assert!((180.0_f64.to_radians() - std::f64::consts::PI).abs() < 1e-10);
        assert!((std::f64::consts::PI.to_degrees() - 180.0).abs() < 1e-10);
        // Test coordinate clamping/wrapping using LatLng methods
        assert_eq!(LatLng::wrap_lng(181.0), -179.0);
        assert_eq!(LatLng::clamp_lat(91.0), 85.0511287798);
    }

    #[test]
    fn test_utm_conversion() {
        let converter = Converter::new();
        let wgs84_point = Point::new(-74.0060, 40.7128); // NYC

        let utm_result = converter.convert_coordinates(
            wgs84_point,
            &CoordinateSystem::WGS84,
            &CoordinateSystem::UTM {
                zone: 18,
                northern: true,
            },
        );

        assert!(utm_result.is_ok());
        let utm_point = utm_result.unwrap();
        assert!(utm_point.x > 0.0 && utm_point.y > 0.0);
    }
}
