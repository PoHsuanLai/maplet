#[cfg(test)]
mod performance_tests {
    use maplet::{
        core::{geo::LatLng, viewport::Viewport, geo::Point},
        layers::tile::TileLayer,
    };
    use std::time::Instant;

    #[tokio::test]
    async fn test_throttled_tile_updates() {
        // Test that tile updates are properly throttled
        let mut layer = TileLayer::for_testing("test".to_string(), "Test".to_string());
        let viewport = Viewport::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            Point::new(800.0, 600.0)
        );

        // Call update_tiles multiple times in quick succession
        let start = Instant::now();
        
        for _ in 0..10 {
            let _ = layer.update_tiles(&viewport);
        }
        
        let elapsed = start.elapsed();
        
        // Should complete quickly due to throttling (not process every call)
        assert!(elapsed.as_millis() < 50, "Tile updates should be throttled");
    }

    #[tokio::test]
    async fn test_conservative_repaint_logic() {
        // Test that needs_repaint is conservative
        let layer = TileLayer::for_testing("test".to_string(), "Test".to_string());
        
        // Fresh layer should not need repaint
        assert!(!layer.needs_repaint(), "Fresh layer should not need repaint");
    }

    #[tokio::test]
    async fn test_reduced_concurrent_limits() {
        // Test that tile loader has reasonable concurrency limits
        let layer = TileLayer::for_testing("test".to_string(), "Test".to_string());
        let config = layer.tile_loader().config();
        
        // Should have conservative limits to prevent excessive requests
        assert!(config.max_concurrent <= 4, "Max concurrent should be <= 4");
        assert!(config.max_retries <= 2, "Max retries should be <= 2");
        assert!(config.retry_delay.as_millis() >= 1000, "Retry delay should be >= 1000ms");
    }
} 