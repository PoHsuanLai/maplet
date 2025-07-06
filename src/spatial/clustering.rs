use crate::prelude::HashMap;
use crate::{
    core::{bounds::Bounds, geo::Point},
    spatial::index::{SpatialIndex, SpatialItem},
    Result,
};

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
    /// Create a new cluster with pre-allocated capacity
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

    /// Create a new cluster with estimated capacity
    pub fn with_capacity(id: String, capacity: usize, zoom_level: f64) -> Self {
        Self {
            id,
            center: Point::new(0.0, 0.0),
            bounds: Bounds::default(),
            items: Vec::with_capacity(capacity),
            zoom_level,
        }
    }

    /// OPTIMIZATION: Create cluster ID without allocation for simple cases
    pub fn create_single_id(index: usize, id_buffer: &mut String) -> String {
        use std::fmt::Write;
        id_buffer.clear();
        write!(id_buffer, "single_{}", index).unwrap();
        id_buffer.clone()
    }

    /// OPTIMIZATION: Create cluster ID for grid cells
    pub fn create_cluster_id(
        grid_x: i32,
        grid_y: i32,
        chunk_index: Option<usize>,
        id_buffer: &mut String,
    ) -> String {
        use std::fmt::Write;
        id_buffer.clear();
        if let Some(chunk) = chunk_index {
            write!(id_buffer, "cluster_{}_{}__{}", grid_x, grid_y, chunk).unwrap();
        } else {
            write!(id_buffer, "cluster_{}_{}", grid_x, grid_y).unwrap();
        }
        id_buffer.clone()
    }

    /// Calculate bounds for a set of items more efficiently
    fn calculate_bounds(items: &[SpatialItem<T>]) -> Bounds {
        if items.is_empty() {
            return Bounds::default();
        }

        // OPTIMIZATION: Use iterator with fold for single pass
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for item in items {
            min_x = min_x.min(item.bounds.min.x);
            min_y = min_y.min(item.bounds.min.y);
            max_x = max_x.max(item.bounds.max.x);
            max_y = max_y.max(item.bounds.max.y);
        }

        Bounds::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Get the number of items in the cluster
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Check if this is a single-item cluster
    pub fn is_single(&self) -> bool {
        self.items.len() == 1
    }

    /// Add an item to this cluster
    pub fn add_item(&mut self, item: SpatialItem<T>) {
        self.items.push(item);
        // Update bounds and center when items are added
        self.bounds = Self::calculate_bounds(&self.items);
        self.center = self.bounds.center();
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
    /// Cache for grid cells to avoid recomputation
    cached_grid: HashMap<(i32, i32), Vec<SpatialItem<T>>>,
    /// Last viewport bounds used for caching
    last_bounds: Option<Bounds>,
    /// Last zoom level used for caching
    last_zoom: Option<f64>,
    /// OPTIMIZATION: Reusable string buffer for ID generation
    id_buffer: String,
}

impl<T: Clone> Clustering<T> {
    /// Create a new clustering instance
    pub fn new(config: ClusteringConfig) -> Self {
        Self {
            config,
            spatial_index: SpatialIndex::new(),
            cached_grid: HashMap::default(),
            last_bounds: None,
            last_zoom: None,
            id_buffer: String::with_capacity(32), // Pre-allocate for typical IDs
        }
    }

    /// Add an item to the clustering system
    pub fn add_item(&mut self, item: SpatialItem<T>) -> Result<()> {
        self.spatial_index.insert(item)?;
        self.invalidate_cache();
        Ok(())
    }

    /// Remove an item from the clustering system
    pub fn remove_item(&mut self, id: &str) -> Result<Option<SpatialItem<T>>> {
        let result = self.spatial_index.remove(id);
        if result.is_ok() {
            self.invalidate_cache();
        }
        result
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.spatial_index.clear();
        self.invalidate_cache();
    }

    /// Invalidate cache when items change
    fn invalidate_cache(&mut self) {
        self.cached_grid.clear();
        self.last_bounds = None;
        self.last_zoom = None;
    }

    /// Generate clusters for the given viewport and zoom level with caching
    pub fn get_clusters(&mut self, viewport_bounds: &Bounds, zoom_level: f64) -> Vec<Cluster<T>> {
        // Check if we can use cached results
        if let (Some(ref last_bounds), Some(last_zoom)) = (&self.last_bounds, self.last_zoom) {
            if last_bounds == viewport_bounds && (last_zoom - zoom_level).abs() < 0.01 {
                return self.clusters_from_cache();
            }
        }

        // If zoom level is high enough, disable clustering
        if zoom_level >= self.config.disable_clustering_at_zoom {
            let items = self.spatial_index.query(viewport_bounds);
            let mut clusters = Vec::with_capacity(items.len());

            for (i, item) in items.into_iter().enumerate() {
                // OPTIMIZATION: Use reusable buffer for ID generation
                let id = Cluster::<T>::create_single_id(i, &mut self.id_buffer);
                clusters.push(Cluster::new(id, vec![item.clone()], zoom_level));
            }

            return clusters;
        }

        // Get all items in the viewport and clone them to avoid borrow issues
        let items: Vec<_> = self
            .spatial_index
            .query(viewport_bounds)
            .into_iter()
            .cloned()
            .collect();

        // Use grid-based clustering for simplicity and performance
        let clusters = self.grid_cluster_owned(items, zoom_level);

        // Update cache
        self.last_bounds = Some(viewport_bounds.clone());
        self.last_zoom = Some(zoom_level);

        clusters
    }

    /// Create clusters from cached grid
    fn clusters_from_cache(&mut self) -> Vec<Cluster<T>> {
        let mut clusters = Vec::with_capacity(self.cached_grid.len());

        for ((grid_x, grid_y), cell_items) in &self.cached_grid {
            if cell_items.len() == 1 {
                // OPTIMIZATION: Use reusable buffer for ID generation
                let id =
                    Cluster::<T>::create_cluster_id(*grid_x, *grid_y, None, &mut self.id_buffer);
                clusters.push(Cluster::new(
                    id,
                    cell_items.clone(),
                    self.last_zoom.unwrap_or(0.0),
                ));
            } else if cell_items.len() <= self.config.max_cluster_size {
                let id =
                    Cluster::<T>::create_cluster_id(*grid_x, *grid_y, None, &mut self.id_buffer);
                clusters.push(Cluster::new(
                    id,
                    cell_items.clone(),
                    self.last_zoom.unwrap_or(0.0),
                ));
            } else {
                // Split large clusters
                let chunk_size = self.config.max_cluster_size;
                for (i, chunk) in cell_items.chunks(chunk_size).enumerate() {
                    let id = Cluster::<T>::create_cluster_id(
                        *grid_x,
                        *grid_y,
                        Some(i),
                        &mut self.id_buffer,
                    );
                    clusters.push(Cluster::new(
                        id,
                        chunk.to_vec(),
                        self.last_zoom.unwrap_or(0.0),
                    ));
                }
            }
        }

        clusters
    }

    /// Optimized grid-based clustering algorithm
    fn grid_cluster_owned(
        &mut self,
        items: Vec<SpatialItem<T>>,
        zoom_level: f64,
    ) -> Vec<Cluster<T>> {
        self.cached_grid.clear();
        let grid_size = self.config.grid_size;

        // OPTIMIZATION: Estimate capacity based on items and grid size
        let estimated_grid_cells = (items.len() / 4).max(16);
        self.cached_grid.reserve(estimated_grid_cells);

        // Group items by grid cell
        for item in items {
            let center = item.bounds.center();
            let grid_x = (center.x / grid_size).floor() as i32;
            let grid_y = (center.y / grid_size).floor() as i32;

            self.cached_grid
                .entry((grid_x, grid_y))
                .or_default()
                .push(item);
        }

        // Create clusters from grid cells
        let mut clusters = Vec::with_capacity(self.cached_grid.len());
        for ((grid_x, grid_y), cell_items) in &self.cached_grid {
            if cell_items.len() == 1 {
                // Single item - create individual cluster
                // OPTIMIZATION: Use reusable buffer for ID generation
                let id =
                    Cluster::<T>::create_cluster_id(*grid_x, *grid_y, None, &mut self.id_buffer);
                clusters.push(Cluster::new(id, cell_items.clone(), zoom_level));
            } else if cell_items.len() <= self.config.max_cluster_size {
                // Multiple items within limit - create cluster
                let id =
                    Cluster::<T>::create_cluster_id(*grid_x, *grid_y, None, &mut self.id_buffer);
                clusters.push(Cluster::new(id, cell_items.clone(), zoom_level));
            } else {
                // Too many items - split into multiple clusters
                let chunk_size = self.config.max_cluster_size;
                let chunks: Vec<_> = cell_items.chunks(chunk_size).collect();
                for (i, chunk) in chunks.into_iter().enumerate() {
                    let id = Cluster::<T>::create_cluster_id(
                        *grid_x,
                        *grid_y,
                        Some(i),
                        &mut self.id_buffer,
                    );
                    clusters.push(Cluster::new(id, chunk.to_vec(), zoom_level));
                }
            }
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
        self.invalidate_cache();
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
