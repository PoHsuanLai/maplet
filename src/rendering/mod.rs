pub mod camera;
pub mod context;
pub mod pipeline;
pub mod resources;

// Re-export main types
pub use camera::Camera;
pub use context::RenderContext;
pub use pipeline::{PipelineConfig, RenderPassType, RenderPipeline};
pub use resources::{ResourceStats, Resources};

pub mod shaders {
    pub const TILE_VERTEX: &str = include_str!("shaders/tile.wgsl");
    pub const VECTOR_VERTEX: &str = include_str!("shaders/vector.wgsl");
}
