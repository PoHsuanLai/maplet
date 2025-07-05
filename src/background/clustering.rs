//! Background clustering tasks
//!
//! This module provides background tasks for spatial clustering operations
//! to keep CPU-intensive clustering computations off the main thread.

use crate::background::tasks::{AsyncExecutor, BackgroundTask, TaskPriority};
use crate::core::bounds::Bounds;
use crate::spatial::{
    clustering::{Cluster, Clustering, ClusteringConfig},
    index::SpatialItem,
};
use crate::Result;

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
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let items = self.items.clone();
            let viewport_bounds = self.viewport_bounds.clone();
            let zoom_level = self.zoom_level;
            let config = self.config.clone();

            AsyncExecutor::execute_blocking_boxed(move || {
                // Use the core clustering implementation
                let mut clustering = Clustering::new(config);
                
                // Add all items to the clustering system
                for item in items {
                    let _ = clustering.add_item(item); // Ignore errors for background processing
                }
                
                // Get clusters for the viewport
                Ok(clustering.get_clusters(&viewport_bounds, zoom_level))
            }).await
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimate based on number of items (grid clustering is O(n))
        let base_time = std::time::Duration::from_millis(5);
        let item_factor = (self.items.len() / 1000).max(1) as u32;
        base_time * item_factor.min(20) // Cap at 100ms
    }
}

/// Task for updating existing clusters with new viewport parameters
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
            priority: TaskPriority::High, // Updates are usually time-sensitive
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for UpdateClustersTask<T> {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let existing_clusters = self.existing_clusters.clone();
            let new_viewport_bounds = self.new_viewport_bounds.clone();
            let new_zoom_level = self.new_zoom_level;
            let config = self.config.clone();

            AsyncExecutor::execute_blocking_boxed(move || {
                // Use the core clustering implementation
                let mut clustering = Clustering::new(config);
                
                // Extract all items from existing clusters and add to clustering system
                for cluster in existing_clusters {
                    for item in cluster.items {
                        let _ = clustering.add_item(item); // Ignore errors for background processing
                    }
                }
                
                // Re-cluster with new parameters
                Ok(clustering.get_clusters(&new_viewport_bounds, new_zoom_level))
            }).await
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

/// Convenience functions for creating clustering background tasks
pub mod tasks {
    use super::*;

    /// Create a clustering task from a set of items
    pub fn cluster_markers<T: Clone + Send + Sync + 'static>(
        task_id: String,
        items: Vec<SpatialItem<T>>,
        viewport_bounds: Bounds,
        zoom_level: f64,
        config: ClusteringConfig,
    ) -> ClusterMarkersTask<T> {
        ClusterMarkersTask::new(task_id, items, viewport_bounds, zoom_level, config)
    }

    /// Create a task to update existing clusters
    pub fn update_clusters<T: Clone + Send + Sync + 'static>(
        task_id: String,
        existing_clusters: Vec<Cluster<T>>,
        new_viewport_bounds: Bounds,
        new_zoom_level: f64,
        config: ClusteringConfig,
    ) -> UpdateClustersTask<T> {
        UpdateClustersTask::new(
            task_id,
            existing_clusters,
            new_viewport_bounds,
            new_zoom_level,
            config,
        )
    }
}
