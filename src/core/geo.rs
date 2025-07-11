use crate::traits::{GeometryOps, PointMath};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Web Mercator projection constants
const EARTH_RADIUS: f64 = 6378137.0;
const MAX_LATITUDE: f64 = 85.0511287798;

/// Represents a geographical coordinate with latitude and longitude
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

impl LatLng {
    /// Creates a new LatLng coordinate
    pub fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng }
    }

    /// Validates that the coordinates are within valid ranges
    pub fn is_valid(&self) -> bool {
        self.lat >= -90.0 && self.lat <= 90.0 && self.lng >= -180.0 && self.lng <= 180.0
    }

    /// Calculates the distance to another LatLng using the Haversine formula
    pub fn distance_to(&self, other: &LatLng) -> f64 {
        let lat1_rad = self.lat.to_radians();
        let lat2_rad = other.lat.to_radians();
        let delta_lat = (other.lat - self.lat).to_radians();
        let delta_lng = (other.lng - self.lng).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lng / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS * c
    }

    /// Wraps longitude to [-180, 180] range
    pub fn wrap_lng(lng: f64) -> f64 {
        let wrapped = lng % 360.0;
        if wrapped > 180.0 {
            wrapped - 360.0
        } else if wrapped < -180.0 {
            wrapped + 360.0
        } else {
            wrapped
        }
    }

    /// Clamps latitude to valid range
    pub fn clamp_lat(lat: f64) -> f64 {
        lat.clamp(-MAX_LATITUDE, MAX_LATITUDE)
    }


}

impl Default for LatLng {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// Represents a point in screen or projected coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }



    pub fn floor(&self) -> Point {
        Point::new(self.x.floor(), self.y.floor())
    }
}

impl Default for Point {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// Implement unified point math operations
impl PointMath for Point {
    fn add(&self, other: &Self) -> Self {
        Point::new(self.x + other.x, self.y + other.y)
    }
    
    fn subtract(&self, other: &Self) -> Self {
        Point::new(self.x - other.x, self.y - other.y)
    }
    
    fn multiply(&self, scalar: f64) -> Self {
        Point::new(self.x * scalar, self.y * scalar)
    }
    
    fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
    
    fn scale(&self, factor: f64) -> Self {
        Point::new(self.x * factor, self.y * factor)
    }
}

/// Represents a bounding box of geographical coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatLngBounds {
    pub south_west: LatLng,
    pub north_east: LatLng,
}

impl Default for LatLngBounds {
    fn default() -> Self {
        Self::new(LatLng::default(), LatLng::default())
    }
}

impl LatLngBounds {
    pub fn new(south_west: LatLng, north_east: LatLng) -> Self {
        Self {
            south_west,
            north_east,
        }
    }

    /// Creates bounds from individual coordinates
    pub fn from_coords(south: f64, west: f64, north: f64, east: f64) -> Self {
        Self::new(LatLng::new(south, west), LatLng::new(north, east))
    }

    /// Calculates bounds from a collection of LatLng points
    pub fn from_points(points: &[LatLng]) -> Option<LatLngBounds> {
        if points.is_empty() {
            return None;
        }

        let first = points[0];
        let mut bounds = LatLngBounds::new(first, first);

        for point in points.iter().skip(1) {
            bounds.extend_with_point(point);
        }

        Some(bounds)
    }

    /// Calculates bounds from coordinate pairs [lng, lat]
    pub fn from_coordinates(coordinates: &[[f64; 2]]) -> Option<LatLngBounds> {
        if coordinates.is_empty() {
            return None;
        }

        let first = LatLng::new(coordinates[0][1], coordinates[0][0]);
        let mut bounds = LatLngBounds::new(first, first);

        for coord in coordinates.iter().skip(1) {
            let point = LatLng::new(coord[1], coord[0]);
            bounds.extend_with_point(&point);
        }

        Some(bounds)
    }



    /// Gets the span of the bounds
    pub fn span(&self) -> LatLng {
        LatLng::new(
            self.north_east.lat - self.south_west.lat,
            self.north_east.lng - self.south_west.lng,
        )
    }

    /// Returns the union of this bounds with another bounds
    pub fn union(&self, other: &LatLngBounds) -> LatLngBounds {
        let south = self.south_west.lat.min(other.south_west.lat);
        let west = self.south_west.lng.min(other.south_west.lng);
        let north = self.north_east.lat.max(other.north_east.lat);
        let east = self.north_east.lng.max(other.north_east.lng);

        LatLngBounds::new(LatLng::new(south, west), LatLng::new(north, east))
    }
}

/// Implement unified geometry operations for LatLngBounds
impl GeometryOps<LatLng> for LatLngBounds {
    fn contains_point(&self, point: &LatLng) -> bool {
        point.lat >= self.south_west.lat
            && point.lat <= self.north_east.lat
            && point.lng >= self.south_west.lng
            && point.lng <= self.north_east.lng
    }
    
    fn intersects_bounds(&self, other: &Self) -> bool {
        !(other.north_east.lat < self.south_west.lat
            || other.south_west.lat > self.north_east.lat
            || other.north_east.lng < self.south_west.lng
            || other.south_west.lng > self.north_east.lng)
    }
    
