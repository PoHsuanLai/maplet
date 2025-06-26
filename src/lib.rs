//! # Maplet
//!
//! A modular, GPU-accelerated, async-aware Rust map engine that can be embedded
//! in any application or run as a standalone map viewer.
//!
//! ## Features
//!
//! - **Modular**: Use only the components you need via feature flags
//! - **GPU-Accelerated**: High-performance rendering with wgpu
//! - **Async-Aware**: Non-blocking tile loading and background processing  
//! - **Cross-Platform**: Works on desktop, web (WASM), and mobile
//! - **Extensible**: Plugin system for custom layers and functionality
//!
//! ## Feature Flags
//!
//! - `render`: GPU rendering support (default)
//! - `egui`: Integration with egui UI framework
//! - `wasm`: WASM compatibility layer
//! - `tokio-runtime`: Tokio async runtime integration
//! - `app`: Full application features (used by maplet-app)
//! - `debug`: Additional debugging and logging

// Core modules (always available)
pub mod animation;
pub mod background;
pub mod core;
pub mod data;
pub mod input;
pub mod layers;
pub mod plugins;
pub mod prelude;
pub mod runtime;
pub mod spatial;
pub mod tiles;

// Feature-gated modules
#[cfg(feature = "render")]
pub mod rendering;

#[cfg(feature = "egui")]
pub mod ui;

pub use crate::core::constants;

// Re-export public API
pub use core::{
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

pub use layers::{
    base::LayerTrait, canvas::CanvasLayer, image::ImageLayer, marker::Marker, tile::TileLayer,
    vector::VectorLayer,
};

pub use input::{events::InputEvent, handler::InputHandler};

#[cfg(feature = "egui")]
pub use ui::{controls::MapControls, popup::Popup, widget::MapWidget};

pub use plugins::{
    base::PluginTrait, draw::DrawPlugin, heatmap::HeatmapPlugin, measure::MeasurePlugin,
};

#[cfg(feature = "render")]
pub use rendering::{context::RenderContext, pipeline::RenderPipeline};

pub use spatial::{clustering::Clustering, index::SpatialIndex};

pub use animation::{transitions::Transition, tweening::Tween};

pub use data::{formats::DataFormat, geojson::GeoJsonLayer};

pub use background::{tasks::TaskPriority, BackgroundTaskManager};

/// Result type used throughout the library
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Common error types
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[cfg(feature = "render")]
    #[error("Render error: {0}")]
    Render(String),

    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),

    #[error("Layer error: {0}")]
    Layer(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),
}

/// Error type alias for convenience
pub type Error = MapError;
