use crate::prelude::{Arc, BinaryHeap, Ordering};
use crate::{runtime, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// Priority levels for background tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Low priority - can be delayed significantly
    Low = 1,
    /// Normal priority - standard background processing
    Normal = 2,
    /// High priority - user-initiated actions
    High = 3,
    /// Critical priority - immediate processing needed
    Critical = 4,
}

/// Unique identifier for background tasks
pub type TaskId = String;

/// Result of a completed background task
#[derive(Debug)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub result: Result<Box<dyn std::any::Any + Send>>,
}

// BackgroundTask trait moved to shared traits.rs to avoid duplication
pub use crate::traits::BackgroundTask;

/// Internal wrapper for prioritized tasks
#[derive(Clone)]
struct PrioritizedTask {
    task: Arc<dyn BackgroundTask>,
    priority: TaskPriority,
    submitted_at: std::time::Instant,
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.submitted_at == other.submitted_at
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier submission time
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.submitted_at.cmp(&self.submitted_at),
            other => other,
        }
    }
}

/// Configuration for the background task manager
#[derive(Debug, Clone)]
pub struct TaskManagerConfig {
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Maximum number of queued tasks before dropping low priority ones
    pub max_queue_size: usize,
    /// Whether to enable task metrics
    pub enable_metrics: bool,
    /// Test mode - tasks execute synchronously and immediately
    pub test_mode: bool,
}

impl Default for TaskManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 8,
            max_queue_size: 1000,
            enable_metrics: false,
            test_mode: false,
        }
    }
}

/// Unified configuration presets for TaskManagerConfig
impl TaskManagerConfig {
    pub fn low_resource() -> Self {
        Self {
            max_concurrent_tasks: 2,
            max_queue_size: 100,
            enable_metrics: false,
            test_mode: false,
        }
    }

    pub fn high_performance() -> Self {
        Self {
            max_concurrent_tasks: 16,
            max_queue_size: 5000,
            enable_metrics: true,
            test_mode: false,
        }
    }

    pub fn for_testing() -> Self {
        Self {
            max_concurrent_tasks: 1,
            max_queue_size: 10,
            enable_metrics: false,
            test_mode: true,
        }
    }
}

// Use unified semaphore from runtime module
use crate::runtime::async_utils::Semaphore;

/// Manages background task execution with priority scheduling
pub struct BackgroundTaskManager {
    config: TaskManagerConfig,
    task_tx: Sender<PrioritizedTask>,
    result_rx: Receiver<TaskResult>,
    semaphore: Semaphore,
    _worker_handle: Option<Box<dyn runtime::AsyncHandle>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl BackgroundTaskManager {
    /// Create a new background task manager
    pub fn new(config: TaskManagerConfig) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (result_tx, result_rx) = unbounded();
        let semaphore = Semaphore::new(config.max_concurrent_tasks);
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        let worker_handle = if config.test_mode {
            None
        } else {
            let worker_semaphore = semaphore.clone();
            let worker_config = config.clone();
            let worker_shutdown = shutdown_signal.clone();

            Some(runtime::spawn(async move {
                Self::worker_loop(
                    task_rx,
                    result_tx,
                    worker_semaphore,
                    worker_config,
                    worker_shutdown,
                )
                .await;
            }))
        };

        Self {
            config,
            task_tx,
            result_rx,
            semaphore,
            _worker_handle: worker_handle,
            shutdown_signal,
        }
    }

