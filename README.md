# Maplet

A modular, GPU-accelerated, async-aware Rust map engine that can be embedded in any application or run as a standalone map viewer. **Inspired by Zed's 120fps rendering techniques** for the smoothest map experience possible.

## ✨ Simple API (New!)

Following [egui's design philosophy](https://github.com/emilk/egui), Maplet now provides a **dead simple** immediate-mode API that just works:

```rust
use maplet::prelude::*;

// Just works - shows San Francisco by default!
ui.add(Map::new());

// Or use helper functions
ui.map(); // Default location
ui.map_at(51.5074, -0.1278); // London
ui.map_at_zoom(40.7128, -74.0060, 10.0); // New York with zoom

// Customize with builder pattern
ui.add(
    Map::new()
        .center(37.7749, -122.4194)
        .zoom(12)
        .size([800.0, 600.0])
        .theme(MapTheme::Dark)
        .controls(true)
        .attribution_text("© My Data")
);

// Or use presets
ui.add(Map::san_francisco());
ui.add(Map::london());
ui.add(Map::tokyo());
ui.add(Map::paris());
```

## 🔥 Before vs After

### ❌ Old Way (Complex)
```rust
// 50+ lines of boilerplate just to show a map!
let center = LatLng::new(37.7749, -122.4194);
let zoom = 12.0;
let size = Point::new(900.0, 700.0);

let mut map = Map::new(center, zoom, size);

// Manual layer creation
let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
if let Err(e) = map.add_layer(Box::new(osm_layer)) {
    eprintln!("Failed to add OSM layer: {}", e);
}

// Complex configuration
let config = MapWidgetConfig {
    interactive: true,
    show_zoom_controls: true,
    show_attribution: true,
    background_color: egui::Color32::from_rgb(230, 230, 230),
    attribution: "© OpenStreetMap contributors".to_string(),
    zoom_sensitivity: 0.15,
    min_zoom: 0.0,
    max_zoom: 18.0,
    zoom_delta: 1.0,
    preferred_size: Some(egui::Vec2::new(900.0, 700.0)),
    smooth_panning: true,
};

let map_widget = MapWidget::with_config(map, config);

// More complexity with mutex management
if let Ok(mut map) = map_widget.map().lock() {
    let _ = map.set_view(center, zoom);
}
```

### ✅ New Way (Simple)
```rust
// Just one line!
ui.add(Map::san_francisco());

// Or with customization
ui.add(Map::new().center(37.7749, -122.4194).zoom(12).theme(MapTheme::Dark));
```

## 🚀 Zed-Inspired Performance

Maplet is built with the same performance principles as the [Zed editor](https://zed.dev), featuring:

- **120 FPS targeting** with ProMotion-style adaptive refresh rates
- **Transform-based animations** for GPU-accelerated smoothness
- **Triple buffering** techniques to avoid frame drops
- **Immediate-mode rendering** with intelligent frame timing
- **Ultra-smooth easing** functions designed for high refresh rate displays

```rust
// Ultra-smooth animations like Zed
ui.add(
    Map::new()
        .performance_profile(MapPerformanceProfile::HighQuality) // 120fps targeting
        .animation_style(EasingType::ZedSmooth) // Zed-inspired easing
);
```

## 🚀 Features

- **Dead Simple**: Works immediately with sensible defaults
- **120 FPS Smooth**: Zed-inspired rendering for ultra-smooth interactions
- **Immediate Mode**: No state management, just call the function
- **Smart Defaults**: Automatically adds tile layers, enables interactions
- **Builder Pattern**: Chainable customization for advanced users
- **Preset Locations**: Quick shortcuts for common cities
- **Multiple Themes**: Light, Dark, Satellite modes
- **GPU-Accelerated**: High-performance rendering with wgpu (always included)
- **Cross-Platform**: Desktop, web (WASM), and mobile
- **Async-Aware**: Non-blocking tile loading
- **ProMotion Support**: Adapts to high refresh rate displays

## 📦 Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
maplet = { version = "0.1", features = ["egui"] }
eframe = "0.26"  # or your preferred egui integration
```

Minimal example:

```rust
use maplet::prelude::*;

fn main() -> Result<(), eframe::Error> {
    eframe::run_simple_native("My Map App", Default::default(), move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.map(); // That's it!
        });
    })
}
```

## 🎨 Themes & Customization

```rust
// Light theme (default)
ui.add(Map::london());

// Dark theme
ui.add(Map::london().theme(MapTheme::Dark));

// Satellite
ui.add(Map::london().theme(MapTheme::Satellite));

// Custom styling
ui.add(
    Map::new()
        .center(48.8566, 2.3522) // Paris
        .zoom(10)
        .size([600.0, 400.0])
        .controls(false) // Hide zoom controls
        .attribution(false) // Hide attribution
        .interactive(false) // Static map
);
```

## 🌍 Preset Locations

For convenience, we provide preset locations:

```rust
ui.add(Map::san_francisco()); // 37.7749, -122.4194, zoom 12
ui.add(Map::new_york());      // 40.7128, -74.0060, zoom 11
ui.add(Map::london());        // 51.5074, -0.1278, zoom 10
ui.add(Map::tokyo());         // 35.6762, 139.6503, zoom 11
ui.add(Map::sydney());        // -33.8688, 151.2093, zoom 11
ui.add(Map::paris());         // 48.8566, 2.3522, zoom 11
```

## 🔧 Advanced Usage

For advanced users, the full API is still available:

```rust
use maplet::{MapBuilder, CoreMap, TileLayer};

// Use the builder for complex setups
let map = MapBuilder::new()
    .web_map(center, zoom, size)
    .with_tile_source(Box::new(custom_tile_source))
    .with_performance(MapPerformanceProfile::HighQuality)
    .build()?;

// Or create maps manually
let mut core_map = CoreMap::new(center, zoom, size);
core_map.add_layer(Box::new(TileLayer::custom(...)))?;
```

## 🎯 Design Philosophy

Inspired by [egui's approach](https://github.com/emilk/egui):

- **Immediate Mode**: No complex state management
- **Smart Defaults**: Works out of the box
- **Simple First**: Helper functions for common cases
- **Builder Pattern**: Only when you need customization
- **No Boilerplate**: Minimal code to get started

## 📄 License

MIT OR Apache-2.0 