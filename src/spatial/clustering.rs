use crate::{
    core::{
        bounds::Bounds,
        geo::Point,
    },
    spatial::index::{SpatialIndex, SpatialItem},
    Result,
};
use std::collections::HashMap;

/// Represents a cluster of markers
#[derive(Debug, Clone)]
pub struct Cluster<T> {
    /// Unique identifier for the cluster
    pub id: String,
    /// Center point of the cluster
    pub center: Point,
    /// Geographic bounds of the cluster
    pub bounds: Bounds,
    /// Items in this cluster
    pub items: Vec<SpatialItem<T>>,
    /// Zoom level at which this cluster was created
    pub zoom_level: f64,
}

impl<T> Cluster<T> {
    /// Create a new cluster
    pub fn new(id: String, items: Vec<SpatialItem<T>>, zoom_level: f64) -> Self {
        let bounds = Self::calculate_bounds(&items);
        let center = bounds.center();

        Self {
            id,
            center,
            bounds,
            items,
            zoom_level,
        }
    }

    /// Calculate bounds for a set of items
    fn calculate_bounds(items: &[SpatialItem<T>]) -> Bounds {
        if items.is_empty() {
            return Bounds::default();
        }

        let mut bounds = items[0].bounds.clone();
        for item in &items[1..] {
            bounds.extend_bounds(&item.bounds);
        }
        bounds
    }

    /// Get the number of items in the cluster
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Check if this is a single-item cluster
    pub fn is_single(&self) -> bool {
        self.items.len() == 1
    }
}

/// Configuration for clustering
#[derive(Debug, Clone)]
pub struct ClusteringConfig {
    /// Maximum distance between items to be considered for clustering (in pixels)
    pub max_cluster_radius: f64,
    /// Minimum zoom level where clustering is disabled
    pub disable_clustering_at_zoom: f64,
    /// Maximum number of items in a single cluster
    pub max_cluster_size: usize,
    /// Grid size for clustering (in pixels)
    pub grid_size: f64,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            max_cluster_radius: 80.0,
            disable_clustering_at_zoom: 15.0,
            max_cluster_size: 100,
            grid_size: 60.0,
        }
    }
}

/// Marker clustering implementation
pub struct Clustering<T> {
    config: ClusteringConfig,
    spatial_index: SpatialIndex<T>,
}

impl<T: Clone> Clustering<T> {
    /// Create a new clustering instance
    pub fn new(config: ClusteringConfig) -> Self {
        Self {
            config,
            spatial_index: SpatialIndex::new(),
        }
    }

    /// Add an item to the clustering system
    pub fn add_item(&mut self, item: SpatialItem<T>) -> Result<()> {
        self.spatial_index.insert(item)
    }

    /// Remove an item from the clustering system
    pub fn remove_item(&mut self, id: &str) -> Result<Option<SpatialItem<T>>> {
        self.spatial_index.remove(id)
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.spatial_index.clear();
    }

    /// Generate clusters for the given viewport and zoom level
    pub fn get_clusters(&self, viewport_bounds: &Bounds, zoom_level: f64) -> Vec<Cluster<T>> {
        // If zoom level is high enough, disable clustering
        if zoom_level >= self.config.disable_clustering_at_zoom {
            return self
                .spatial_index
                .query(viewport_bounds)
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    Cluster::new(format!("single_{}", i), vec![item.clone()], zoom_level)
                })
                .collect();
        }

        // Get all items in the viewport
        let items = self.spatial_index.query(viewport_bounds);

        // Use grid-based clustering for simplicity and performance
        self.grid_cluster(items, zoom_level)
    }

    /// Grid-based clustering algorithm
    fn grid_cluster(&self, items: Vec<&SpatialItem<T>>, zoom_level: f64) -> Vec<Cluster<T>> {
        let mut grid: HashMap<(i32, i32), Vec<SpatialItem<T>>> = HashMap::new();
        let grid_size = self.config.grid_size;

        // Group items by grid cell
        for item in items {
            let center = item.bounds.center();
            let grid_x = (center.x / grid_size).floor() as i32;
            let grid_y = (center.y / grid_size).floor() as i32;

            grid.entry((grid_x, grid_y))
                .or_default()
                .push(item.clone());
        }

        // Create clusters from grid cells
        let mut clusters = Vec::new();
        for ((grid_x, grid_y), cell_items) in grid {
            if cell_items.len() == 1 {
                // Single item - create individual cluster
                clusters.push(Cluster::new(
                    format!("single_{}_{}", grid_x, grid_y),
                    cell_items,
                    zoom_level,
                ));
            } else if cell_items.len() <= self.config.max_cluster_size {
                // Multiple items within limit - create cluster
                clusters.push(Cluster::new(
                    format!("cluster_{}_{}", grid_x, grid_y),
                    cell_items,
                    zoom_level,
                ));
            } else {
                // Too many items - split into multiple clusters
                let chunks: Vec<_> = cell_items.chunks(self.config.max_cluster_size).collect();
                for (i, chunk) in chunks.into_iter().enumerate() {
                    clusters.push(Cluster::new(
                        format!("cluster_{}_{}_{}", grid_x, grid_y, i),
                        chunk.to_vec(),
                        zoom_level,
                    ));
                }
            }
        }

        clusters
    }

    /// Distance-based clustering algorithm (more sophisticated but slower)
    fn distance_cluster(&self, items: Vec<&SpatialItem<T>>, zoom_level: f64) -> Vec<Cluster<T>> {
        let mut clusters = Vec::new();
        let mut processed = vec![false; items.len()];

        for (i, item) in items.iter().enumerate() {
            if processed[i] {
                continue;
            }

            let mut cluster_items = vec![(*item).clone()];
            processed[i] = true;

            let item_center = item.bounds.center();

            // Find nearby items
            for (j, other_item) in items.iter().enumerate() {
                if i == j || processed[j] {
                    continue;
                }

                let other_center = other_item.bounds.center();
                let distance = ((item_center.x - other_center.x).powi(2)
                    + (item_center.y - other_center.y).powi(2))
                .sqrt();

                if distance <= self.config.max_cluster_radius {
                    cluster_items.push((*other_item).clone());
                    processed[j] = true;

                    if cluster_items.len() >= self.config.max_cluster_size {
                        break;
                    }
                }
            }

            clusters.push(Cluster::new(
                format!("cluster_{}", clusters.len()),
                cluster_items,
                zoom_level,
            ));
        }

        clusters
    }

    /// Get all items (for debugging/inspection)
    pub fn get_all_items(&self) -> Vec<&SpatialItem<T>> {
        self.spatial_index.all_items()
    }

    /// Get the number of items in the clustering system
    pub fn len(&self) -> usize {
        self.spatial_index.len()
    }

    /// Check if the clustering system is empty
    pub fn is_empty(&self) -> bool {
        self.spatial_index.is_empty()
    }

    /// Update the clustering configuration
    pub fn set_config(&mut self, config: ClusteringConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &ClusteringConfig {
        &self.config
    }
}

impl<T: Clone> Default for Clustering<T> {
    fn default() -> Self {
        Self::new(ClusteringConfig::default())
    }
}