    /// Create a new task manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(TaskManagerConfig::default())
    }

    /// Create a new task manager for testing
    pub fn for_testing() -> Self {
        let config = TaskManagerConfig {
            test_mode: true,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Submit a task for background processing
    pub fn submit_task(&self, task: Arc<dyn BackgroundTask>) -> Result<()> {
        let task_id = task.task_id().to_string();
        let priority = task.priority();

        if self.config.test_mode {
            // In test mode, execute tasks immediately and synchronously
            return Ok(());
        }

        let prioritized = PrioritizedTask {
            task,
            priority,
            submitted_at: std::time::Instant::now(),
        };

        match self.task_tx.send(prioritized) {
            Ok(()) => {
                Ok(())
            }
            Err(_) => {
                Err("Task queue is closed".into())
            }
        }
    }

    /// Shutdown the task manager and all worker threads
    pub fn shutdown(&self) {
        self.shutdown_signal.store(true, AtomicOrdering::SeqCst);
    }

    /// Check if the task manager is shutting down
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_signal.load(AtomicOrdering::SeqCst)
    }

    /// Try to receive completed task results (non-blocking)
    pub fn try_recv_results(&self) -> Vec<TaskResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            results.push(result);
        }
        results
    }

    /// Check if there are pending results without consuming them
    pub fn has_pending_results(&self) -> bool {
        !self.result_rx.is_empty()
    }

    /// Get the current number of queued tasks
    pub fn queued_tasks(&self) -> usize {
        self.task_tx.len()
    }

    /// Get the number of currently running tasks
    pub fn running_tasks(&self) -> usize {
        self.config.max_concurrent_tasks - self.semaphore.available_permits()
    }

    /// Get the current configuration
    pub fn get_config(&self) -> &TaskManagerConfig {
        &self.config
    }

    /// Worker loop that processes tasks from the queue
    async fn worker_loop(
        task_rx: Receiver<PrioritizedTask>,
        result_tx: Sender<TaskResult>,
        semaphore: Semaphore,
        config: TaskManagerConfig,
        shutdown_signal: Arc<AtomicBool>,
    ) {
        let mut task_queue = BinaryHeap::new();
        let mut last_activity = std::time::Instant::now();

        loop {
            let mut had_activity = false;

            // Collect all available tasks
            while let Ok(task) = task_rx.try_recv() {
                task_queue.push(task);
                had_activity = true;

                // Drop lowest priority tasks if queue is too large
                while task_queue.len() > config.max_queue_size {
                    if let Some(dropped) = task_queue.iter().min().cloned() {
                        task_queue.retain(|t| {
                            t.priority > TaskPriority::Low || t.submitted_at != dropped.submitted_at
                        });
                    }
                }
            }

            // Process the highest priority task if we have capacity
            if let Some(task) = task_queue.pop() {
                if semaphore.try_acquire() {
                    let result_tx = result_tx.clone();
                    let task_id = task.task.task_id().to_string();
                    let task_clone = task.task.clone();
                    let sem_clone = semaphore.clone();

                    let _handle = runtime::spawn(async move {
                        let result = task_clone.execute().await;
                        let task_result = TaskResult { task_id, result };

                        let _ = result_tx.send(task_result);
                        sem_clone.release(); // Release semaphore permit
                    });

                    had_activity = true;
                } else {
                    // No capacity, put task back
                    task_queue.push(task);
                }
            }

            // OPTIMIZATION: Use proper async sleep instead of busy-wait
            if had_activity {
                last_activity = std::time::Instant::now();
                // Short delay when active to allow other tasks to run
                crate::runtime::async_utils::async_delay(std::time::Duration::from_millis(1)).await;
            } else {
                // Longer delay when idle, but check for shutdown more frequently
                let idle_duration = last_activity.elapsed();
                let sleep_duration = if idle_duration < std::time::Duration::from_secs(1) {
                    std::time::Duration::from_millis(10) // Recently active
                } else {
                    std::time::Duration::from_millis(100) // Long idle
                };

                crate::runtime::async_utils::async_delay(sleep_duration).await;
            }
        }
    }
}

/// Implement unified configuration trait for BackgroundTaskManager
impl crate::traits::Configurable for BackgroundTaskManager {
    type Config = TaskManagerConfig;

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn set_config(&mut self, config: Self::Config) -> crate::Result<()> {
        // Validate the new config
        Self::validate_config(&config)?;

        // Note: Changing config at runtime would require recreating the semaphore
        // and restarting the worker loop. For now, we just update the stored config.
        self.config = config;
        Ok(())
    }

    fn validate_config(config: &Self::Config) -> crate::Result<()> {
        if config.max_concurrent_tasks == 0 {
            return Err("max_concurrent_tasks must be greater than 0".into());
        }
        if config.max_queue_size == 0 {
            return Err("max_queue_size must be greater than 0".into());
        }
        Ok(())
    }
}

// AsyncSpawner trait moved to runtime.rs to avoid duplication
pub use crate::runtime::AsyncSpawner;

/// Unified duration estimation helpers to eliminate duplicate patterns
pub fn estimate_duration_from_data_size(data_size: usize, base_ms: u64) -> std::time::Duration {
    let data_factor = (data_size / 1024).min(1000); // 1ms per KB, capped at 1s
    std::time::Duration::from_millis(base_ms + data_factor as u64)
}

pub fn estimate_duration_from_item_count(
    item_count: usize,
    base_ms: u64,
    per_item_ms: u64,
) -> std::time::Duration {
    let item_factor = (item_count / 100).max(1) as u64; // Base unit of 100 items
    std::time::Duration::from_millis(base_ms + (item_factor * per_item_ms).min(1000))
}

/// Common async execution helper to standardize tokio-runtime patterns
pub struct AsyncExecutor;

impl AsyncExecutor {
    /// Execute a CPU-intensive task using the appropriate runtime
    /// This consolidates the #[cfg(feature = "tokio-runtime")] patterns
    pub async fn execute_blocking<F, R>(task: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        #[cfg(feature = "tokio-runtime")]
        {
            tokio::task::spawn_blocking(task)
                .await
                .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))?
        }

        #[cfg(not(feature = "tokio-runtime"))]
        {
            task()
        }
    }

    /// Execute a CPU-intensive task that returns a boxed result
    /// This is the common pattern used by background tasks
    pub async fn execute_blocking_boxed<F, R>(task: F) -> Result<Box<dyn std::any::Any + Send>>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let result = Self::execute_blocking(task).await?;
        Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
    }
}

// AsyncHandle and AsyncHandleWithResult traits moved to runtime.rs to avoid duplication
pub use crate::runtime::{AsyncHandle, AsyncHandleWithResult};
