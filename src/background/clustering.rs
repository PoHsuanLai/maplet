//! Background clustering tasks
//!
//! This module provides background tasks for spatial clustering operations
//! to keep CPU-intensive clustering computations off the main thread.

use std::sync::Arc;
use crate::background::tasks::{BackgroundTask, TaskPriority};
use crate::spatial::{clustering::{Cluster, ClusteringConfig}, index::SpatialItem};
use crate::core::{bounds::Bounds, geo::Point};
use crate::Result;

#[cfg(feature = "debug")]
use log::{debug, info};

/// Task for performing marker clustering in the background
pub struct ClusterMarkersTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    items: Vec<SpatialItem<T>>,
    viewport_bounds: Bounds,
    zoom_level: f64,
    config: ClusteringConfig,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> ClusterMarkersTask<T> {
    pub fn new(
        task_id: String,
        items: Vec<SpatialItem<T>>,
        viewport_bounds: Bounds,
        zoom_level: f64,
        config: ClusteringConfig,
    ) -> Self {
        Self {
            task_id,
            items,
            viewport_bounds,
            zoom_level,
            config,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for ClusterMarkersTask<T> {
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>> {
        Box::pin(async move {
            let items = self.items.clone();
            let viewport_bounds = self.viewport_bounds.clone();
            let zoom_level = self.zoom_level;
            let config = self.config.clone();

            #[cfg(feature = "tokio-runtime")]
            let result = tokio::task::spawn_blocking(move || {
                // Perform clustering computation
                let clusters = cluster_items(items, viewport_bounds, zoom_level, config);
                clusters
            }).await
            .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))?;
            
            #[cfg(not(feature = "tokio-runtime"))]
            let result = {
                // Perform clustering computation synchronously
                cluster_items(items, viewport_bounds, zoom_level, config)
            };

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimate based on number of items
        let base_time = std::time::Duration::from_millis(20);
        let item_factor = (self.items.len() / 500).max(1) as u32;
        base_time * item_factor.min(25) // Cap at 500ms
    }
}

/// Task for updating clusters when the viewport changes
pub struct UpdateClustersTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    existing_clusters: Vec<Cluster<T>>,
    new_viewport_bounds: Bounds,
    new_zoom_level: f64,
    config: ClusteringConfig,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> UpdateClustersTask<T> {
    pub fn new(
        task_id: String,
        existing_clusters: Vec<Cluster<T>>,
        new_viewport_bounds: Bounds,
        new_zoom_level: f64,
        config: ClusteringConfig,
    ) -> Self {
        Self {
            task_id,
            existing_clusters,
            new_viewport_bounds,
            new_zoom_level,
            config,
            priority: TaskPriority::High, // High priority for viewport updates
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for UpdateClustersTask<T> {
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>> {
        Box::pin(async move {
            let existing_clusters = self.existing_clusters.clone();
            let new_viewport_bounds = self.new_viewport_bounds.clone();
            let new_zoom_level = self.new_zoom_level;
            let config = self.config.clone();

            let result = tokio::task::spawn_blocking(move || {
                // Extract all items from existing clusters
                let mut all_items = Vec::new();
                for cluster in existing_clusters {
                    all_items.extend(cluster.items);
                }

                // Re-cluster with new parameters
                let new_clusters = cluster_items(all_items, new_viewport_bounds, new_zoom_level, config);
                new_clusters
            }).await
            .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))?;

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Faster than initial clustering since we're reusing data
        let base_time = std::time::Duration::from_millis(10);
        let cluster_factor = (self.existing_clusters.len() / 100).max(1) as u32;
        base_time * cluster_factor.min(15) // Cap at 150ms
    }
}

/// Core clustering algorithm implementation
fn cluster_items<T: Clone>(
    items: Vec<SpatialItem<T>>,
    viewport_bounds: Bounds,
    zoom_level: f64,
    config: ClusteringConfig,
) -> Vec<Cluster<T>> {
    use std::collections::HashMap;

    // If zoom level is high enough, disable clustering
    if zoom_level >= config.disable_clustering_at_zoom {
        return items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                Cluster::new(format!("single_{}", i), vec![item], zoom_level)
            })
            .collect();
    }

    // Filter items to viewport
    let viewport_items: Vec<_> = items
        .into_iter()
        .filter(|item| viewport_bounds.intersects(&item.bounds))
        .collect();

    // Use grid-based clustering for performance
    let mut grid: HashMap<(i32, i32), Vec<SpatialItem<T>>> = HashMap::new();
    let grid_size = config.grid_size;

    // Group items by grid cell
    for item in viewport_items {
        let center = item.bounds.center();
        let grid_x = (center.x / grid_size).floor() as i32;
        let grid_y = (center.y / grid_size).floor() as i32;

        grid.entry((grid_x, grid_y))
            .or_default()
            .push(item);
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
        } else if cell_items.len() <= config.max_cluster_size {
            // Multiple items within limit - create cluster
            clusters.push(Cluster::new(
                format!("cluster_{}_{}", grid_x, grid_y),
                cell_items,
                zoom_level,
            ));
        } else {
            // Too many items - split into multiple clusters
            let chunks: Vec<_> = cell_items.chunks(config.max_cluster_size).collect();
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

/// Convenience functions for creating clustering background tasks
pub mod tasks {
    use super::*;

    /// Create a task to cluster markers
    pub fn cluster_markers<T: Clone + Send + Sync + 'static>(
        task_id: String,
        items: Vec<SpatialItem<T>>,
        viewport_bounds: Bounds,
        zoom_level: f64,
        config: ClusteringConfig,
    ) -> ClusterMarkersTask<T> {
        ClusterMarkersTask::new(task_id, items, viewport_bounds, zoom_level, config)
    }

    /// Create a task to update clusters
    pub fn update_clusters<T: Clone + Send + Sync + 'static>(
        task_id: String,
        existing_clusters: Vec<Cluster<T>>,
        new_viewport_bounds: Bounds,
        new_zoom_level: f64,
        config: ClusteringConfig,
    ) -> UpdateClustersTask<T> {
        UpdateClustersTask::new(task_id, existing_clusters, new_viewport_bounds, new_zoom_level, config)
    }
}