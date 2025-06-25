//! # Map-RS
//!
//! A comprehensive, Rust-native mapping library inspired by Leaflet.
//!
//! This library provides a modular, extensible architecture for building
//! interactive maps with support for various layer types, tile systems,
//! and user interactions.

pub mod animation;
pub mod core;
pub mod data;
pub mod input;
pub mod layers;
pub mod plugins;
pub mod rendering;
pub mod spatial;
pub mod ui;
pub mod tiles;
pub use crate::core::constants;

// Re-export public API
pub use core::{
    bounds::Bounds,
    geo::{LatLng, LatLngBounds, TileCoord},
    map::Map,
    viewport::Viewport,
};

pub use layers::{
    base::LayerTrait, canvas::CanvasLayer, image::ImageLayer, marker::Marker, tile::TileLayer,
    vector::VectorLayer,
};

pub use input::{events::InputEvent, handler::InputHandler};

pub use ui::{controls::MapControls, popup::Popup, widget::MapWidget};

pub use plugins::{
    base::PluginTrait, draw::DrawPlugin, heatmap::HeatmapPlugin, measure::MeasurePlugin,
};

pub use rendering::{context::RenderContext, pipeline::RenderPipeline};

pub use spatial::{clustering::Clustering, index::SpatialIndex};

pub use animation::{transitions::Transition, tweening::Tween};

pub use data::{formats::DataFormat, geojson::GeoJsonLayer};

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
}

/// Error type alias for convenience
pub type Error = MapError;
