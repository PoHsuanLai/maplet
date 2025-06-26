//! Prelude module for common maplet types and traits
//!
//! This module re-exports the most commonly used types, traits, and functions
//! for easy importing with `use maplet::prelude::*;`

// Core types
pub use crate::core::{
    map::{Map, MapOptions},
    builder::MapBuilder,
    config::{
        MapPerformanceProfile, MapPerformanceOptions,
        FrameTimingConfig, TileLoadingConfig, InteractionAnimationConfig, GpuRenderingConfig,
        TextureFilterMode,
    },
    viewport::Viewport,
    geo::{LatLng, LatLngBounds, Point, TileCoord},
    bounds::Bounds,
};

// Layer system
pub use crate::layers::{
    base::LayerTrait,
    tile::TileLayer,
    vector::VectorLayer,
    marker::Marker,
    manager::LayerManager,
};

// Data types
pub use crate::data::{
    geojson::{GeoJson, GeoJsonLayer, GeoJsonFeature, FeatureStyle},
};

// Plugin system
pub use crate::plugins::{
    base::PluginTrait,
    draw::DrawPlugin,
    measure::MeasurePlugin,
    heatmap::HeatmapPlugin,
};

// Input handling
pub use crate::input::{
    events::{InputEvent, KeyCode, KeyModifiers},
    handler::InputHandler,
};

// Animation  
pub use crate::animation::{
    transitions::Transition,
    interpolation::Interpolatable,
};

// Spatial
pub use crate::spatial::{
    index::{SpatialIndex, SpatialItem},
    clustering::{Cluster, ClusteringConfig},
};

// Background tasks
pub use crate::background::{
    tasks::{BackgroundTask, TaskPriority, TaskId},
    geojson::GeoJsonParseTask,
};

// Runtime abstraction
pub use crate::runtime::{
    AsyncSpawner, AsyncHandle, AsyncHandleWithResult,
    spawn, spawn_with_result, runtime,
};

// Tile system
pub use crate::tiles::{
    loader::TileLoader,
    source::TileSource,
    cache::TileCache,
};

// Rendering (feature-gated)
#[cfg(feature = "render")]
pub use crate::rendering::{
    context::RenderContext,
    pipeline::RenderPipeline,
};

// UI integration (feature-gated)
#[cfg(feature = "egui")]
pub use crate::ui::{
    widget::MapWidget,
    controls::MapControls,
    style::MapStyle,
};

// Result and Error types
pub use crate::{Result, Error as MapError};

// Common standard library re-exports
pub use std::{
    sync::Arc,
    collections::HashMap,
    time::{Duration, Instant},
};

// Async re-exports
pub use futures::Future;