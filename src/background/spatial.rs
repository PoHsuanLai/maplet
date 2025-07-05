use crate::background::tasks::{AsyncExecutor, BackgroundTask, TaskPriority};
use crate::core::{bounds::Bounds, geo::Point};
use crate::spatial::index::{SpatialIndex, SpatialItem};
use crate::Result;

/// Task for building a spatial index from a large set of items
pub struct BuildSpatialIndexTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    items: Vec<SpatialItem<T>>,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> BuildSpatialIndexTask<T> {
    pub fn new(task_id: String, items: Vec<SpatialItem<T>>) -> Self {
        Self {
            task_id,
            items,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for BuildSpatialIndexTask<T> {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let items = self.items.clone();

            AsyncExecutor::execute_blocking_boxed(move || {
                let mut index = SpatialIndex::new();
                for item in items {
                    index.insert(item)?;
                }
                Ok(index)
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
        // Estimate based on number of items (R-tree construction is O(n log n))
        let base_time = std::time::Duration::from_millis(10);
        let item_factor = ((self.items.len() as f64).log2().ceil() as u32).max(1);
        base_time * item_factor.min(50) // Cap at 500ms
    }
}

/// Task for performing spatial queries on an index
pub struct SpatialQueryTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    index: SpatialIndex<T>,
    query_bounds: Bounds,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> SpatialQueryTask<T> {
    pub fn new(task_id: String, index: SpatialIndex<T>, query_bounds: Bounds) -> Self {
        Self {
            task_id,
            index,
            query_bounds,
            priority: TaskPriority::High, // Queries are usually user-initiated
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for SpatialQueryTask<T> {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let index = self.index.clone();
            let query_bounds = self.query_bounds.clone();

            AsyncExecutor::execute_blocking_boxed(move || {
                let items = index.query(&query_bounds);
                // Clone the items to own them
                let owned_items: Vec<SpatialItem<T>> = items.into_iter().cloned().collect();
                Ok(owned_items)
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
        // Spatial queries are typically fast
        std::time::Duration::from_millis(5)
    }
}

/// Task for performing radius-based spatial queries
pub struct RadiusQueryTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    index: SpatialIndex<T>,
    center: Point,
    radius: f64,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> RadiusQueryTask<T> {
    pub fn new(task_id: String, index: SpatialIndex<T>, center: Point, radius: f64) -> Self {
        Self {
            task_id,
            index,
            center,
            radius,
            priority: TaskPriority::High,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for RadiusQueryTask<T> {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let index = self.index.clone();
            let center = self.center;
            let radius = self.radius;

            AsyncExecutor::execute_blocking_boxed(move || {
                let items = index.query_radius(&center, radius);
                // Clone the items to own them
                let owned_items: Vec<SpatialItem<T>> = items.into_iter().cloned().collect();
                Ok(owned_items)
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
        // Radius queries are typically fast
        std::time::Duration::from_millis(5)
    }
}

/// Represents different types of index updates
#[derive(Debug, Clone)]
pub enum IndexUpdate<T> {
    /// Insert a new item
    Insert(SpatialItem<T>),
    /// Remove an item by ID
    Remove(String),
    /// Update an existing item
    Update(String, SpatialItem<T>),
}

/// Task for batch updating a spatial index
pub struct BatchUpdateIndexTask<T: Clone + Send + Sync + 'static> {
    task_id: String,
    index: SpatialIndex<T>,
    updates: Vec<IndexUpdate<T>>,
    priority: TaskPriority,
}

impl<T: Clone + Send + Sync + 'static> BatchUpdateIndexTask<T> {
    pub fn new(
        task_id: String,
        index: SpatialIndex<T>,
        updates: Vec<IndexUpdate<T>>,
    ) -> Self {
        Self {
            task_id,
            index,
            updates,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl<T: Clone + Send + Sync + 'static> BackgroundTask for BatchUpdateIndexTask<T> {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let mut index = self.index.clone();
            let updates = self.updates.clone();

            AsyncExecutor::execute_blocking_boxed(move || {
                for update in updates {
                    match update {
                        IndexUpdate::Insert(item) => {
                            index.insert(item)?;
                        }
                        IndexUpdate::Remove(id) => {
                            index.remove(&id)?;
                        }
                        IndexUpdate::Update(id, new_item) => {
                            index.update(&id, new_item)?;
                        }
                    }
                }
                Ok(index)
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
        // Estimate based on number of updates
        let base_time = std::time::Duration::from_millis(2);
        let update_factor = (self.updates.len() / 100).max(1) as u32;
        base_time * update_factor.min(50) // Cap at 100ms
    }
}

/// Convenience functions for creating spatial indexing background tasks
pub mod tasks {
    use super::*;

    /// Create a task to build a spatial index
    pub fn build_spatial_index<T: Clone + Send + Sync + 'static>(
        task_id: String,
        items: Vec<SpatialItem<T>>,
    ) -> BuildSpatialIndexTask<T> {
        BuildSpatialIndexTask::new(task_id, items)
    }

    /// Create a task to query a spatial index
    pub fn spatial_query<T: Clone + Send + Sync + 'static>(
        task_id: String,
        index: SpatialIndex<T>,
        query_bounds: Bounds,
    ) -> SpatialQueryTask<T> {
        SpatialQueryTask::new(task_id, index, query_bounds)
    }

    /// Create a task for radius-based queries
    pub fn radius_query<T: Clone + Send + Sync + 'static>(
        task_id: String,
        index: SpatialIndex<T>,
        center: Point,
        radius: f64,
    ) -> RadiusQueryTask<T> {
        RadiusQueryTask::new(task_id, index, center, radius)
    }

    /// Create a task for batch updates
    pub fn batch_update_index<T: Clone + Send + Sync + 'static>(
        task_id: String,
        index: SpatialIndex<T>,
        updates: Vec<IndexUpdate<T>>,
    ) -> BatchUpdateIndexTask<T> {
        BatchUpdateIndexTask::new(task_id, index, updates)
    }
}
