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
//! ## Simple Usage (Recommended)
//!
//! ```rust
//! // Just add a map - works immediately!
//! ui.add(maplet::Map::new());
//!
//! // Or use helper functions
//! ui.map(); // Default location (San Francisco)
//! ui.map_at(51.5074, -0.1278); // London
//! ui.map_at_zoom(40.7128, -74.0060, 10.0); // New York with zoom
//!
//! // Or customize with builder pattern
//! ui.add(
//!     maplet::Map::new()
//!         .center(37.7749, -122.4194)
//!         .zoom(12)
//!         .size([800.0, 600.0])
//!         .theme(maplet::MapTheme::Dark)
//! );
//!
//! // Or use presets
//! ui.add(maplet::Map::san_francisco());
//! ui.add(maplet::Map::london());
//! ui.add(maplet::Map::tokyo());
//! ```
//!
//! ## Feature Flags
//!
//! - `egui`: Integration with egui UI framework (default)
//! - `wasm`: WASM compatibility layer (default)
//! - `tokio-runtime`: Tokio async runtime integration (default)
//! - `animations`: Animation system (default)
//! - `serde-support`: Serde serialization support (default)
//! - `debug`: Additional debugging and logging (default)
//! - `app`: Full application features (used by maplet-app)
//!
//! GPU rendering is now always available for the best map experience.

// Core modules (always available)

pub mod background;
pub mod core;
pub mod data;
pub mod input;
pub mod layers;
pub mod plugins;
pub mod prelude;
pub mod rendering;
pub mod runtime;
pub mod spatial;

// Feature-gated modules
#[cfg(feature = "egui")]
pub mod ui;

// Re-export public API
pub use core::{
    bounds::Bounds,
    builder::MapBuilder,
    config::{
        FrameTimingConfig, GpuRenderingConfig, InteractionAnimationConfig, MapPerformanceOptions,
        MapPerformanceProfile, TextureFilterMode, TileLoadingConfig,
    },
    geo::{LatLng, LatLngBounds, Point, TileCoord},
    map::{Map as CoreMap, MapOptions},
    viewport::Viewport,
};

pub use layers::{
    base::LayerTrait, canvas::CanvasLayer, image::ImageLayer, marker::Marker, tile::TileLayer,
    vector::VectorLayer,
};

pub use input::{events::InputEvent, handler::InputHandler};

#[cfg(feature = "egui")]
pub use ui::{
    controls::ControlManager, 
    popup::Popup, 
    widget::{AdvancedMapWidget, Map, MapCursor, MapTheme, MapWidgetConfig, MapWidgetExt},
    UiMapExt,
};

pub use plugins::{
    base::PluginTrait, draw::DrawPlugin, heatmap::HeatmapPlugin, measure::MeasurePlugin,
};

pub use rendering::{context::RenderContext, pipeline::RenderPipeline};

pub use spatial::{clustering::Clustering, index::SpatialIndex};

pub use layers::animation::{AnimationManager, EasingType, Transform};

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
