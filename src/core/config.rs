//! Configuration system for map performance and behavior tuning
//!
//! This module provides a hierarchical configuration system that allows users
//! to easily configure different aspects of the map rendering engine through
//! presets or custom configurations.

#[derive(Debug, Clone, PartialEq)]
pub enum MapPerformanceProfile {
    Balanced,
    LowQuality,
    HighQuality,
    Custom(MapPerformanceOptions),
}

impl MapPerformanceProfile {
    pub fn resolve(&self) -> MapPerformanceOptions {
        match self {
            Self::Balanced => MapPerformanceOptions {
                framerate: FrameTimingConfig {
                    target_fps: Some(60),
                    render_on_idle: false,
                    min_update_interval_ms: 16,
                },
                tile_loader: TileLoadingConfig {
                    cache_size: 1024,
                    fetch_batch_size: 6,
                    lazy_eviction: true,
                    prefetch_buffer: 2,
                    max_retries: 3,
                    retry_delay_ms: 500,
                    exponential_backoff: true,
                    error_tile_url: None,
                    show_parent_tiles: true,
                    preload_zoom_tiles: true,
                },

                rendering: GpuRenderingConfig {
                    msaa_samples: 4,
                    texture_filter: TextureFilterMode::Linear,
                    enable_vector_smoothing: true,
                    glyph_atlas_max_bytes: 2_000_000,
                },
            },
            Self::LowQuality => MapPerformanceOptions {
                framerate: FrameTimingConfig {
                    target_fps: Some(30),
                    render_on_idle: false,
                    min_update_interval_ms: 33,
                },
                tile_loader: TileLoadingConfig {
                    cache_size: 256,
                    fetch_batch_size: 2,
                    lazy_eviction: true,
                    prefetch_buffer: 1,
                    max_retries: 2,
                    retry_delay_ms: 250,
                    exponential_backoff: false,
                    error_tile_url: None,
                    show_parent_tiles: false,
                    preload_zoom_tiles: false,
                },

                rendering: GpuRenderingConfig {
                    msaa_samples: 0,
                    texture_filter: TextureFilterMode::Nearest,
                    enable_vector_smoothing: false,
                    glyph_atlas_max_bytes: 512_000,
                },
            },
            Self::HighQuality => MapPerformanceOptions {
                framerate: FrameTimingConfig {
                    target_fps: Some(120),
                    render_on_idle: true,
                    min_update_interval_ms: 8,
                },
                tile_loader: TileLoadingConfig {
                    cache_size: 4096,
                    fetch_batch_size: 12,
                    lazy_eviction: false,
                    prefetch_buffer: 3,
                    max_retries: 5,
                    retry_delay_ms: 1000,
                    exponential_backoff: true,
                    error_tile_url: None,
                    show_parent_tiles: true,
                    preload_zoom_tiles: true,
                },
                rendering: GpuRenderingConfig {
                    msaa_samples: 8,
                    texture_filter: TextureFilterMode::Anisotropic(16),
                    enable_vector_smoothing: true,
                    glyph_atlas_max_bytes: 8_000_000,
                },
            },
            Self::Custom(options) => options.clone(),
        }
    }
}

impl Default for MapPerformanceProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Unified configuration manager for all map-related configurations
#[derive(Debug, Clone, Default)]
pub struct UnifiedMapConfig {
    pub performance: MapPerformanceOptions,
    pub task_manager: crate::background::tasks::TaskManagerConfig,
    pub tile_loader: crate::layers::tile::loader::TileLoaderConfig,
    #[cfg(feature = "egui")]
    pub ui_controls: crate::ui::controls::ControlConfig,
}

/// Unified configuration presets that affect all subsystems
impl UnifiedMapConfig {
    /// Low resource configuration for mobile or constrained environments
    pub fn low_resource() -> Self {
        Self {
            performance: MapPerformanceProfile::LowQuality.resolve(),
            task_manager: crate::background::tasks::TaskManagerConfig::low_resource(),
            tile_loader: crate::layers::tile::loader::TileLoaderConfig::low_resource(),
            #[cfg(feature = "egui")]
            ui_controls: crate::ui::controls::ControlConfig::minimal(),
        }
    }

    /// High performance configuration for desktop applications
    pub fn high_performance() -> Self {
        Self {
            performance: MapPerformanceProfile::HighQuality.resolve(),
            task_manager: crate::background::tasks::TaskManagerConfig::high_performance(),
            tile_loader: crate::layers::tile::loader::TileLoaderConfig::high_performance(),
            #[cfg(feature = "egui")]
            ui_controls: crate::ui::controls::ControlConfig::full_controls(),
        }
    }

    /// Testing configuration with reduced resources and timeouts
    pub fn for_testing() -> Self {
        Self {
            performance: MapPerformanceProfile::LowQuality.resolve(),
            task_manager: crate::background::tasks::TaskManagerConfig::for_testing(),
            tile_loader: crate::layers::tile::loader::TileLoaderConfig::for_testing(),
            #[cfg(feature = "egui")]
            ui_controls: crate::ui::controls::ControlConfig::minimal(),
        }
    }

