//! Configuration demonstration showing how to use different performance profiles
//! and custom configurations with the MapBuilder.

use maplet::{
    animation::interpolation::EasingFunction,
    core::geo::{LatLng, Point},
    FrameTimingConfig, GpuRenderingConfig, InteractionAnimationConfig, MapBuilder,
    MapPerformanceOptions, MapPerformanceProfile, TextureFilterMode, TileLoadingConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the Tokio runtime for async operations
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        // Example coordinates (New York City)
        let center = LatLng::new(40.7128, -74.0060);
        let size = Point::new(800.0, 600.0);

        println!("🧭 MapLet Configuration Demo");
        println!("=============================\n");

        // 1. Using preset performance profiles
        demo_preset_profiles(center, size).await?;

        // 2. Using custom performance configuration
        demo_custom_configuration(center, size).await?;

        // 3. Using convenience map builders
        demo_convenience_builders(center, size).await?;

        println!("✅ All configuration examples completed successfully!");

        Ok::<(), maplet::Error>(())
    })?;

    Ok(())
}

async fn demo_preset_profiles(
    center: LatLng,
    size: Point,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 1. Preset Performance Profiles");
    println!("==================================");

    // Balanced profile - good for most applications
    let balanced_map = MapBuilder::new()
        .with_center_and_zoom(center, 10.0, size)
        .with_performance(MapPerformanceProfile::Balanced)
        .with_dragging(true)
        .with_scroll_wheel_zoom(true);

    println!("✓ Balanced Profile:");
    println!("  - Target FPS: 60");
    println!("  - Cache Size: 1024 tiles");
    println!("  - MSAA: 4x");
    println!("  - Animations: Enabled with EaseOutCubic");

    // Low quality profile - optimized for performance
    let low_quality_map = MapBuilder::new()
        .with_center_and_zoom(center, 10.0, size)
        .with_performance(MapPerformanceProfile::LowQuality)
        .with_dragging(true);

    println!("✓ Low Quality Profile:");
    println!("  - Target FPS: 30");
    println!("  - Cache Size: 256 tiles");
    println!("  - MSAA: Disabled");
    println!("  - Animations: Disabled");

    // High quality profile - maximum visual fidelity
    let high_quality_map = MapBuilder::new()
        .with_center_and_zoom(center, 10.0, size)
        .with_performance(MapPerformanceProfile::HighQuality)
        .with_dragging(true);

    println!("✓ High Quality Profile:");
    println!("  - Target FPS: Uncapped");
    println!("  - Cache Size: 4096 tiles");
    println!("  - MSAA: 8x");
    println!("  - Animations: Enabled with advanced easing");
    println!();

    Ok(())
}

async fn demo_custom_configuration(
    center: LatLng,
    size: Point,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎛️  2. Custom Performance Configuration");
    println!("======================================");

    // Create a custom performance configuration
    let custom_performance = MapPerformanceOptions {
        framerate: FrameTimingConfig {
            target_fps: Some(45),       // 45 FPS cap
            render_on_idle: true,       // Continue rendering when idle
            min_update_interval_ms: 16, // Update at ~60Hz
        },
        tile_loader: TileLoadingConfig {
            cache_size: 2048,    // 2048 tiles in cache
            fetch_batch_size: 8, // Load 8 tiles concurrently
            lazy_eviction: true, // Lazy cache eviction
        },
        animation: InteractionAnimationConfig {
            enable_transitions: true, // Enable smooth transitions
            pan_easing: EasingFunction::EaseInOutCubic,
            zoom_easing: EasingFunction::EaseOutExpo,
            max_zoom_step_per_frame: 0.1, // Smooth zoom steps
        },
        rendering: GpuRenderingConfig {
            msaa_samples: 4,                                   // 4x MSAA
            texture_filter: TextureFilterMode::Anisotropic(8), // 8x anisotropic filtering
            enable_vector_smoothing: true,                     // Enable vector smoothing
            glyph_atlas_max_bytes: 4_000_000,                  // 4MB glyph atlas
        },
    };

    let custom_map = MapBuilder::new()
        .with_center_and_zoom(center, 10.0, size)
        .with_performance_options(custom_performance.clone())
        .with_dragging(true)
        .with_scroll_wheel_zoom(true)
        .with_double_click_zoom(true);

    println!("✓ Custom Configuration:");
    println!(
        "  - Target FPS: {} (render on idle: {})",
        custom_performance.framerate.target_fps.unwrap_or(0),
        custom_performance.framerate.render_on_idle
    );
    println!(
        "  - Cache: {} tiles, {} concurrent fetches",
        custom_performance.tile_loader.cache_size, custom_performance.tile_loader.fetch_batch_size
    );
    println!(
        "  - Pan Easing: {:?}",
        custom_performance.animation.pan_easing
    );
    println!(
        "  - Zoom Easing: {:?}",
        custom_performance.animation.zoom_easing
    );
    println!(
        "  - MSAA: {}x samples",
        custom_performance.rendering.msaa_samples
    );
    println!(
        "  - Texture Filter: {:?}",
        custom_performance.rendering.texture_filter
    );
    println!(
        "  - Estimated VRAM: {:.1}MB",
        custom_performance.rendering.estimated_vram_usage_mb()
    );
    println!();

    Ok(())
}

