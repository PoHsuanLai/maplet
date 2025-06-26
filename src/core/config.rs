//! Configuration system for map performance and behavior tuning
//!
//! This module provides a hierarchical configuration system that allows users
//! to easily configure different aspects of the map rendering engine through
//! presets or custom configurations.

use crate::animation::interpolation::EasingFunction;

/// Top-level performance profile enum representing presets optimized for different environments
#[derive(Debug, Clone, PartialEq)]
pub enum MapPerformanceProfile {
    /// Default balanced behavior - good for most applications
    Balanced,
    /// Aggressive optimizations for slow hardware or battery use
    LowQuality,
    /// Maximum fidelity, smoother transitions, heavier rendering
    HighQuality,
    /// User-defined configuration
    Custom(MapPerformanceOptions),
}

impl MapPerformanceProfile {
    /// Resolve the profile to a concrete configuration
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
                animation: InteractionAnimationConfig {
                    enable_transitions: true,
                    pan_easing: EasingFunction::EaseOutCubic,
                    zoom_easing: EasingFunction::EaseOutCubic,
                    max_zoom_step_per_frame: 0.15,
                    zoom_animation_threshold: 0.05,
                    zoom_duration_ms: 350,
                    pan_duration_ms: 300,
                    zoom_to_cursor: true,
                    use_transform_animations: true,
                    smooth_wheel_zoom: true,
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
                animation: InteractionAnimationConfig {
                    enable_transitions: false,
                    pan_easing: EasingFunction::Linear,
                    zoom_easing: EasingFunction::Linear,
                    max_zoom_step_per_frame: 0.5,
                    zoom_animation_threshold: 0.05,
                    zoom_duration_ms: 350,
                    pan_duration_ms: 200,
                    zoom_to_cursor: true,
                    use_transform_animations: true,
                    smooth_wheel_zoom: true,
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
                    target_fps: None, // Uncapped
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
                animation: InteractionAnimationConfig {
                    enable_transitions: true,
                    pan_easing: EasingFunction::EaseInOutBack,
                    zoom_easing: EasingFunction::EaseInOutExpo,
                    max_zoom_step_per_frame: 0.05,
                    zoom_animation_threshold: 0.05,
                    zoom_duration_ms: 350,
                    pan_duration_ms: 400,
                    zoom_to_cursor: true,
                    use_transform_animations: true,
                    smooth_wheel_zoom: true,
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

/// Full configuration struct containing all subsystem configurations
#[derive(Debug, Clone, PartialEq)]
pub struct MapPerformanceOptions {
    pub framerate: FrameTimingConfig,
    pub tile_loader: TileLoadingConfig,
    pub animation: InteractionAnimationConfig,
    pub rendering: GpuRenderingConfig,
}

impl Default for MapPerformanceOptions {
    fn default() -> Self {
        MapPerformanceProfile::default().resolve()
    }
}

/// Controls redraw behavior, frame pacing, and throttling
#[derive(Debug, Clone, PartialEq)]
pub struct FrameTimingConfig {
    /// Target maximum framerate (e.g., 60 = 60Hz, None = uncapped)
    pub target_fps: Option<u32>,
    /// If true, continue rendering idle frames (useful for smooth zoom, but costly)
    pub render_on_idle: bool,
    /// Minimum milliseconds between logic updates (debounces state refresh)
    pub min_update_interval_ms: u64,
}

impl FrameTimingConfig {
    /// Get the target frame duration in milliseconds
    pub fn target_frame_duration_ms(&self) -> Option<u64> {
        self.target_fps.map(|fps| 1000 / fps as u64)
    }

    /// Check if we should render based on timing constraints
    pub fn should_render(&self, last_render_time: std::time::Instant) -> bool {
        if self.render_on_idle {
            return true;
        }

        let elapsed = last_render_time.elapsed();
        if let Some(target_duration) = self.target_frame_duration_ms() {
            elapsed.as_millis() >= target_duration as u128
        } else {
            true
        }
    }

    /// Check if we should update logic based on timing constraints
    pub fn should_update(&self, last_update_time: std::time::Instant) -> bool {
        let elapsed = last_update_time.elapsed();
        elapsed.as_millis() >= self.min_update_interval_ms as u128
    }
}

/// Controls how many tiles are downloaded and cached
#[derive(Debug, Clone, PartialEq)]
pub struct TileLoadingConfig {
    /// Number of tiles to cache in memory
    pub cache_size: usize,
    /// Number of concurrent tile fetches
    pub fetch_batch_size: usize,
    /// If true, evict tiles lazily (after animation completes)
    pub lazy_eviction: bool,
    /// Number of extra tile layers to prefetch around the visible area
    pub prefetch_buffer: u32,
    /// Maximum number of retry attempts for failed tiles
    pub max_retries: u32,
    /// Base delay in milliseconds between retry attempts
    pub retry_delay_ms: u64,
    /// Whether to use exponential backoff for retries
    pub exponential_backoff: bool,
    /// URL for fallback/error tiles (like Leaflet's errorTileUrl)
    pub error_tile_url: Option<String>,
    /// Whether to show parent tiles while loading children (smooth zoom)
    pub show_parent_tiles: bool,
    /// Whether to preload tiles for next zoom level during zoom animation
    pub preload_zoom_tiles: bool,
}

impl TileLoadingConfig {
    /// Get the memory budget in bytes (approximate)
    pub fn estimated_memory_usage(&self) -> usize {
        // Estimate: average tile is ~15KB (varies by format and compression)
        self.cache_size * 15_000
    }

    /// Get recommended concurrent task limit for background processing
    pub fn recommended_concurrent_tasks(&self) -> usize {
        // Balance between fetch batch size and system resources
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

/// Controls how pan/zoom transitions behave
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionAnimationConfig {
    /// Use animated transitions (vs. instant jumps)
    pub enable_transitions: bool,
    /// Easing curve for panning (e.g., ease-in-out, cubic, linear)
    pub pan_easing: EasingFunction,
    /// Easing curve for zooming
    pub zoom_easing: EasingFunction,
    /// Maximum zoom delta per frame (controls smooth zoom ramp)
    pub max_zoom_step_per_frame: f32,
    /// Maximum zoom difference that will trigger animation (like Leaflet's zoomAnimationThreshold)
    pub zoom_animation_threshold: f32,
    /// Duration in milliseconds for zoom animations
    pub zoom_duration_ms: u64,
    /// Duration in milliseconds for pan animations
    pub pan_duration_ms: u64,
    /// Whether to animate zoom around the point where user clicked/scrolled
    pub zoom_to_cursor: bool,
    /// Whether to use transform-based animations (faster than repositioning)
    pub use_transform_animations: bool,
    /// Whether to enable smooth wheel zoom (continuous vs step-based)
    pub smooth_wheel_zoom: bool,
}

impl InteractionAnimationConfig {
    /// Get the default animation duration for pan transitions
    pub fn default_pan_duration_ms(&self) -> u64 {
        if self.enable_transitions {
            match self.pan_easing {
                EasingFunction::Linear => 200,
                EasingFunction::EaseInOutBack | EasingFunction::EaseInOutExpo => 400,
                _ => 300,
            }
        } else {
            0
        }
    }

    /// Get the default animation duration for zoom transitions
    pub fn default_zoom_duration_ms(&self) -> u64 {
        if self.enable_transitions {
            match self.zoom_easing {
                EasingFunction::Linear => 150,
                EasingFunction::EaseInOutBack | EasingFunction::EaseInOutExpo => 350,
                _ => 250,
            }
        } else {
            0
        }
    }
}

impl Default for InteractionAnimationConfig {
    fn default() -> Self {
        Self {
            enable_transitions: true,
            pan_easing: EasingFunction::EaseOutCubic,
            zoom_easing: EasingFunction::EaseOutCubic,
            max_zoom_step_per_frame: 0.15,
            zoom_animation_threshold: 0.05,
            zoom_duration_ms: 350,
            pan_duration_ms: 300,
            zoom_to_cursor: true,
            use_transform_animations: true,
            smooth_wheel_zoom: true,
        }
    }
}

/// Controls rendering quality, GPU-side smoothing, and anti-aliasing
#[derive(Debug, Clone, PartialEq)]
pub struct GpuRenderingConfig {
    /// Level of multisampling (0 = none, 4 = good, 8 = high)
    pub msaa_samples: u32,
    /// Texture filtering algorithm
    pub texture_filter: TextureFilterMode,
    /// Whether to enable label/vector anti-aliasing
    pub enable_vector_smoothing: bool,
    /// Limit glyph atlas size (affects label rendering)
    pub glyph_atlas_max_bytes: usize,
}

impl GpuRenderingConfig {
    /// Check if MSAA is enabled
    pub fn is_msaa_enabled(&self) -> bool {
        self.msaa_samples > 0
    }

    /// Get the sample count for WGPU (must be power of 2)
    pub fn wgpu_sample_count(&self) -> u32 {
        if self.msaa_samples == 0 {
            1
        } else {
            // Ensure it's a valid power of 2
            self.msaa_samples.next_power_of_two().min(8)
        }
    }

    /// Get estimated VRAM usage for atlases and buffers
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

/// Texture filtering algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilterMode {
    /// Nearest neighbor (pixelated, fastest)
    Nearest,
    /// Linear interpolation (smooth, balanced)
    Linear,
    /// Anisotropic filtering with specified sample count
    Anisotropic(u8),
}

impl TextureFilterMode {
    /// Convert to wgpu filter mode
    #[cfg(feature = "render")]
    pub fn to_wgpu_filter(&self) -> (wgpu::FilterMode, wgpu::FilterMode) {
        match self {
            Self::Nearest => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
            Self::Linear | Self::Anisotropic(_) => {
                (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear)
            }
        }
    }

    /// Get anisotropy level (1 = disabled, higher = more samples)
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
        assert!(balanced.animation.enable_transitions);

        // Low quality should prioritize performance
        assert_eq!(low_quality.framerate.target_fps, Some(30));
        assert!(low_quality.tile_loader.cache_size < balanced.tile_loader.cache_size);
        assert!(!low_quality.animation.enable_transitions);

        // High quality should prioritize visual fidelity
        assert_eq!(high_quality.framerate.target_fps, None); // Uncapped
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

        // Should not render immediately after a render
        let now = std::time::Instant::now();
        assert!(!config.should_render(now));
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
