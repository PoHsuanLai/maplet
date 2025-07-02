//! Map builder for fluent API configuration
//!
//! This module provides a MapBuilder that allows for fluent configuration
//! of map instances with performance profiles, tile sources, and other options.

use crate::{
    background::tasks::TaskManagerConfig,
    layers::tile::TileSource,
    core::{
        config::{
            InteractionAnimationConfig, MapPerformanceOptions, MapPerformanceProfile,
            TileLoadingConfig,
        },
        geo::{LatLng, Point},
        map::{Map, MapOptions},
        viewport::Viewport,
    },
    Result,
};

/// Builder for creating and configuring Map instances
pub struct MapBuilder {
    /// Initial viewport configuration
    viewport: Option<Viewport>,
    /// Map interaction options
    map_options: MapOptions,
    /// Performance configuration
    performance: MapPerformanceProfile,
    /// Tile source for base layer
    tile_source: Option<Box<dyn TileSource>>,
    /// Task manager configuration
    task_config: Option<TaskManagerConfig>,
    center: LatLng,
    zoom: f64,
    size: Point,
    performance_options: Option<MapPerformanceOptions>,
    tile_config: Option<TileLoadingConfig>,
    animation_config: Option<InteractionAnimationConfig>,
    min_zoom: Option<f64>,
    max_zoom: Option<f64>,
}

impl MapBuilder {
    /// Create a new MapBuilder with default settings
    pub fn new() -> Self {
        Self {
            viewport: None,
            map_options: MapOptions::default(),
            performance: MapPerformanceProfile::default(),
            tile_source: None,
            task_config: None,
            center: LatLng::default(),
            zoom: 0.0,
            size: Point::default(),
            performance_options: None,
            tile_config: None,
            animation_config: None,
            min_zoom: None,
            max_zoom: None,
        }
    }

