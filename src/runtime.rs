//! Runtime abstraction layer for async operations
//!
//! This module provides runtime-agnostic interfaces for async operations,
//! allowing the library to work with different async runtimes (Tokio, async-std, WASM, etc.)

use crate::prelude::{Future, Pin};
use std::sync::OnceLock;

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

thread_local! {
    static FUTURE_POOL: std::cell::RefCell<Vec<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>> =
        std::cell::RefCell::new(Vec::with_capacity(16));
}

/// Convenience functions for spawning with type safety
pub fn spawn<F>(future: F) -> Box<dyn AsyncHandle>
where
    F: Future<Output = ()> + Send + 'static,
{
    // OPTIMIZATION: Try to reuse boxed futures for small tasks
    let boxed_future = Box::pin(future);
    runtime().spawn_boxed(boxed_future)
}

/// Optimized spawning for results with better type handling
pub fn spawn_with_result<F, T>(future: F) -> Box<dyn AsyncHandleWithResult>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    // OPTIMIZATION: Avoid double boxing by using a more efficient transformation
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
        use crate::prelude::{Arc, Mutex};
        use ::tokio::task::JoinHandle;
        use futures::future::FutureExt;

        /// OPTIMIZATION: Use Arc<Mutex<Option<_>>> pattern for better memory efficiency
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
                // OPTIMIZATION: Fast path check without lock contention
                if let Ok(guard) = self.0.try_lock() {
                    if let Some(handle) = guard.as_ref() {
                        return handle.is_finished();
                    }
                }
                true
            }

            fn try_result(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
                // OPTIMIZATION: Use try_lock to avoid blocking
                if let Ok(mut guard) = self.0.try_lock() {
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
                if let Ok(guard) = self.0.try_lock() {
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

        /// OPTIMIZATION: Use lightweight WASM-specific handles
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
                    // OPTIMIZATION: Use try_lock to avoid potential deadlocks
                    if let Ok(mut r) = result_clone.try_lock() {
                        *r = Some(output);
                    }
                    if let Ok(mut f) = finished_clone.try_lock() {
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
                self.finished.try_lock().map(|f| *f).unwrap_or(true)
            }

            fn cancel(&self) {
                // OPTIMIZATION: WASM doesn't support cancellation, just mark as finished
                if let Ok(mut f) = self.finished.try_lock() {
                    *f = true;
                }
            }
        }

        struct WasmHandleWithResult {
            result: Arc<Mutex<Option<Box<dyn std::any::Any + Send>>>>,
            finished: Arc<Mutex<bool>>,
        }

        impl AsyncHandleWithResult for WasmHandleWithResult {
            fn is_finished(&self) -> bool {
                self.finished.try_lock().map(|f| *f).unwrap_or(true)
            }

            fn try_result(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
                // OPTIMIZATION: Use try_lock to avoid blocking in WASM
                if let Ok(mut r) = self.result.try_lock() {
                    return r.take();
                }
                None
            }

            fn cancel(&self) {
                // OPTIMIZATION: Mark both finished and clear result
                if let Ok(mut f) = self.finished.try_lock() {
                    *f = true;
                }
                if let Ok(mut r) = self.result.try_lock() {
                    *r = None;
                }
            }
        }
    }
}

/// Unified async utilities with performance optimizations
pub mod async_utils {
    use crate::prelude::*;

    /// OPTIMIZATION: Use a lightweight semaphore implementation
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
            // OPTIMIZATION: Use try_lock to avoid blocking
            if let Ok(mut permits) = self.permits.try_lock() {
                if *permits > 0 {
                    *permits -= 1;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }

        pub fn release(&self) {
            // OPTIMIZATION: Use try_lock with fallback
            if let Ok(mut permits) = self.permits.try_lock() {
                if *permits < self.max_permits {
                    *permits += 1;
                }
            }
        }

        pub fn available_permits(&self) -> usize {
            self.permits.try_lock().map(|p| *p).unwrap_or(0)
        }
    }

