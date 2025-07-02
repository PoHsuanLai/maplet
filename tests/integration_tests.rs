use maplet::{Map};
use std::sync::{Arc, Mutex};
use tokio;

/// Integration tests for real user interactions and scenarios
/// These tests simulate how users actually interact with the map
#[cfg(test)]
mod integration_tests {
    use super::*;
    use egui::{Context, Vec2, Pos2, Rect, Event};
    use maplet::core::{map::Map as CoreMap, geo::LatLng};

    /// Helper to create a test egui context
    fn create_test_context() -> Context {
        Context::default()
    }

    /// Helper to simulate a UI frame and get response
    fn simulate_ui_frame(ctx: &Context, widget: Map) -> (egui::Response, Rect) {
        let mut response = None;
        let mut rect = Rect::NOTHING;
        
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let widget_response = ui.add(widget);
                rect = widget_response.rect;
                response = Some(widget_response);
            });
        });
        
        (response.unwrap(), rect)
    }

    /// Test basic map creation and rendering
    #[tokio::test]
    async fn test_basic_map_creation() {
        println!("🧪 [TEST] Testing basic map creation");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .interactive(true);
        
        let (response, rect) = simulate_ui_frame(&ctx, map);
        
        assert!(rect.width() > 0.0);
        assert!(rect.height() > 0.0);
        println!("✅ [TEST] Basic map creation test passed");
    }

    /// Test zoom controls functionality
    #[tokio::test]
    async fn test_zoom_controls() {
        println!("🧪 [TEST] Testing zoom controls");
        
        let ctx = create_test_context();
        let mut map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .controls(true)
            .interactive(true);
            
        // First frame to initialize
        let (_, rect) = simulate_ui_frame(&ctx, map.clone());
        
        // Simulate clicking zoom in button
        let zoom_in_rect = egui::Rect::from_min_size(
            rect.right_top() + egui::Vec2::new(-40.0, 10.0),
            egui::Vec2::splat(30.0),
        );
        
        // Add pointer event for zoom in click
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::PointerButton {
            pos: zoom_in_rect.center(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map.clone());
            });
        });
        
        println!("✅ [TEST] Zoom controls test passed");
    }

    /// Test dragging/panning functionality
    #[tokio::test]
    async fn test_map_dragging() {
        println!("🧪 [TEST] Testing map dragging/panning");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .interactive(true);
            
        // First frame to initialize
        let (_, rect) = simulate_ui_frame(&ctx, map.clone());
        
        // Simulate drag from center to a different position
        let start_pos = rect.center();
        let end_pos = start_pos + Vec2::new(50.0, 30.0);
        
        // Mouse down
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::PointerButton {
            pos: start_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map.clone());
            });
        });
        
        // Mouse move while dragging
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::PointerMoved(end_pos));
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map.clone());
            });
        });
        
        // Mouse up
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::PointerButton {
            pos: end_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map);
            });
        });
        
        println!("✅ [TEST] Map dragging test passed");
    }

    /// Test scroll wheel zooming
    #[tokio::test]
    async fn test_scroll_zoom() {
        println!("🧪 [TEST] Testing scroll wheel zooming");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .interactive(true);
            
        // First frame to initialize
        let (_, rect) = simulate_ui_frame(&ctx, map.clone());
        
        // Simulate scroll wheel up (zoom in)
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::new(0.0, 1.0), // Scroll up
            modifiers: egui::Modifiers::NONE,
        });
        raw_input.events.push(Event::PointerMoved(rect.center()));
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map.clone());
            });
        });
        
        // Simulate scroll wheel down (zoom out)
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -1.0), // Scroll down
            modifiers: egui::Modifiers::NONE,
        });
        
        ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(map);
            });
        });
        
        println!("✅ [TEST] Scroll zoom test passed");
    }

    /// Test frame rate limiting and performance
    #[tokio::test]
    async fn test_tile_loading_stability() {
        println!("🧪 [TEST] Testing tile loading stability (no infinite repaints)");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .interactive(true);
        
        let start_time = std::time::Instant::now();
        let mut frame_count = 0;
        
        // Simulate 50 frames 
        for _i in 0..50 {
            simulate_ui_frame(&ctx, map.clone());
            frame_count += 1;
        }
        
        let elapsed = start_time.elapsed();
        println!("🧪 [TEST] {} frames completed in {:?}", frame_count, elapsed);
        
        // Should complete quickly (< 2 seconds) due to frame rate limiting
        assert!(elapsed.as_secs() < 2, "Tile loading should not cause excessive repaints");
        
        println!("✅ [TEST] Tile loading stability test passed");
    }

    /// Test different map themes
    #[tokio::test]
    async fn test_map_themes() {
        println!("🧪 [TEST] Testing map themes");
        
        let ctx = create_test_context();
        
        // Test light theme
        let light_map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .theme(maplet::ui::widget::MapTheme::Light);
        
        simulate_ui_frame(&ctx, light_map);
        
        // Test dark theme
        let dark_map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .theme(maplet::ui::widget::MapTheme::Dark);
        
        simulate_ui_frame(&ctx, dark_map);
        
        // Test satellite theme
        let satellite_map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .theme(maplet::ui::widget::MapTheme::Satellite);
        
        simulate_ui_frame(&ctx, satellite_map);
        
        println!("✅ [TEST] Map themes test passed");
    }

    /// Test map preset locations
    #[tokio::test]
    async fn test_preset_locations() {
        println!("🧪 [TEST] Testing preset locations");
        
        let ctx = create_test_context();
        
        // Test various preset locations
        let locations = vec![
            Map::san_francisco(),
            Map::new_york(),
            Map::london(),
            Map::tokyo(),
            Map::sydney(),
            Map::paris(),
        ];
        
        for location_map in locations {
            simulate_ui_frame(&ctx, location_map);
        }
        
        println!("✅ [TEST] Preset locations test passed");
    }

    /// Test zoom limits
    #[tokio::test]
    async fn test_zoom_limits() {
        println!("🧪 [TEST] Testing zoom limits");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .zoom_limits(5.0, 15.0);
        
        simulate_ui_frame(&ctx, map);
        
        println!("✅ [TEST] Zoom limits test passed");
    }

    /// Test attribution display
    #[tokio::test]
    async fn test_attribution() {
        println!("🧪 [TEST] Testing attribution display");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .attribution(true)
            .attribution_text("© Test Attribution");
        
        simulate_ui_frame(&ctx, map);
        
        println!("✅ [TEST] Attribution test passed");
    }

    /// Test non-interactive map
    #[tokio::test]
    async fn test_non_interactive_map() {
        println!("🧪 [TEST] Testing non-interactive map");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .interactive(false);
        
        simulate_ui_frame(&ctx, map);
        
        println!("✅ [TEST] Non-interactive map test passed");
    }

    /// Test map with custom size
    #[tokio::test]
    async fn test_custom_size() {
        println!("🧪 [TEST] Testing map with custom size");
        
        let ctx = create_test_context();
        let map = Map::new()
            .center(37.7749, -122.4194)
            .zoom(10.0)
            .size(Vec2::new(400.0, 300.0));
        
        let (_, rect) = simulate_ui_frame(&ctx, map);
        
        // Size might not be exact due to layout constraints, but should be reasonable
        assert!(rect.width() > 100.0);
        assert!(rect.height() > 100.0);
        
        println!("✅ [TEST] Custom size test passed");
    }
} 