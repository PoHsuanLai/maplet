//! Prelude module for common maplet types and traits
//!
//! This module re-exports the most commonly used types, traits, and functions
//! for easy importing with `use maplet::prelude::*;`

// Core types
pub use crate::core::{
    bounds::Bounds,
    builder::MapBuilder,
    config::{
        FrameTimingConfig, GpuRenderingConfig, InteractionAnimationConfig, MapPerformanceOptions,
        MapPerformanceProfile, TextureFilterMode, TileLoadingConfig,
    },
    geo::{LatLng, LatLngBounds, Point, TileCoord},
    map::{Map, MapOptions},
    viewport::Viewport,
};

// Layer system
pub use crate::layers::{
    base::LayerTrait, manager::LayerManager, marker::Marker, tile::TileLayer, vector::VectorLayer,
};

// Data types
pub use crate::data::geojson::{FeatureStyle, GeoJson, GeoJsonFeature, GeoJsonLayer};

// Plugin system
pub use crate::plugins::{
    base::PluginTrait, draw::DrawPlugin, heatmap::HeatmapPlugin, measure::MeasurePlugin,
};

// Input handling
pub use crate::input::{
    events::{EventHandled, InputEvent, KeyCode, KeyModifiers},
    gestures::GestureRecognizer,
    handler::{InputEventHandler, InputHandler},
};

// Animation
pub use crate::animation::{
    interpolation::Interpolation,
    transitions::{Transition, TransitionManager, TransitionType},
    tweening::{Tween, TweenManager, Tweenable},
};

// Spatial
pub use crate::spatial::{
    clustering::{Cluster, Clustering},
    index::{SpatialIndex, SpatialItem},
};

// Background tasks
pub use crate::background::tasks::{BackgroundTask, BackgroundTaskManager, TaskManagerConfig, TaskPriority};

// Runtime abstraction
pub use crate::runtime::{
    runtime, spawn, spawn_with_result, AsyncHandle, AsyncHandleWithResult, AsyncSpawner,
};

// Tile system
pub use crate::tiles::{
    cache::TileCache,
    loader::{TileLoader, TileLoaderConfig},
    source::TileSource,
};

// Rendering (feature-gated)
#[cfg(feature = "render")]
pub use crate::rendering::{context::RenderContext, pipeline::RenderPipeline};

// UI integration (feature-gated)
#[cfg(feature = "egui")]
pub use crate::ui::{controls::MapControls, style::MapStyle, widget::MapWidget};

// Result and Error types
pub use crate::{Error as MapError, Result};

// Common standard library re-exports with better performance hashmaps
pub use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Use FxHashMap and FxHashSet for better performance
pub use fxhash::{FxHashMap as HashMap, FxHashSet as HashSet, FxHasher};

// Async re-exports
#[cfg(feature = "tokio-runtime")]
pub use futures::Future;