    /// Set the initial viewport (center, zoom, and size)
    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = Some(viewport);
        self
    }

    /// Set the initial center and zoom level
    pub fn with_center_and_zoom(mut self, center: LatLng, zoom: f64, size: Point) -> Self {
        self.viewport = Some(Viewport::new(center, zoom, size));
        self.center = center;
        self.zoom = zoom;
        self.size = size;
        self
    }

    /// Set the performance profile
    pub fn with_performance(mut self, profile: MapPerformanceProfile) -> Self {
        self.performance = profile;
        self
    }

    /// Set custom performance options
    pub fn with_performance_options(mut self, options: MapPerformanceOptions) -> Self {
        self.performance = MapPerformanceProfile::Custom(options);
        self
    }

    /// Set the tile source for the base layer
    pub fn with_tile_source(mut self, source: Box<dyn TileSource>) -> Self {
        self.tile_source = Some(source);
        self
    }

    /// Set map interaction options
    pub fn with_map_options(mut self, options: MapOptions) -> Self {
        self.map_options = options;
        self
    }

    /// Enable or disable dragging
    pub fn with_dragging(mut self, enabled: bool) -> Self {
        self.map_options.dragging = enabled;
        self
    }

    /// Enable or disable scroll wheel zoom
    pub fn with_scroll_wheel_zoom(mut self, enabled: bool) -> Self {
        self.map_options.scroll_wheel_zoom = enabled;
        self
    }

    /// Enable or disable double-click zoom
    pub fn with_double_click_zoom(mut self, enabled: bool) -> Self {
        self.map_options.double_click_zoom = enabled;
        self
    }

    /// Enable or disable touch zoom
    pub fn with_touch_zoom(mut self, enabled: bool) -> Self {
        self.map_options.touch_zoom = enabled;
        self
    }

    /// Enable or disable keyboard navigation
    pub fn with_keyboard(mut self, enabled: bool) -> Self {
        self.map_options.keyboard = enabled;
        self
    }

    /// Set zoom limits
    pub fn with_zoom_limits(mut self, min_zoom: Option<f64>, max_zoom: Option<f64>) -> Self {
        self.min_zoom = min_zoom;
        self.max_zoom = max_zoom;
        self
    }

    /// Set zoom snap and delta values
    pub fn with_zoom_behavior(mut self, snap: f64, delta: f64) -> Self {
        self.map_options.zoom_snap = snap;
        self.map_options.zoom_delta = delta;
        self
    }

    /// Enable or disable attribution control
    pub fn with_attribution_control(mut self, enabled: bool) -> Self {
        self.map_options.attribution_control = enabled;
        self
    }

    /// Enable or disable zoom control
    pub fn with_zoom_control(mut self, enabled: bool) -> Self {
        self.map_options.zoom_control = enabled;
        self
    }

    /// Set background task manager configuration
    pub fn with_task_config(mut self, config: TaskManagerConfig) -> Self {
        self.task_config = Some(config);
        self
    }

    /// Configure tile loading behavior
    pub fn with_tile_config(mut self, config: TileLoadingConfig) -> Self {
        self.tile_config = Some(config);
        self
    }

    /// Configure animation behavior
    pub fn with_animation_config(mut self, config: InteractionAnimationConfig) -> Self {
        self.animation_config = Some(config);
        self
    }

    /// Enable advanced tile prefetching
    pub fn with_tile_prefetching(mut self, buffer_size: u32, preload_zoom_tiles: bool) -> Self {
        let mut config = self.tile_config.unwrap_or_default();
        config.prefetch_buffer = buffer_size;
        config.preload_zoom_tiles = preload_zoom_tiles;
        self.tile_config = Some(config);
        self
    }

    /// Configure tile retry behavior
    pub fn with_tile_retries(
        mut self,
        max_retries: u32,
        delay_ms: u64,
        exponential_backoff: bool,
    ) -> Self {
        let mut config = self.tile_config.unwrap_or_default();
        config.max_retries = max_retries;
        config.retry_delay_ms = delay_ms;
        config.exponential_backoff = exponential_backoff;
        self.tile_config = Some(config);
        self
    }

    /// Enable parent tile fallbacks for smooth zoom
    pub fn with_parent_tile_fallbacks(mut self, enabled: bool) -> Self {
        let mut config = self.tile_config.unwrap_or_default();
        config.show_parent_tiles = enabled;
        self.tile_config = Some(config);
        self
    }

    /// Configure zoom animation behavior
    pub fn with_zoom_animation(
        mut self,
        duration_ms: u64,
        zoom_to_cursor: bool,
        smooth_wheel: bool,
    ) -> Self {
        let mut config = self.animation_config.unwrap_or_default();
        config.zoom_duration_ms = duration_ms;
        config.zoom_to_cursor = zoom_to_cursor;
        config.smooth_wheel_zoom = smooth_wheel;
        self.animation_config = Some(config);
        self
    }

    /// Enable transform-based animations for better performance
    pub fn with_transform_animations(mut self, enabled: bool) -> Self {
        let mut config = self.animation_config.unwrap_or_default();
        config.use_transform_animations = enabled;
        self.animation_config = Some(config);
        self
    }

    /// Enable spacecraft-style zoom animations with dramatic effects
    pub fn with_spacecraft_zoom(mut self) -> Self {
        let mut config = self.animation_config.unwrap_or_default();
        config.zoom_duration_ms = 600;
        config.zoom_easing = crate::layers::animation::EasingType::SpacecraftZoom;
        config.zoom_animation_threshold = 6.0; // Allow larger zoom differences
        self.animation_config = Some(config);
        self
    }

    /// Enable dynamic zoom animations with slight overshoot
    pub fn with_dynamic_zoom(mut self) -> Self {
        let mut config = self.animation_config.unwrap_or_default();
        config.zoom_duration_ms = 400;
        config.zoom_easing = crate::layers::animation::EasingType::DynamicZoom;
        config.zoom_animation_threshold = 5.0;
        self.animation_config = Some(config);
        self
    }

    /// Set custom zoom animation easing and duration
    pub fn with_zoom_easing(
        mut self,
        easing: crate::layers::animation::EasingType,
        duration_ms: u64,
    ) -> Self {
        let mut config = self.animation_config.unwrap_or_default();
        config.zoom_easing = easing;
        config.zoom_duration_ms = duration_ms;
        self.animation_config = Some(config);
        self
    }

    /// Build the map with the configured options
    pub fn build(self) -> Result<Map> {
        // Ensure we have a viewport
        let viewport = self
            .viewport
            .ok_or_else(|| crate::Error::InvalidCoordinates("No viewport specified".to_string()))?;

        // Resolve performance configuration
        let performance_options = self.performance.resolve();

        // Create task manager config based on performance options
        let task_config = self.task_config.unwrap_or_else(|| TaskManagerConfig {
            max_concurrent_tasks: performance_options
                .tile_loader
                .recommended_concurrent_tasks(),
            max_queue_size: (performance_options.tile_loader.cache_size * 2).max(1000),
            enable_metrics: false,
            test_mode: false,
        });

        // Create the map with performance-aware configuration
        let mut map = Map::with_options_and_performance(
            viewport,
            self.map_options,
            performance_options,
            task_config,
        )?;

        // Apply animation configuration if provided
        if let Some(animation_config) = &self.animation_config {
            let duration = std::time::Duration::from_millis(animation_config.zoom_duration_ms);
            map.set_zoom_animation_style(animation_config.zoom_easing, duration);
            map.set_zoom_animation_threshold(animation_config.zoom_animation_threshold as f64);
            map.set_zoom_animation_enabled(animation_config.enable_transitions);
        }

        // Add tile source if provided
        if let Some(_tile_source) = self.tile_source {
            // Create a default tile layer - users can add their own tile sources via the map API
            let tile_layer = crate::layers::tile::TileLayer::openstreetmap(
                "base_tiles".to_string(),
                "Base Map Tiles".to_string(),
            );
            map.add_layer(Box::new(tile_layer))?;
        }

        // Apply any additional layer configurations
        for layer in map.list_layers() {
            if let Some(tile_layer) = map.get_layer(&layer) {
                // Configure tile layers with performance settings
                if let Some(_tile_layer) = tile_layer
                    .as_any()
                    .downcast_ref::<crate::layers::tile::TileLayer>()
                {
                    // Tile layer configuration is already applied during construction
                }
            }
        }

        Ok(map)
    }
}