    /// Mobile-friendly configuration
    pub fn mobile_optimized() -> Self {
        Self {
            performance: MapPerformanceProfile::LowQuality.resolve(),
            task_manager: crate::background::tasks::TaskManagerConfig::low_resource(),
            tile_loader: crate::layers::tile::loader::TileLoaderConfig::low_resource(),
            #[cfg(feature = "egui")]
            ui_controls: crate::ui::controls::ControlConfig::mobile_friendly(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapPerformanceOptions {
    pub framerate: FrameTimingConfig,
    pub tile_loader: TileLoadingConfig,
    pub rendering: GpuRenderingConfig,
}

impl Default for MapPerformanceOptions {
    fn default() -> Self {
        MapPerformanceProfile::default().resolve()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameTimingConfig {
    pub target_fps: Option<u32>,
    pub render_on_idle: bool,
    pub min_update_interval_ms: u64,
}

impl FrameTimingConfig {
    pub fn target_frame_duration_ms(&self) -> Option<u64> {
        self.target_fps.map(|fps| 1000 / fps as u64)
    }

    // Removed should_render() and should_update() - timing is now controlled by UpdateOrchestrator
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileLoadingConfig {
    pub cache_size: usize,
    pub fetch_batch_size: usize,
    pub lazy_eviction: bool,
    pub prefetch_buffer: u32,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub exponential_backoff: bool,
    pub error_tile_url: Option<String>,
    pub show_parent_tiles: bool,
    pub preload_zoom_tiles: bool,
}

impl TileLoadingConfig {
    pub fn estimated_memory_usage(&self) -> usize {
        self.cache_size * 15_000
    }

    pub fn recommended_concurrent_tasks(&self) -> usize {
        (self.fetch_batch_size * 2).clamp(4, 16)
    }
}

impl Default for TileLoadingConfig {
    fn default() -> Self {
        Self {
            cache_size: 1024,
            fetch_batch_size: 8,
            lazy_eviction: true,
            prefetch_buffer: 2,
            max_retries: 3,
            retry_delay_ms: 500,
            exponential_backoff: true,
            error_tile_url: None,
            show_parent_tiles: true,
            preload_zoom_tiles: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuRenderingConfig {
    pub msaa_samples: u32,
    pub texture_filter: TextureFilterMode,
    pub enable_vector_smoothing: bool,
    pub glyph_atlas_max_bytes: usize,
}

impl GpuRenderingConfig {
    pub fn is_msaa_enabled(&self) -> bool {
        self.msaa_samples > 0
    }

    pub fn wgpu_sample_count(&self) -> u32 {
        if self.msaa_samples == 0 {
            1
        } else {
            self.msaa_samples.next_power_of_two().min(8)
        }
    }

    pub fn estimated_vram_usage_mb(&self) -> f32 {
        let glyph_atlas_mb = self.glyph_atlas_max_bytes as f32 / 1_048_576.0;
        let msaa_overhead = if self.is_msaa_enabled() {
            self.msaa_samples as f32 * 0.5
        } else {
            0.0
        };

        glyph_atlas_mb + msaa_overhead + 16.0 // Base overhead
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilterMode {
    Nearest,
    Linear,
    Anisotropic(u8),
}

impl TextureFilterMode {
    pub fn to_wgpu_filter(&self) -> (wgpu::FilterMode, wgpu::FilterMode) {
        match self {
            Self::Nearest => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
            Self::Linear | Self::Anisotropic(_) => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
            }
        }
    }

    pub fn anisotropy_level(&self) -> u8 {
        match self {
            Self::Anisotropic(level) => *level,
            _ => 1,
        }
    }
}

impl Default for TextureFilterMode {
    fn default() -> Self {
        Self::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_profile_presets() {
        let balanced = MapPerformanceProfile::Balanced.resolve();
        let low_quality = MapPerformanceProfile::LowQuality.resolve();
        let high_quality = MapPerformanceProfile::HighQuality.resolve();

        // Balanced should have reasonable defaults
        assert_eq!(balanced.framerate.target_fps, Some(60));
        assert_eq!(balanced.tile_loader.cache_size, 1024);

        // Low quality should prioritize performance
        assert_eq!(low_quality.framerate.target_fps, Some(30));
        assert!(low_quality.tile_loader.cache_size < balanced.tile_loader.cache_size);

        // High quality should prioritize visual fidelity
        assert_eq!(high_quality.framerate.target_fps, Some(120)); // Zed-style 120fps targeting
        assert!(high_quality.tile_loader.cache_size > balanced.tile_loader.cache_size);
        assert!(high_quality.rendering.msaa_samples > balanced.rendering.msaa_samples);
    }

    #[test]
    fn test_frame_timing_config() {
        let config = FrameTimingConfig {
            target_fps: Some(60),
            render_on_idle: false,
            min_update_interval_ms: 16,
        };

        assert_eq!(config.target_frame_duration_ms(), Some(16));

        // Timing decisions are now handled by UpdateOrchestrator, not FrameTimingConfig
    }

    #[test]
    fn test_tile_loading_config() {
        let config = TileLoadingConfig {
            cache_size: 1000,
            fetch_batch_size: 8,
            lazy_eviction: true,
            prefetch_buffer: 2,
            max_retries: 3,
            retry_delay_ms: 500,
            exponential_backoff: true,
            error_tile_url: None,
            show_parent_tiles: true,
            preload_zoom_tiles: true,
        };

        assert!(config.estimated_memory_usage() > 0);
        assert!(config.recommended_concurrent_tasks() >= 4);
        assert!(config.recommended_concurrent_tasks() <= 16);
    }

    #[test]
    fn test_texture_filter_mode() {
        assert_eq!(TextureFilterMode::Nearest.anisotropy_level(), 1);
        assert_eq!(TextureFilterMode::Linear.anisotropy_level(), 1);
        assert_eq!(TextureFilterMode::Anisotropic(16).anisotropy_level(), 16);
    }

    #[test]
    fn test_gpu_rendering_config() {
        let config = GpuRenderingConfig {
            msaa_samples: 4,
            texture_filter: TextureFilterMode::Linear,
            enable_vector_smoothing: true,
            glyph_atlas_max_bytes: 1_000_000,
        };

        assert!(config.is_msaa_enabled());
        assert_eq!(config.wgpu_sample_count(), 4);
        assert!(config.estimated_vram_usage_mb() > 0.0);
    }
}
