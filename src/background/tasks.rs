use crate::{runtime, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
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

/// Simple semaphore implementation for concurrency control
#[derive(Debug)]
struct SimpleSemaphore {
    permits: std::sync::Arc<std::sync::Mutex<usize>>,
    max_permits: usize,
}

impl SimpleSemaphore {
    fn new(permits: usize) -> Self {
        Self {
            permits: std::sync::Arc::new(std::sync::Mutex::new(permits)),
            max_permits: permits,
        }
    }

    fn try_acquire(&self) -> bool {
        if let Ok(mut permits) = self.permits.lock() {
            if *permits > 0 {
                *permits -= 1;
                return true;
            }
        }
        false
    }

    fn release(&self) {
        if let Ok(mut permits) = self.permits.lock() {
            if *permits < self.max_permits {
                *permits += 1;
            }
        }
    }

    fn available_permits(&self) -> usize {
        self.permits.lock().map(|permits| *permits).unwrap_or(0)
    }
}

/// Manages background task execution with priority scheduling
pub struct BackgroundTaskManager {
    config: TaskManagerConfig,
    task_tx: Sender<PrioritizedTask>,
    result_rx: Receiver<TaskResult>,
    semaphore: Arc<SimpleSemaphore>,
    _worker_handle: Option<Box<dyn runtime::AsyncHandle>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl BackgroundTaskManager {
    /// Create a new background task manager
    pub fn new(config: TaskManagerConfig) -> Self {
        println!("🏗️ [DEBUG] BackgroundTaskManager::new() - Creating task manager with max_concurrent={}, max_queue_size={}", 
            config.max_concurrent_tasks, config.max_queue_size);
        
        let (task_tx, task_rx) = unbounded();
        let (result_tx, result_rx) = unbounded();
        let semaphore = Arc::new(SimpleSemaphore::new(config.max_concurrent_tasks));
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        let worker_handle = if config.test_mode {
            println!("🧪 [DEBUG] BackgroundTaskManager::new() - Test mode enabled, skipping worker loop");
            None
        } else {
            let worker_semaphore = semaphore.clone();
            let worker_config = config.clone();
            let worker_shutdown = shutdown_signal.clone();

            println!("🚀 [DEBUG] BackgroundTaskManager::new() - Spawning worker loop");
            Some(runtime::spawn(async move {
                Self::worker_loop(task_rx, result_tx, worker_semaphore, worker_config, worker_shutdown).await;
            }))
        };

        println!("✅ [DEBUG] BackgroundTaskManager::new() - Task manager created successfully");
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
        println!("⚙️ [DEBUG] BackgroundTaskManager::with_default_config() - Creating task manager with default config");
        Self::new(TaskManagerConfig::default())
    }

    /// Create a new task manager for testing
    pub fn for_testing() -> Self {
        println!("🧪 [DEBUG] BackgroundTaskManager::for_testing() - Creating test task manager");
        let config = TaskManagerConfig{ test_mode: true, ..Default::default() };
        Self::new(config)
    }

    /// Submit a task for background processing
    pub fn submit_task(&self, task: Arc<dyn BackgroundTask>) -> Result<()> {
        let task_id = task.task_id().to_string();
        let priority = task.priority();
        
        println!("📝 [DEBUG] BackgroundTaskManager::submit_task() - Submitting task '{}' with priority {:?}", task_id, priority);

        if self.config.test_mode {
            println!("🧪 [DEBUG] BackgroundTaskManager::submit_task() - Test mode: executing task synchronously");
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
                println!("✅ [DEBUG] BackgroundTaskManager::submit_task() - Task '{}' queued successfully", task_id);
                Ok(())
            }
            Err(_) => {
                println!("❌ [DEBUG] BackgroundTaskManager::submit_task() - Failed to queue task '{}' (channel closed)", task_id);
                Err("Task queue is closed".into())
            }
        }
    }

    /// Shutdown the task manager and all worker threads
    pub fn shutdown(&self) {
        println!("🛑 [DEBUG] BackgroundTaskManager::shutdown() - Shutting down task manager");
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

    /// Worker loop that processes tasks from the queue
    async fn worker_loop(
        task_rx: Receiver<PrioritizedTask>,
        result_tx: Sender<TaskResult>,
        semaphore: Arc<SimpleSemaphore>,
        config: TaskManagerConfig,
        shutdown_signal: Arc<AtomicBool>,
    ) {
        let mut task_queue = BinaryHeap::new();

        loop {
            // Collect all available tasks
            while let Ok(task) = task_rx.try_recv() {
                task_queue.push(task);

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
                } else {
                    // No capacity, put task back
                    task_queue.push(task);
                }
            }

            // Brief pause to prevent busy-waiting
            // This is a simple approach - in production you might want more sophisticated timing
            let sleep_duration = if task_queue.is_empty() { 100 } else { 10 };

            // Use a simple delay mechanism that works across runtimes
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_millis(sleep_duration) {
                // Simple busy-wait - could be improved with platform-specific sleep
                std::hint::spin_loop();
            }

            // Check if we should exit (all channels closed and no tasks)
            if task_queue.is_empty() && task_rx.is_empty() {
                // Check if the channel is actually disconnected
                if task_rx.try_recv().is_err() {
                    // Channel is likely disconnected, continue anyway
                    continue;
                }
            }

            // Check if we should exit due to shutdown signal
            if shutdown_signal.load(AtomicOrdering::SeqCst) {
                break;
            }
        }
    }
}

// AsyncSpawner trait moved to runtime.rs to avoid duplication
pub use crate::runtime::AsyncSpawner;

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
