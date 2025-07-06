//! Prelude module for common maplet types and traits
//!
//! This module re-exports the most commonly used types, traits, and functions
//! for easy importing with `use maplet::prelude::*;`

pub use crate::core::{
    bounds::Bounds,
    builder::MapBuilder,
    config::{
        FrameTimingConfig, GpuRenderingConfig, MapPerformanceOptions, MapPerformanceProfile,
        TextureFilterMode, TileLoadingConfig, UnifiedMapConfig,
    },
    geo::{LatLng, LatLngBounds, Point, TileCoord},
    map::{Map as CoreMap, MapOptions, UpdateOrchestrator, UpdatePerformanceMetrics},
    viewport::Viewport,
};

pub use crate::layers::{
    base::LayerTrait, manager::LayerManager, marker::Marker, tile::TileLayer, vector::VectorLayer,
};

pub use crate::data::geojson::{FeatureStyle, GeoJson, GeoJsonFeature, GeoJsonLayer};

pub use crate::plugins::{
    base::PluginTrait, draw::DrawPlugin, heatmap::HeatmapPlugin, measure::MeasurePlugin,
};

pub use crate::input::{
    events::{EventHandled, InputEvent, KeyCode, KeyModifiers},
    handler::{Action, InputHandler},
};

pub use crate::layers::animation::{AnimationManager, ZoomAnimation, ZoomAnimationState};

pub use crate::core::viewport::Transform;

pub use crate::spatial::{
    clustering::{Cluster, Clustering},
    index::{SpatialIndex, SpatialItem},
};

pub use crate::background::tasks::{BackgroundTaskManager, TaskManagerConfig, TaskPriority};

pub use crate::runtime::{
    runtime, spawn, spawn_with_result, AsyncHandle, AsyncHandleWithResult, AsyncSpawner,
};

pub use crate::layers::tile::{
    cache::TileCache,
    loader::{TileLoader, TileLoaderConfig},
    source::TileSource,
};

pub use crate::rendering::{context::RenderContext, pipeline::RenderPipeline};

#[cfg(feature = "egui")]
pub use crate::ui::{
    components::*,
    elements::UiManager,
    style::{MapStyle, MapThemes},
    traits::*,
    widget::{Map, MapTheme},
    UiMapExt,
};

pub use crate::{Error as MapError, Result};

pub use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
    future::Future,
    pin::Pin,
    sync::Arc,
    sync::Mutex,
    time::{Duration, Instant},
};

pub use fxhash::{FxHashMap as HashMap, FxHashSet as HashSet, FxHasher};

// Future is now included in std imports above

// Shared traits for common patterns
pub use crate::traits::{
    BackgroundTask, CacheStats, Cacheable, ConfigBuilder, ConfigPreset, Configurable,
    CoordinateTransform, LayerOperations, Renderable, SpatialOperations, ViewportAware,
};

#[cfg(feature = "egui")]
pub use crate::traits::UiRenderable;