async fn demo_convenience_builders(
    center: LatLng,
    size: Point,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 3. Convenience Map Builders");
    println!("==============================");

    // Web map - optimized for web browsers
    let web_map = MapBuilder::web_map(center, 10.0, size);
    println!("✓ Web Map: Balanced performance, all interactions enabled");

    // Mobile map - optimized for mobile devices
    let mobile_map = MapBuilder::mobile_map(center, 10.0, size);
    println!("✓ Mobile Map: Low quality, touch-optimized, scroll wheel disabled");

    // Desktop map - optimized for desktop applications
    let desktop_map = MapBuilder::desktop_map(center, 10.0, size);
    println!("✓ Desktop Map: High quality, all interactions enabled");

    // Minimal map - basic functionality only
    let minimal_map = MapBuilder::minimal_map(center, 10.0, size);
    println!("✓ Minimal Map: No interactions, minimal UI, optimized for performance");

    // Demonstrate method chaining with additional customization
    let customized_web_map = MapBuilder::web_map(center, 12.0, size)
        .with_zoom_limits(Some(5.0), Some(18.0))
        .with_zoom_behavior(0.5, 0.5) // Finer zoom control
        .with_attribution_control(false);

    println!("✓ Customized Web Map: Web defaults + custom zoom limits and behavior");
    println!();

    Ok(())
}

/// Helper function to demonstrate runtime performance configuration changes
#[allow(dead_code)]
async fn demo_runtime_configuration_changes() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 4. Runtime Configuration Changes");
    println!("===================================");

    // This function demonstrates runtime changes but is commented out
    // due to compilation complexity in the demo
    println!("✓ Runtime configuration changes would allow:");
    println!("  - Switching performance profiles on the fly");
    println!("  - Checking timing constraints (should_render/should_update)");
    println!("  - Tracking timing operations (mark_render/mark_update)");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_configuration_examples() {
        let center = LatLng::new(40.7128, -74.0060);
        let size = Point::new(400.0, 300.0);

        // Test that all convenience builders work
        let _web = MapBuilder::web_map(center, 10.0, size);
        let _mobile = MapBuilder::mobile_map(center, 10.0, size);
        let _desktop = MapBuilder::desktop_map(center, 10.0, size);
        let _minimal = MapBuilder::minimal_map(center, 10.0, size);

        // Test custom configuration
        let custom = MapPerformanceOptions {
            framerate: FrameTimingConfig {
                target_fps: Some(30),
                render_on_idle: false,
                min_update_interval_ms: 33,
            },
            tile_loader: TileLoadingConfig {
                cache_size: 512,
                fetch_batch_size: 4,
                lazy_eviction: true,
            },
            animation: InteractionAnimationConfig {
                enable_transitions: false,
                pan_easing: EasingFunction::Linear,
                zoom_easing: EasingFunction::Linear,
                max_zoom_step_per_frame: 0.5,
            },
            rendering: GpuRenderingConfig {
                msaa_samples: 0,
                texture_filter: TextureFilterMode::Nearest,
                enable_vector_smoothing: false,
                glyph_atlas_max_bytes: 1_000_000,
            },
        };

        let _custom_builder = MapBuilder::new()
            .with_center_and_zoom(center, 10.0, size)
            .with_performance_options(custom);

        // All builders should be valid
    }
}
