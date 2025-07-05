//! Shared trait abstractions for common patterns
//!
//! This module provides reusable trait abstractions that eliminate
//! code duplication across different parts of the codebase.

use crate::{
    core::{bounds::Bounds, geo::{LatLng, Point}},
    Result,
};
use std::future::Future;
use std::pin::Pin;

/// Trait for coordinate transformation operations
/// Unifies the coordinate transformation patterns found across the codebase
pub trait CoordinateTransform {
    /// Transform from one coordinate system to another
    fn transform_point(&self, point: Point) -> Result<Point>;
    
    /// Transform geographic coordinates to pixel coordinates
    fn project(&self, lat_lng: LatLng, zoom: f64) -> Point;
    
    /// Transform pixel coordinates to geographic coordinates  
    fn unproject(&self, point: Point, zoom: f64) -> LatLng;
    
    /// Get the bounds transformation for a given zoom level
    fn transform_bounds(&self, bounds: Bounds, zoom: f64) -> Bounds;
}

/// Trait for background tasks that can be executed asynchronously
/// This is an alias for AsyncProcessor that matches the BackgroundTask interface
pub trait BackgroundTask: Send + Sync {
    /// Execute the task and return the result
    fn execute(&self) -> Pin<Box<dyn Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>>;
    
    /// Get the task ID
    fn task_id(&self) -> &str;
    
    /// Get the task priority
    fn priority(&self) -> crate::background::tasks::TaskPriority {
        crate::background::tasks::TaskPriority::Normal
    }
    
    /// Get an estimate of task duration (for scheduling)
    fn estimated_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(100)
    }
}

/// Trait for spatial operations
/// Unifies spatial indexing and querying patterns
pub trait SpatialOperations<T> {
    /// Insert an item into the spatial structure
    fn insert(&mut self, id: String, bounds: Bounds, data: T) -> Result<()>;
    
    /// Remove an item by ID
    fn remove(&mut self, id: &str) -> Result<Option<T>>;
    
    /// Query items within bounds
    fn query(&self, bounds: &Bounds) -> Vec<&T>;
    
    /// Query items within radius of a point
    fn query_radius(&self, center: &Point, radius: f64) -> Vec<&T>;
    
    /// Clear all items
    fn clear(&mut self);
    
    /// Get the number of items
    fn len(&self) -> usize;
    
    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Trait for renderable components
/// Unifies rendering patterns across UI and layer systems
pub trait Renderable {
    type Context;
    type Error;
    
    /// Render the component
    fn render(&mut self, context: &mut Self::Context) -> std::result::Result<(), Self::Error>;
    
    /// Check if the component is visible
    fn is_visible(&self) -> bool;
    
    /// Set visibility
    fn set_visible(&mut self, visible: bool);
    
    /// Get the component bounds (if applicable)
    fn bounds(&self) -> Option<Bounds> {
        None
    }
}

/// Specialized UI renderable trait for egui components
#[cfg(feature = "egui")]
pub trait UiRenderable {
    /// Render the UI component using egui
    fn render(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> Result<egui::Response>;
    
    /// Check if the component is visible
    fn is_visible(&self) -> bool;
    
    /// Set visibility
    fn set_visible(&mut self, visible: bool);
}

/// Trait for viewport-aware components
/// Standardizes viewport change handling
pub trait ViewportAware {
    /// Handle viewport changes
    fn on_viewport_changed(&mut self, viewport: &crate::core::viewport::Viewport) -> Result<()>;
    
    /// Check if component requires viewport updates
    fn requires_viewport_updates(&self) -> bool {
        true
    }
}

/// Trait for configurable components
/// Unifies configuration patterns
pub trait Configurable {
    type Config: Clone;
    
    /// Get the current configuration
    fn config(&self) -> &Self::Config;
    
    /// Set new configuration
    fn set_config(&mut self, config: Self::Config) -> Result<()>;
    
    /// Validate configuration
    fn validate_config(config: &Self::Config) -> Result<()> {
        let _ = config; // Default implementation accepts all configs
        Ok(())
    }
}

/// Trait for cacheable operations
/// Standardizes caching patterns used across the codebase
pub trait Cacheable {
    type Key: Clone + Eq + std::hash::Hash;
    type Value: Clone;
    
    /// Get cached value
    fn get_cached(&self, key: &Self::Key) -> Option<Self::Value>;
    
    /// Cache a value
    fn cache(&mut self, key: Self::Key, value: Self::Value);
    
    /// Invalidate cache entry
    fn invalidate(&mut self, key: &Self::Key);
    
    /// Clear entire cache
    fn clear_cache(&mut self);
    
    /// Get cache statistics
    fn cache_stats(&self) -> CacheStats {
        CacheStats::default()
    }
}

/// Cache statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

/// Trait for layer-like objects
/// Unifies common layer operations and extends LayerTrait functionality
pub trait LayerOperations: Send + Sync {
    /// Get layer ID
    fn id(&self) -> &str;
    
    /// Get layer name
    fn name(&self) -> &str;
    
    /// Get layer type
    fn layer_type(&self) -> crate::layers::base::LayerType;
    
    /// Check if layer is visible
    fn is_visible(&self) -> bool;
    
    /// Set layer visibility
    fn set_visible(&mut self, visible: bool);
    
    /// Get layer opacity (0.0 to 1.0)
    fn opacity(&self) -> f32;
    
    /// Set layer opacity
    fn set_opacity(&mut self, opacity: f32);
    
    /// Get layer z-index for ordering
    fn z_index(&self) -> i32;
    
    /// Set layer z-index
    fn set_z_index(&mut self, z_index: i32);
    
    /// Layer lifecycle events
    fn on_add(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }
    
    fn on_remove(&self, _map: &mut crate::core::map::Map) -> Result<()> {
        Ok(())
    }
    
    /// Render the layer
    fn render(&mut self, context: &mut crate::rendering::context::RenderContext, viewport: &crate::core::viewport::Viewport) -> Result<()>;
    
    /// Handle input events
    fn handle_input(&mut self, _input: &crate::input::events::InputEvent) -> Result<()> {
        Ok(())
    }
    
    /// Update layer state
    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }
    
    /// Get layer bounds if applicable
    fn bounds(&self) -> Option<crate::core::geo::LatLngBounds> {
        None
    }
    
    /// Check if layer intersects with given bounds
    fn intersects_bounds(&self, bounds: &crate::core::geo::LatLngBounds) -> bool {
        if let Some(layer_bounds) = self.bounds() {
            layer_bounds.intersects(bounds)
        } else {
            true
        }
    }
    
    /// Get layer options
    fn options(&self) -> serde_json::Value;
    
    /// Set layer options
    fn set_options(&mut self, options: serde_json::Value) -> Result<()>;
    
    /// Dynamic casting support
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
} 