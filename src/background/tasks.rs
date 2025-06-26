use std::sync::Arc;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use crossbeam_channel::{Receiver, Sender, unbounded};
use crate::{runtime, Result};

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

/// A background task that can be executed asynchronously
/// Made object-safe by removing generic associated types
pub trait BackgroundTask: Send + Sync {
    /// Execute the task
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>>;
    
    /// Get the task ID
    fn task_id(&self) -> &str;
    
    /// Get the task priority
    fn priority(&self) -> TaskPriority;
    
    /// Get an estimate of task duration (for scheduling)
    fn estimated_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(100) // Default 100ms
    }
}

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
}

impl Default for TaskManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 8,
            max_queue_size: 1000,
            enable_metrics: false,
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
    _worker_handle: Box<dyn runtime::AsyncHandle>,
}

impl BackgroundTaskManager {
    /// Create a new background task manager
    pub fn new(config: TaskManagerConfig) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (result_tx, result_rx) = unbounded();
        let semaphore = Arc::new(SimpleSemaphore::new(config.max_concurrent_tasks));
        
        let worker_semaphore = semaphore.clone();
        let worker_config = config.clone();
        
        let worker_handle = runtime::spawn(async move {
            Self::worker_loop(task_rx, result_tx, worker_semaphore, worker_config).await;
        });

        Self {
            config,
            task_tx,
            result_rx,
            semaphore,
            _worker_handle: worker_handle,
        }
    }

    /// Create a new task manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(TaskManagerConfig::default())
    }

    /// Submit a task for background processing
    pub fn submit_task(&self, task: Arc<dyn BackgroundTask>) -> Result<()> {
        let priority = task.priority();
        
        let prioritized = PrioritizedTask {
            task,
            priority,
            submitted_at: std::time::Instant::now(),
        };

        self.task_tx.send(prioritized)
            .map_err(|_| crate::Error::Plugin("Task queue is closed".to_string()))?;

        Ok(())
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
    ) {
        let mut task_queue = BinaryHeap::new();
        
        loop {
            // Collect all available tasks
            while let Ok(task) = task_rx.try_recv() {
                task_queue.push(task);
                
                // Drop lowest priority tasks if queue is too large
                while task_queue.len() > config.max_queue_size {
                    if let Some(dropped) = task_queue.iter().min().cloned() {
                        task_queue.retain(|t| t.priority > TaskPriority::Low || t.submitted_at != dropped.submitted_at);
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
                        let task_result = TaskResult {
                            task_id,
                            result,
                        };
                        
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
                if let Err(_) = task_rx.try_recv() {
                    // Channel is likely disconnected, continue anyway
                    continue;
                }
            }
        }
    }
} 