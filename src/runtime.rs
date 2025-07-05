//! Runtime abstraction layer for async operations
//!
//! This module provides runtime-agnostic interfaces for async operations,
//! allowing the library to work with different async runtimes (Tokio, async-std, WASM, etc.)

use crate::prelude::{Future, Pin};

/// A trait for spawning async tasks (object-safe version)
pub trait AsyncSpawner: Send + Sync + 'static {
    /// Spawn a future and return a handle to it
    fn spawn_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Box<dyn AsyncHandle>;

    /// Spawn a future that returns a value  
    fn spawn_with_result_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>>,
    ) -> Box<dyn AsyncHandleWithResult>;
}

/// Handle to a spawned async task
pub trait AsyncHandle: Send + Sync {
    /// Check if the task is finished
    fn is_finished(&self) -> bool;

    /// Cancel the task
    fn cancel(&self);
}

/// Handle to a spawned async task that returns a result
pub trait AsyncHandleWithResult: Send + Sync {
    /// Check if the task is finished
    fn is_finished(&self) -> bool;

    /// Try to get the result if available
    fn try_result(&mut self) -> Option<Box<dyn std::any::Any + Send>>;

    /// Cancel the task
    fn cancel(&self);
}

/// Convenience functions for spawning with type safety
pub fn spawn<F>(future: F) -> Box<dyn AsyncHandle>
where
    F: Future<Output = ()> + Send + 'static,
{
    println!("🚀 [DEBUG] runtime::spawn() - Spawning new async task");
    runtime().spawn_boxed(Box::pin(future))
}

pub fn spawn_with_result<F, T>(future: F) -> Box<dyn AsyncHandleWithResult>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    println!("🚀 [DEBUG] runtime::spawn_with_result() - Spawning new async task with result");
    let boxed_future = Box::pin(async move {
        let result = future.await;
        Box::new(result) as Box<dyn std::any::Any + Send>
    });
    runtime().spawn_with_result_boxed(boxed_future)
}

/// Default spawner implementations
pub mod spawners {
    use super::*;

    #[cfg(feature = "tokio-runtime")]
    pub mod tokio_impl {
        use super::*;
        use ::tokio::task::JoinHandle;
        use futures::future::FutureExt;
        use crate::prelude::{Arc, Mutex};

        /// Type alias for tokio handle with result
        type TokioHandleResult = Arc<Mutex<Option<JoinHandle<Box<dyn std::any::Any + Send>>>>>;

        /// Tokio-based async spawner
        pub struct TokioSpawner;

        impl AsyncSpawner for TokioSpawner {
            fn spawn_boxed(
                &self,
                future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
            ) -> Box<dyn AsyncHandle> {
                let handle = ::tokio::spawn(future);
                Box::new(TokioHandle(handle))
            }

            fn spawn_with_result_boxed(
                &self,
                future: Pin<
                    Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>,
                >,
            ) -> Box<dyn AsyncHandleWithResult> {
                let handle = ::tokio::spawn(future);
                Box::new(TokioHandleWithResult(Arc::new(Mutex::new(Some(handle)))))
            }
        }

        struct TokioHandle(JoinHandle<()>);

        impl AsyncHandle for TokioHandle {
            fn is_finished(&self) -> bool {
                self.0.is_finished()
            }

            fn cancel(&self) {
                self.0.abort();
            }
        }

        struct TokioHandleWithResult(TokioHandleResult);

        impl AsyncHandleWithResult for TokioHandleWithResult {
            fn is_finished(&self) -> bool {
                if let Ok(guard) = self.0.lock() {
                    if let Some(handle) = guard.as_ref() {
                        return handle.is_finished();
                    }
                }
                true
            }

            fn try_result(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
                if let Ok(mut guard) = self.0.lock() {
                    if let Some(handle) = guard.take() {
                        if handle.is_finished() {
                            return handle.now_or_never().and_then(|r| r.ok());
                        } else {
                            *guard = Some(handle);
                        }
                    }
                }
                None
            }

            fn cancel(&self) {
                if let Ok(guard) = self.0.lock() {
                    if let Some(handle) = guard.as_ref() {
                        handle.abort();
                    }
                }
            }
        }
    }

    #[cfg(feature = "wasm")]
    pub mod wasm {
        use super::*;
        use crate::prelude::{Arc, Mutex};

        /// WASM-compatible async spawner
        pub struct WasmSpawner;

        impl AsyncSpawner for WasmSpawner {
            fn spawn_boxed(
                &self,
                future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
            ) -> Box<dyn AsyncHandle> {
                wasm_bindgen_futures::spawn_local(future);
                Box::new(WasmHandle {
                    finished: Arc::new(Mutex::new(false)),
                })
            }

            fn spawn_with_result_boxed(
                &self,
                future: Pin<
                    Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>,
                >,
            ) -> Box<dyn AsyncHandleWithResult> {
                let result = Arc::new(Mutex::new(None));
                let result_clone = result.clone();
                let finished = Arc::new(Mutex::new(false));
                let finished_clone = finished.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let output = future.await;
                    if let Ok(mut r) = result_clone.lock() {
                        *r = Some(output);
                    }
                    if let Ok(mut f) = finished_clone.lock() {
                        *f = true;
                    }
                });

