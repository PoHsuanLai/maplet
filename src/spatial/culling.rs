use crate::core::{bounds::Bounds, geo::Point};

/// Simple axis-aligned frustum culling helper.
///
/// The current implementation is 2-D (screen-space).
/// It can be extended later to handle 3-D frusta or quadtree acceleration.
pub struct Culling;

impl Culling {
    /// Returns `true` if the supplied rectangle is at least partially inside the viewport.
    ///
    /// `viewport` – the current screen rectangle in pixel coordinates.
    /// `target`   – the object's bounding rectangle in the **same** coordinate system.
    pub fn aabb_intersects(viewport: &Bounds, target: &Bounds) -> bool {
        viewport.intersects(target)
    }

    /// Returns `true` if a point lies inside the viewport rectangle.
    pub fn point_visible(viewport: &Bounds, p: &Point) -> bool {
        viewport.contains(p)
    }

    /// Cull a slice of bounding boxes, collecting the indices of the visible ones.
    pub fn visible_indices<'a>(
        viewport: &Bounds,
        objects: impl Iterator<Item = &'a Bounds>,
    ) -> Vec<usize> {
        objects
            .enumerate()
            .filter_map(|(idx, b)| {
                if Self::aabb_intersects(viewport, b) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }
}
