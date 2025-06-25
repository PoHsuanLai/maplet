use crate::{
    core::{
        bounds::Bounds,
        geo::{LatLng, Point},
    },
    Result,
};

use rstar::{RTree, RTreeObject, AABB, PointDistance};

/// A spatial item that can be indexed via an R-tree
#[derive(Debug, Clone)]
pub struct SpatialItem<T> {
    pub id: String,
    pub bounds: Bounds,
    pub data: T,
}

impl<T> SpatialItem<T> {
    pub fn new(id: String, bounds: Bounds, data: T) -> Self {
        Self { id, bounds, data }
    }

    pub fn from_point(id: String, point: Point, data: T) -> Self {
        let bounds = Bounds::new(point, point);
        Self::new(id, bounds, data)
    }

    pub fn from_lat_lng(id: String, lat_lng: LatLng, data: T) -> Self {
        let point = Point::new(lat_lng.lng, lat_lng.lat);
        Self::from_point(id, point, data)
    }
}

impl<T> PartialEq for SpatialItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for SpatialItem<T> {}

// --- rstar integration -------------------------------------------------------------------------

impl<T> RTreeObject for SpatialItem<T> {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.bounds.min.x, self.bounds.min.y],
            [self.bounds.max.x, self.bounds.max.y],
        )
    }
}

impl<T> PointDistance for SpatialItem<T> {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let center = self.bounds.center();
        let dx = center.x - point[0];
        let dy = center.y - point[1];
        dx * dx + dy * dy
    }

    fn contains_point(&self, point: &[f64; 2]) -> bool {
        let p = crate::core::geo::Point::new(point[0], point[1]);
        self.bounds.contains(&p)
    }
}

/// R-tree based spatial index (replaces the previous linear index)
pub struct SpatialIndex<T> {
    rtree: RTree<SpatialItem<T>>,
    bounds: Option<Bounds>,
}

impl<T: Clone> SpatialIndex<T> {
    pub fn new() -> Self {
        Self {
            rtree: RTree::new(),
            bounds: None,
        }
    }

    pub fn insert(&mut self, item: SpatialItem<T>) -> Result<()> {
        if let Some(ref mut b) = self.bounds {
            b.extend_bounds(&item.bounds);
        } else {
            self.bounds = Some(item.bounds.clone());
        }

        self.rtree.insert(item);
        Ok(())
    }

    pub fn query(&self, bounds: &Bounds) -> Vec<&SpatialItem<T>> {
        let envelope = AABB::from_corners(
            [bounds.min.x, bounds.min.y],
            [bounds.max.x, bounds.max.y],
        );
        self.rtree.locate_in_envelope_intersecting(&envelope).collect()
    }

    pub fn query_radius(&self, center: &Point, radius: f64) -> Vec<&SpatialItem<T>> {
        let center_arr = [center.x, center.y];
        self.rtree.locate_within_distance(center_arr, radius).collect()
    }

    pub fn remove(&mut self, id: &str) -> Result<Option<SpatialItem<T>>> {
        // First find the element immutably, clone it, then remove mutably.
        let found = self.rtree.iter().find(|obj| obj.id == id).cloned();

        if let Some(item) = found {
            let removed = self.rtree.remove(&item);

            // Update global bounds
            if self.rtree.size() == 0 {
                self.bounds = None;
            } else {
                let env = self.rtree.root().envelope();
                self.bounds = Some(Bounds::from_coords(
                    env.lower()[0],
                    env.lower()[1],
                    env.upper()[0],
                    env.upper()[1],
                ));
            }

            Ok(removed)
        } else {
            Ok(None)
        }
    }

    pub fn all_items(&self) -> Vec<&SpatialItem<T>> {
        self.rtree.iter().collect()
    }

    pub fn bounds(&self) -> Option<Bounds> {
        self.bounds.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.rtree.size() == 0
    }

    pub fn len(&self) -> usize {
        self.rtree.size()
    }

    pub fn clear(&mut self) {
        self.rtree = RTree::new();
        self.bounds = None;
    }

    pub fn get(&self, id: &str) -> Option<&SpatialItem<T>> {
        self.rtree.iter().find(|item| item.id == id)
    }

    pub fn update(&mut self, id: &str, new_item: SpatialItem<T>) -> Result<bool> {
        if self.remove(id)?.is_some() {
            self.insert(new_item)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<T: Clone> Default for SpatialIndex<T> {
    fn default() -> Self {
        Self::new()
    }
}