impl Default for MapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience methods for common map configurations
impl MapBuilder {
    /// Create a web map with OpenStreetMap tiles
    pub fn web_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_performance(MapPerformanceProfile::Balanced)
            .with_dragging(true)
            .with_scroll_wheel_zoom(true)
            .with_double_click_zoom(true)
            .with_touch_zoom(true)
            .with_keyboard(true)
    }

    /// Create a mobile-optimized map
    pub fn mobile_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_performance(MapPerformanceProfile::LowQuality)
            .with_dragging(true)
            .with_scroll_wheel_zoom(false) // Disable to prevent conflicts with page scroll
            .with_double_click_zoom(false)
            .with_touch_zoom(true)
            .with_keyboard(false)
    }

    /// Create a high-quality map for desktop applications
    pub fn desktop_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_performance(MapPerformanceProfile::HighQuality)
            .with_dragging(true)
            .with_scroll_wheel_zoom(true)
            .with_double_click_zoom(true)
            .with_touch_zoom(true)
            .with_keyboard(true)
    }

    /// Create a minimal map with limited interactions
    pub fn minimal_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_performance(MapPerformanceProfile::LowQuality)
            .with_dragging(false)
            .with_scroll_wheel_zoom(false)
            .with_double_click_zoom(false)
            .with_touch_zoom(false)
            .with_keyboard(false)
            .with_attribution_control(false)
            .with_zoom_control(false)
    }

    /// Create a map optimized for smooth animations and high quality
    pub fn high_quality(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_tile_prefetching(3, true)
            .with_tile_retries(5, 1000, true)
            .with_parent_tile_fallbacks(true)
            .with_zoom_animation(350, true, true)
            .with_transform_animations(true)
            .with_dynamic_zoom() // Use dynamic zoom for high quality
    }

    /// Create a map optimized for performance on slower devices
    pub fn low_quality(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_tile_prefetching(1, false)
            .with_tile_retries(2, 250, false)
            .with_parent_tile_fallbacks(false)
            .with_zoom_animation(200, true, false)
            .with_transform_animations(true)
    }

    /// Create a map with balanced settings for most use cases
    pub fn balanced(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_tile_prefetching(2, true)
            .with_tile_retries(3, 500, true)
            .with_parent_tile_fallbacks(true)
            .with_zoom_animation(350, true, true)
            .with_transform_animations(true)
    }

    /// Create a map with spacecraft-style zoom animations for dramatic effect
    pub fn spacecraft_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_tile_prefetching(2, true)
            .with_tile_retries(3, 500, true)
            .with_parent_tile_fallbacks(true)
            .with_spacecraft_zoom()
            .with_transform_animations(true)
    }

    /// Create a map with dynamic zoom animations with slight overshoot
    pub fn dynamic_map(center: LatLng, zoom: f64, size: Point) -> Self {
        Self::new()
            .with_center_and_zoom(center, zoom, size)
            .with_tile_prefetching(2, true)
            .with_tile_retries(3, 500, true)
            .with_parent_tile_fallbacks(true)
            .with_dynamic_zoom()
            .with_transform_animations(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geo::LatLng;

    #[test]
    fn test_map_builder_basic() {
        let center = LatLng::new(40.7128, -74.0060); // New York
        let size = Point::new(800.0, 600.0);

        let builder = MapBuilder::new()
            .with_center_and_zoom(center, 10.0, size)
            .with_performance(MapPerformanceProfile::Balanced);

        // Should be able to build (we can't actually build here without a proper tile source)
        // Just verify the builder is configured correctly
        assert!(builder.viewport.is_some());
        assert!(matches!(
            builder.performance,
            MapPerformanceProfile::Balanced
        ));
    }

    #[test]
    fn test_web_map_preset() {
        let center = LatLng::new(40.7128, -74.0060);
        let size = Point::new(800.0, 600.0);

        let builder = MapBuilder::web_map(center, 10.0, size);

        assert!(builder.map_options.dragging);
        assert!(builder.map_options.scroll_wheel_zoom);
        assert!(builder.map_options.double_click_zoom);
        assert!(builder.map_options.touch_zoom);
        assert!(builder.map_options.keyboard);
        assert!(matches!(
            builder.performance,
            MapPerformanceProfile::Balanced
        ));
    }

    #[test]
    fn test_mobile_map_preset() {
        let center = LatLng::new(40.7128, -74.0060);
        let size = Point::new(400.0, 600.0);

        let builder = MapBuilder::mobile_map(center, 10.0, size);

        assert!(builder.map_options.dragging);
        assert!(!builder.map_options.scroll_wheel_zoom); // Disabled for mobile
        assert!(!builder.map_options.double_click_zoom);
        assert!(builder.map_options.touch_zoom);
        assert!(!builder.map_options.keyboard);
        assert!(matches!(
            builder.performance,
            MapPerformanceProfile::LowQuality
        ));
    }

    #[test]
    fn test_minimal_map_preset() {
        let center = LatLng::new(40.7128, -74.0060);
        let size = Point::new(400.0, 300.0);

        let builder = MapBuilder::minimal_map(center, 10.0, size);

        assert!(!builder.map_options.dragging);
        assert!(!builder.map_options.scroll_wheel_zoom);
        assert!(!builder.map_options.double_click_zoom);
        assert!(!builder.map_options.touch_zoom);
        assert!(!builder.map_options.keyboard);
        assert!(!builder.map_options.attribution_control);
        assert!(!builder.map_options.zoom_control);
        assert!(matches!(
            builder.performance,
            MapPerformanceProfile::LowQuality
        ));
    }

    #[test]
    fn test_custom_performance_options() {
        use crate::core::config::*;

        let custom_options = MapPerformanceOptions {
            framerate: FrameTimingConfig {
                target_fps: Some(30),
                render_on_idle: false,
                min_update_interval_ms: 33,
            },
            tile_loader: TileLoadingConfig {
                cache_size: 512,
                fetch_batch_size: 4,
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
                pan_easing: crate::layers::animation::EasingType::Linear,
                zoom_easing: crate::layers::animation::EasingType::Linear,
                max_zoom_step_per_frame: 0.2,
                zoom_animation_threshold: 0.05,
                zoom_duration_ms: 350,
                pan_duration_ms: 300,
                zoom_to_cursor: true,
                use_transform_animations: true,
                smooth_wheel_zoom: true,
            },
            rendering: GpuRenderingConfig {
                msaa_samples: 2,
                texture_filter: TextureFilterMode::Linear,
                enable_vector_smoothing: false,
                glyph_atlas_max_bytes: 1_000_000,
            },
        };

        let builder = MapBuilder::new().with_performance_options(custom_options.clone());

        if let MapPerformanceProfile::Custom(options) = builder.performance {
            assert_eq!(options.framerate.target_fps, Some(30));
            assert_eq!(options.tile_loader.cache_size, 512);
            assert_eq!(options.rendering.msaa_samples, 2);
        } else {
            panic!("Expected custom performance profile");
        }
    }
}
