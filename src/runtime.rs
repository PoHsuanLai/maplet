//! Runtime abstraction layer for async operations
//!
//! This module provides runtime-agnostic interfaces for async operations,
//! allowing the library to work with different async runtimes (Tokio, async-std, WASM, etc.)

use std::future::Future;
use std::pin::Pin;

/// A trait for spawning async tasks (object-safe version)
pub trait AsyncSpawner: Send + Sync + 'static {
    /// Spawn a future and return a handle to it
    fn spawn_boxed(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> Box<dyn AsyncHandle>;
    
    /// Spawn a future that returns a value  
    fn spawn_with_result_boxed(&self, future: Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>>) -> Box<dyn AsyncHandleWithResult>;
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
    runtime().spawn_boxed(Box::pin(future))
}

pub fn spawn_with_result<F, T>(future: F) -> Box<dyn AsyncHandleWithResult>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
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
        use std::sync::{Arc, Mutex};
        use ::tokio::task::JoinHandle;
        use futures::future::FutureExt;
        
        /// Tokio-based async spawner
        pub struct TokioSpawner;
        
        impl AsyncSpawner for TokioSpawner {
            fn spawn_boxed(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> Box<dyn AsyncHandle> {
                let handle = ::tokio::spawn(future);
                Box::new(TokioHandle(handle))
            }
            
            fn spawn_with_result_boxed(&self, future: Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>>) -> Box<dyn AsyncHandleWithResult> {
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
        
        struct TokioHandleWithResult(Arc<Mutex<Option<JoinHandle<Box<dyn std::any::Any + Send>>>>>);
        
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
        use std::sync::{Arc, Mutex};
        
        /// WASM-compatible async spawner
        pub struct WasmSpawner;
        
        impl AsyncSpawner for WasmSpawner {
            fn spawn_boxed(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> Box<dyn AsyncHandle> {
                wasm_bindgen_futures::spawn_local(future);
                Box::new(WasmHandle { finished: Arc::new(Mutex::new(false)) })
            }
            
            fn spawn_with_result_boxed(&self, future: Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send>> + Send + 'static>>) -> Box<dyn AsyncHandleWithResult> {
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

/// Global runtime instance
static mut RUNTIME: Option<Box<dyn AsyncSpawner>> = None;
static RUNTIME_INIT: std::sync::Once = std::sync::Once::new();

/// Initialize the runtime with a specific spawner
pub fn init_runtime(spawner: Box<dyn AsyncSpawner>) {
    RUNTIME_INIT.call_once(|| {
        unsafe {
            RUNTIME = Some(spawner);
        }
    });
}

/// Get the global runtime spawner
pub fn runtime() -> &'static dyn AsyncSpawner {
    RUNTIME_INIT.call_once(|| {
        #[cfg(feature = "tokio-runtime")]
        {
            unsafe {
                RUNTIME = Some(Box::new(spawners::tokio_impl::TokioSpawner));
            }
        }
        
        #[cfg(all(feature = "wasm", not(feature = "tokio-runtime")))]
        {
            unsafe {
                RUNTIME = Some(Box::new(spawners::wasm::WasmSpawner));
            }
        }
        
        #[cfg(not(any(feature = "tokio-runtime", feature = "wasm")))]
        {
            panic!("No async runtime available. Enable 'tokio-runtime' or 'wasm' feature.");
        }
    });
    
    unsafe { RUNTIME.as_ref().unwrap().as_ref() }
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