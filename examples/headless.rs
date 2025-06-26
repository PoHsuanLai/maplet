use maplet::{
    core::{geo::LatLng, geo::Point},
    layers::tile::TileLayer,
    Map,
};

/// Example of using maplet in headless mode without any UI
fn main() -> maplet::Result<()> {
    println!("🗺️ Maplet Headless Example");
    println!("==========================");

    // Create a map without rendering
    let center = LatLng::new(37.7749, -122.4194); // San Francisco
    let size = Point::new(1024.0, 768.0);
    let mut map = Map::new(center, 12.0, size);

    println!("✅ Map created:");
    println!("   Center: {:.4}, {:.4}", center.lat, center.lng);
    println!("   Zoom: {}", map.viewport().zoom);
    println!(
        "   Size: {}x{}",
        map.viewport().size.x,
        map.viewport().size.y
    );

    // Add a tile layer (this won't actually fetch tiles in headless mode)
    let tile_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
    map.add_layer(Box::new(tile_layer))?;
    println!("✅ Added OpenStreetMap tile layer");

    // Perform some map operations
    println!("\n🎯 Performing map operations:");

    // Set different views
    let locations = [
        ("New York", LatLng::new(40.7128, -74.0060), 11.0),
        ("London", LatLng::new(51.5074, -0.1278), 10.0),
        ("Tokyo", LatLng::new(35.6762, 139.6503), 12.0),
    ];

    for (name, location, zoom) in locations {
        map.set_view(location, zoom)?;
        println!(
            "   📍 {} - {:.4}, {:.4} at zoom {}",
            name, location.lat, location.lng, zoom
        );

        // Simulate some updates
        map.update(16.67)?; // ~60fps frame time
    }

    // Test panning
    println!("\n🚀 Testing pan operations:");
    let pan_deltas = [
        (100.0, 0.0),   // East
        (0.0, 100.0),   // South
        (-50.0, -50.0), // Northwest
    ];

    for (dx, dy) in pan_deltas {
        let old_center = map.viewport().center;
        map.pan(Point::new(dx, dy))?;
        let new_center = map.viewport().center;

        println!(
            "   Pan by ({}, {}) - Center moved from ({:.4}, {:.4}) to ({:.4}, {:.4})",
            dx, dy, old_center.lat, old_center.lng, new_center.lat, new_center.lng
        );
    }

    // Test zoom operations
    println!("\n🔍 Testing zoom operations:");
    let zoom_levels = [10.0, 15.0, 8.0, 12.0];

    for zoom in zoom_levels {
        map.zoom_to(zoom, None)?;
        println!("   Zoomed to level: {}", zoom);
    }

    // Test layer operations
    println!("\n📋 Layer information:");
    let layers = map.list_layers();
    println!("   Active layers: {:?}", layers);

    // Test event processing
    println!("\n⚡ Processing events:");
    let events = map.process_events();
    println!("   Processed {} events", events.len());

    // Final state
    println!("\n📊 Final map state:");
    let viewport = map.viewport();
    println!(
        "   Center: {:.6}, {:.6}",
        viewport.center.lat, viewport.center.lng
    );
    println!("   Zoom: {:.2}", viewport.zoom);
    println!("   Size: {:.0}x{:.0}", viewport.size.x, viewport.size.y);

    println!("\n✅ Headless example completed successfully!");
    println!("   This demonstrates that maplet can work without any UI framework.");
    println!("   Perfect for server-side map processing, testing, or CLI tools.");

    Ok(())
}