    fn extend_with_point(&mut self, point: &LatLng) {
        self.south_west.lat = self.south_west.lat.min(point.lat);
        self.south_west.lng = self.south_west.lng.min(point.lng);
        self.north_east.lat = self.north_east.lat.max(point.lat);
        self.north_east.lng = self.north_east.lng.max(point.lng);
    }
    
    fn center(&self) -> LatLng {
        LatLng::new(
            (self.south_west.lat + self.north_east.lat) / 2.0,
            (self.south_west.lng + self.north_east.lng) / 2.0,
        )
    }
    
    fn is_valid(&self) -> bool {
        self.south_west.lat <= self.north_east.lat && self.south_west.lng <= self.north_east.lng
    }
    
    fn area(&self) -> f64 {
        if !self.is_valid() {
            0.0
        } else {
            (self.north_east.lat - self.south_west.lat) * (self.north_east.lng - self.south_west.lng)
        }
    }
}

/// Represents a tile coordinate in the slippy map tile system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl TileCoord {
    pub fn new(x: u32, y: u32, z: u8) -> Self {
        Self { x, y, z }
    }

    /// Creates a tile coordinate from a LatLng and zoom level
    pub fn from_lat_lng(lat_lng: &LatLng, zoom: u8) -> Self {
        let lat_rad = LatLng::clamp_lat(lat_lng.lat).to_radians();
        let n = 2_f64.powi(zoom as i32);

        let x = ((lat_lng.lng + 180.0) / 360.0 * n).floor() as u32;
        let y = ((1.0 - lat_rad.tan().asinh() / PI) / 2.0 * n).floor() as u32;

        Self::new(x, y, zoom)
    }

    /// Converts tile coordinate to LatLng (northwest corner)
    pub fn to_lat_lng(&self) -> LatLng {
        let n = 2_f64.powi(self.z as i32);
        let lng = self.x as f64 / n * 360.0 - 180.0;
        let lat_rad = (PI * (1.0 - 2.0 * self.y as f64 / n)).sinh().atan();
        let lat = lat_rad.to_degrees();

        LatLng::new(lat, lng)
    }

    /// Gets the bounds of the tile
    pub fn bounds(&self) -> LatLngBounds {
        let nw = self.to_lat_lng();
        let se_tile = TileCoord::new(self.x + 1, self.y + 1, self.z);
        let se = se_tile.to_lat_lng();

        LatLngBounds::new(LatLng::new(se.lat, nw.lng), LatLng::new(nw.lat, se.lng))
    }

    /// Gets the parent tile at a lower zoom level
    pub fn parent(&self) -> Option<TileCoord> {
        if self.z == 0 {
            None
        } else {
            Some(TileCoord::new(self.x / 2, self.y / 2, self.z - 1))
        }
    }

    /// Gets the child tiles at a higher zoom level
    pub fn children(&self) -> Vec<TileCoord> {
        if self.z >= 18 {
            Vec::new()
        } else {
            vec![
                TileCoord::new(self.x * 2, self.y * 2, self.z + 1),
                TileCoord::new(self.x * 2 + 1, self.y * 2, self.z + 1),
                TileCoord::new(self.x * 2, self.y * 2 + 1, self.z + 1),
                TileCoord::new(self.x * 2 + 1, self.y * 2 + 1, self.z + 1),
            ]
        }
    }

    /// Checks if the tile is valid for the given zoom level
    pub fn is_valid(&self) -> bool {
        let max_coord = 2_u32.pow(self.z as u32);
        self.x < max_coord && self.y < max_coord
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lat_lng_creation() {
        let coord = LatLng::new(40.7128, -74.0060);
        assert_eq!(coord.lat, 40.7128);
        assert_eq!(coord.lng, -74.0060);
        assert!(coord.is_valid());
    }

    #[test]
    fn test_lat_lng_distance() {
        let nyc = LatLng::new(40.7128, -74.0060);
        let la = LatLng::new(34.0522, -118.2437);
        let distance = nyc.distance_to(&la);

        // Distance should be approximately 3944 km
        assert!((distance - 3944000.0).abs() < 10000.0);
    }

    #[test]
    fn test_tile_coord_conversion() {
        let lat_lng = LatLng::new(40.7128, -74.0060);
        let tile = TileCoord::from_lat_lng(&lat_lng, 10);
        let back_to_lat_lng = tile.to_lat_lng();

        // Should be reasonably close (within tile boundaries)
        assert!((back_to_lat_lng.lat - lat_lng.lat).abs() < 1.0);
        assert!((back_to_lat_lng.lng - lat_lng.lng).abs() < 1.0);
    }

    #[test]
    fn test_bounds_contains() {
        let bounds = LatLngBounds::from_coords(40.0, -75.0, 41.0, -73.0);
        let point_inside = LatLng::new(40.5, -74.0);
        let point_outside = LatLng::new(42.0, -74.0);

        assert!(bounds.contains_point(&point_inside));
        assert!(!bounds.contains_point(&point_outside));
    }
}