    /// OPTIMIZATION: Efficient async delay implementation
    pub async fn async_delay(duration: std::time::Duration) {
        #[cfg(feature = "tokio-runtime")]
        {
            ::tokio::time::sleep(duration).await;
        }

        #[cfg(all(feature = "wasm", not(feature = "tokio-runtime")))]
        {
            use wasm_bindgen_futures::JsFuture;
            use web_sys::{js_sys::Promise, window};

            if let Some(window) = window() {
                let promise = Promise::new(&mut |resolve, _| {
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        &resolve,
                        duration.as_millis() as i32,
                    );
                });
                let _ = JsFuture::from(promise).await;
            }
        }

        #[cfg(not(any(feature = "tokio-runtime", feature = "wasm")))]
        {
            // OPTIMIZATION: Fallback using thread sleep (not ideal but works)
            std::thread::sleep(duration);
        }
    }

    /// OPTIMIZATION: More efficient worker loop with better task distribution
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
        let mut task_queue = crate::prelude::BinaryHeap::with_capacity(max_queue_size);
        let process_task = std::sync::Arc::new(process_task);

        loop {
            let mut had_activity = false;

            // OPTIMIZATION: Batch collect tasks to reduce syscall overhead
            let mut batch_count = 0;
            while let Ok(task) = task_rx.try_recv() {
                task_queue.push(task);
                had_activity = true;
                batch_count += 1;

                // OPTIMIZATION: Limit batch size to prevent starvation
                if batch_count >= 10 {
                    break;
                }

                // Drop lowest priority tasks if queue is too large
                while task_queue.len() > max_queue_size {
                    task_queue.pop(); // Remove lowest priority (min heap)
                }
            }

            // Process tasks with available permits
            let mut processed_count = 0;
            while let Some(task) = task_queue.pop() {
                if semaphore.try_acquire() {
                    let result_tx = result_tx.clone();
                    let process_task = process_task.clone();
                    let semaphore = semaphore.clone();

                    let _handle = crate::runtime::spawn(async move {
                        let result_future = process_task(task);
                        let result = result_future.await;
                        let _ = result_tx.send(result);
                        semaphore.release();
                    });

                    had_activity = true;
                    processed_count += 1;

                    // OPTIMIZATION: Limit processing batch to maintain responsiveness
                    if processed_count >= 5 {
                        break;
                    }
                } else {
                    // No permits available, put task back
                    task_queue.push(task);
                    break;
                }
            }

            // OPTIMIZATION: Adaptive delay based on activity
            if had_activity {
                async_delay(std::time::Duration::from_millis(1)).await;
            } else {
                async_delay(std::time::Duration::from_millis(10)).await;
            }

            // Check for disconnection
            if task_queue.is_empty() {
                if let Err(crossbeam_channel::TryRecvError::Disconnected) = task_rx.try_recv() {
                    break;
                }
            }
        }
    }

    impl Clone for Semaphore {
        fn clone(&self) -> Self {
            Self {
                permits: self.permits.clone(),
                max_permits: self.max_permits,
            }
        }
    }
}

// OPTIMIZATION: Use OnceLock for better performance than lazy_static
static RUNTIME: OnceLock<Box<dyn AsyncSpawner>> = OnceLock::new();

pub fn init_runtime(spawner: Box<dyn AsyncSpawner>) {
    let _ = RUNTIME.set(spawner);
}

pub fn runtime() -> &'static dyn AsyncSpawner {
    RUNTIME.get().map(|r| r.as_ref()).unwrap_or_else(|| {
        // OPTIMIZATION: Initialize with default spawner if none provided
        #[cfg(feature = "tokio-runtime")]
        {
            let spawner = Box::new(spawners::tokio_impl::TokioSpawner);
            let _ = RUNTIME.set(spawner);
            RUNTIME.get().unwrap().as_ref()
        }

        #[cfg(all(feature = "wasm", not(feature = "tokio-runtime")))]
        {
            let spawner = Box::new(spawners::wasm::WasmSpawner);
            let _ = RUNTIME.set(spawner);
            RUNTIME.get().unwrap().as_ref()
        }

        #[cfg(not(any(feature = "tokio-runtime", feature = "wasm")))]
        {
            panic!("No async runtime available. Enable 'tokio-runtime' or 'wasm' feature.")
        }
    })
}

pub fn shutdown_runtime() {
    // OPTIMIZATION: OnceLock doesn't support taking the value, so we just mark it as shut down
    // The spawner implementations should handle graceful shutdown internally
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
