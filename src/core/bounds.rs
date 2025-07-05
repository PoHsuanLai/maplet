use crate::core::geo::Point;
use crate::traits::{GeometryOps, MatrixTransform};
use serde::{Deserialize, Serialize};

/// Represents a bounding box in screen/pixel coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    /// Creates new bounds from two points
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Creates bounds from individual coordinates
    pub fn from_coords(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Creates bounds from a center point and size
    pub fn from_center_and_size(center: Point, width: f64, height: f64) -> Self {
        let half_width = width / 2.0;
        let half_height = height / 2.0;
        Self::new(
            Point::new(center.x - half_width, center.y - half_height),
            Point::new(center.x + half_width, center.y + half_height),
        )
    }

    /// Gets the width of the bounds
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Gets the height of the bounds
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Gets the size as a Point
    pub fn size(&self) -> Point {
        Point::new(self.width(), self.height())
    }

    /// Gets the center point of the bounds
    pub fn center(&self) -> Point {
        Point::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    /// Checks if the bounds contain a point
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Checks if the bounds intersect with another bounds
    pub fn intersects(&self, other: &Bounds) -> bool {
        !(other.max.x < self.min.x
            || other.min.x > self.max.x
            || other.max.y < self.min.y
            || other.min.y > self.max.y)
    }

    /// Gets the intersection of two bounds
    pub fn intersection(&self, other: &Bounds) -> Option<Bounds> {
        if !self.intersects(other) {
            return None;
        }

        Some(Bounds::new(
            Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        ))
    }

    /// Extends the bounds to include a point
    pub fn extend(&mut self, point: &Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }

    /// Extends the bounds to include another bounds
    pub fn extend_bounds(&mut self, other: &Bounds) {
        self.extend(&other.min);
        self.extend(&other.max);
    }

    /// Expands the bounds by a given amount
    pub fn expand(&mut self, amount: f64) {
        self.min.x -= amount;
        self.min.y -= amount;
        self.max.x += amount;
        self.max.y += amount;
    }

    /// Returns a new bounds expanded by the given amount
    pub fn expanded(&self, amount: f64) -> Bounds {
        let mut expanded = self.clone();
        expanded.expand(amount);
        expanded
    }

    /// Checks if the bounds are valid (min <= max)
    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y
    }

    /// Gets the area of the bounds
    pub fn area(&self) -> f64 {
        if !self.is_valid() {
            0.0
        } else {
            self.width() * self.height()
        }
    }

    /// Clamps a point to be within the bounds
    pub fn clamp(&self, point: &Point) -> Point {
        Point::new(
            point.x.clamp(self.min.x, self.max.x),
            point.y.clamp(self.min.y, self.max.y),
        )
    }

    /// Gets the four corner points of the bounds
    pub fn corners(&self) -> [Point; 4] {
        [
            self.min,                           // bottom-left
            Point::new(self.max.x, self.min.y), // bottom-right
            self.max,                           // top-right
            Point::new(self.min.x, self.max.y), // top-left
        ]
    }

    /// Creates empty bounds (invalid bounds that can be extended)
    pub fn empty() -> Self {
        Self::new(
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        )
    }

    /// Returns a new bounds that extends this bounds with another bounds  
    pub fn extend_with(&self, other: &Bounds) -> Bounds {
        if !self.is_valid() {
            return other.clone();
        }
        if !other.is_valid() {
            return self.clone();
        }

        Bounds::new(
            Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        )
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Self::new(Point::new(0.0, 0.0), Point::new(0.0, 0.0))
    }
}

/// Implement unified geometry operations for Bounds
impl GeometryOps<Point> for Bounds {
    fn contains_point(&self, point: &Point) -> bool {
        self.contains(point)
    }
    
    fn intersects_bounds(&self, other: &Self) -> bool {
        self.intersects(other)
    }
    
    fn extend_with_point(&mut self, point: &Point) {
        self.extend(point)
    }
    
    fn center(&self) -> Point {
        self.center()
    }
    
    fn is_valid(&self) -> bool {
        self.is_valid()
    }
    
    fn area(&self) -> f64 {
        self.area()
    }
}

/// Implement unified matrix transformation for Points
impl MatrixTransform for Point {
    fn apply_transform(&self, matrix: &[f64; 6]) -> Self {
        Point::new(
            matrix[0] * self.x + matrix[2] * self.y + matrix[4], // a*x + c*y + e
            matrix[1] * self.x + matrix[3] * self.y + matrix[5], // b*x + d*y + f
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_creation() {
        let bounds = Bounds::from_coords(10.0, 20.0, 30.0, 40.0);
        assert_eq!(bounds.width(), 20.0);
        assert_eq!(bounds.height(), 20.0);
        assert_eq!(bounds.center(), Point::new(20.0, 30.0));
    }

    #[test]
    fn test_bounds_contains() {
        let bounds = Bounds::from_coords(10.0, 20.0, 30.0, 40.0);
        assert!(bounds.contains(&Point::new(15.0, 25.0)));
        assert!(!bounds.contains(&Point::new(5.0, 25.0)));
    }

    #[test]
    fn test_bounds_intersection() {
        let bounds1 = Bounds::from_coords(0.0, 0.0, 10.0, 10.0);
        let bounds2 = Bounds::from_coords(5.0, 5.0, 15.0, 15.0);

        let intersection = bounds1.intersection(&bounds2).unwrap();
        assert_eq!(intersection.min, Point::new(5.0, 5.0));
        assert_eq!(intersection.max, Point::new(10.0, 10.0));
    }

    #[test]
    fn test_bounds_no_intersection() {
        let bounds1 = Bounds::from_coords(0.0, 0.0, 5.0, 5.0);
        let bounds2 = Bounds::from_coords(10.0, 10.0, 15.0, 15.0);

        assert!(bounds1.intersection(&bounds2).is_none());
    }
}
