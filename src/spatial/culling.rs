use crate::core::{bounds::Bounds, geo::Point};

/// Simple axis-aligned frustum culling helper.
///
/// The current implementation is 2-D (screen-space).
/// It can be extended later to handle 3-D frusta or quadtree acceleration.
/// Uses unified geometry operations to eliminate duplication
pub struct Culling;

impl Culling {
    /// Returns `true` if the supplied rectangle is at least partially inside the viewport.
    /// Delegates to unified Bounds::intersects method
    pub fn aabb_intersects(viewport: &Bounds, target: &Bounds) -> bool {
        viewport.intersects(target)
    }

    /// Returns `true` if a point lies inside the viewport rectangle.
    /// Delegates to unified Bounds::contains method
    pub fn point_visible(viewport: &Bounds, p: &Point) -> bool {
        viewport.contains(p)
    }

    /// Cull a slice of bounding boxes, collecting the indices of the visible ones.
    /// Uses unified intersection method
    pub fn visible_indices<'a>(
        viewport: &Bounds,
        objects: impl Iterator<Item = &'a Bounds>,
    ) -> Vec<usize> {
        objects
            .enumerate()
            .filter_map(|(idx, b)| {
                if viewport.intersects(b) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }
}