                Box::new(WasmHandleWithResult { result, finished })
            }
        }

        struct WasmHandle {
            finished: Arc<Mutex<bool>>,
        }

        impl AsyncHandle for WasmHandle {
            fn is_finished(&self) -> bool {
                self.finished.lock().map(|f| *f).unwrap_or(true)
            }

            fn cancel(&self) {
                // WASM tasks can't be cancelled easily, just mark as finished
                if let Ok(mut finished) = self.finished.lock() {
                    *finished = true;
                }
            }
        }

        struct WasmHandleWithResult {
            result: Arc<Mutex<Option<Box<dyn std::any::Any + Send>>>>,
            finished: Arc<Mutex<bool>>,
        }

        impl AsyncHandleWithResult for WasmHandleWithResult {
            fn is_finished(&self) -> bool {
                self.finished.lock().map(|f| *f).unwrap_or(true)
            }

            fn try_result(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
                if self.is_finished() {
                    if let Ok(mut result) = self.result.lock() {
                        return result.take();
                    }
                }
                None
            }

            fn cancel(&self) {
                if let Ok(mut finished) = self.finished.lock() {
                    *finished = true;
                }
            }
        }
    }
}

/// Unified async utilities to consolidate duplicate patterns
pub mod async_utils {
    use super::*;
    
    /// Unified semaphore implementation to replace multiple custom semaphores
    #[derive(Debug, Clone)]
    pub struct Semaphore {
        permits: std::sync::Arc<std::sync::Mutex<usize>>,
        max_permits: usize,
    }
    
    impl Semaphore {
        pub fn new(permits: usize) -> Self {
            Self {
                permits: std::sync::Arc::new(std::sync::Mutex::new(permits)),
                max_permits: permits,
            }
        }
        
        pub fn try_acquire(&self) -> bool {
            if let Ok(mut permits) = self.permits.lock() {
                if *permits > 0 {
                    *permits -= 1;
                    return true;
                }
            }
            false
        }
        
        pub fn release(&self) {
            if let Ok(mut permits) = self.permits.lock() {
                if *permits < self.max_permits {
                    *permits += 1;
                }
            }
        }
        
        pub fn available_permits(&self) -> usize {
            self.permits.lock().map(|permits| *permits).unwrap_or(0)
        }
    }
    
    /// Unified async delay function that works across runtimes
    pub async fn async_delay(duration: std::time::Duration) {
        #[cfg(feature = "tokio-runtime")]
        {
            tokio::time::sleep(duration).await;
        }
        
        #[cfg(not(feature = "tokio-runtime"))]
        {
            // Use a simple async delay that doesn't block
            let start = std::time::Instant::now();
            while start.elapsed() < duration {
                std::hint::spin_loop();
            }
        }
    }
    
    /// Unified worker loop pattern to eliminate duplicate implementations
    pub async fn unified_worker_loop<T, R>(
        task_rx: crossbeam_channel::Receiver<T>,
        result_tx: crossbeam_channel::Sender<R>,
        semaphore: Semaphore,
        max_queue_size: usize,
        process_task: impl Fn(T) -> Pin<Box<dyn Future<Output = R> + Send>> + Send + Sync + 'static,
    ) where
        T: Send + Sync + 'static + Ord + Clone,
        R: Send + Sync + 'static,
    {
        let mut task_queue = std::collections::BinaryHeap::new();
        let process_task = std::sync::Arc::new(process_task);
        
        loop {
            // Collect all available tasks
            while let Ok(task) = task_rx.try_recv() {
                task_queue.push(task);
                
                // Drop tasks if queue is too large
                while task_queue.len() > max_queue_size {
                    task_queue.pop();
                }
            }
            
            // Process the highest priority task if we have capacity
            if let Some(task) = task_queue.pop() {
                if semaphore.try_acquire() {
                    let result_tx = result_tx.clone();
                    let process_task = process_task.clone();
                    let semaphore = semaphore.clone();
                    
                    spawn(async move {
                        let result = process_task(task).await;
                        let _ = result_tx.send(result);
                        semaphore.release();
                    });
                } else {
                    // No capacity, put task back
                    task_queue.push(task);
                }
            }
            
            // Brief pause to prevent busy-waiting
            let sleep_duration = if task_queue.is_empty() { 
                std::time::Duration::from_millis(100) 
            } else { 
                std::time::Duration::from_millis(10) 
            };
            
            async_delay(sleep_duration).await;
            
            // Check if we should exit (channel closed and no tasks)
            if task_queue.is_empty() && task_rx.is_empty() && task_rx.try_recv().is_err() {
                break;
            }
        }
    }
}

/// Global runtime instance  
static RUNTIME: std::sync::OnceLock<Box<dyn AsyncSpawner>> = std::sync::OnceLock::new();

/// Initialize the runtime with a specific spawner
pub fn init_runtime(spawner: Box<dyn AsyncSpawner>) {
    let _ = RUNTIME.set(spawner);
}

/// Get the global runtime spawner
pub fn runtime() -> &'static dyn AsyncSpawner {
    RUNTIME
        .get_or_init(|| {
            #[cfg(feature = "tokio-runtime")]
            {
                Box::new(spawners::tokio_impl::TokioSpawner)
            }

            #[cfg(all(feature = "wasm", not(feature = "tokio-runtime")))]
            {
                Box::new(spawners::wasm::WasmSpawner)
            }

            #[cfg(not(any(feature = "tokio-runtime", feature = "wasm")))]
            {
                panic!("No async runtime available. Enable 'tokio-runtime' or 'wasm' feature.");
            }
        })
        .as_ref()
}

/// Shutdown the runtime gracefully
pub fn shutdown_runtime() {
    // Currently this is a minimal implementation
    // In the future, this could cancel all active handles
    log::debug!("Runtime shutdown requested");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "tokio-runtime")]
    #[::tokio::test]
    async fn test_tokio_spawner() {
        let handle = spawn(async {
            ::tokio::time::sleep(::tokio::time::Duration::from_millis(10)).await;
        });

        // Should not be finished immediately
        assert!(!handle.is_finished());

        // Wait a bit and check again
        ::tokio::time::sleep(::tokio::time::Duration::from_millis(20)).await;
        assert!(handle.is_finished());
    }
}
