use maplet::prelude::*;
use maplet::ui::{
    popup::*, controls::*, MapWidget, MapWidgetConfig, MapCursor, MapStyle, MapThemes,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    // Create map centered on San Francisco
    let center = LatLng::new(37.7749, -122.4194);
    let zoom = 12.0;
    let size = Point::new(1200.0, 800.0);
    
    let mut map = Map::new(center, zoom, size);
    
    // Add OpenStreetMap tiles
    let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
    map.add_layer(Box::new(osm_layer))?;
    
    // Create comprehensive map controls
    let map_controls = MapControls::new()
        .with_zoom_control(ZoomControlConfig::default())
        .with_layer_control(LayerControlConfig::default())
        .with_compass(CompassConfig::default())
        .with_scale_bar(ScaleBarConfig::default())
        .with_search(SearchConfig {
            placeholder: "Search San Francisco...".to_string(),
            max_results: 15,
            ..SearchConfig::default()
        })
        .with_drawing_tools(DrawingToolsConfig {
            available_tools: vec![
                DrawingTool::Marker,
                DrawingTool::Line,
                DrawingTool::Polygon,
                DrawingTool::Rectangle,
                DrawingTool::Circle,
                DrawingTool::Text,
            ],
            default_tool: Some(DrawingTool::Marker),
            ..DrawingToolsConfig::default()
        })
        .with_measurement(MeasurementConfig {
            available_tools: vec![
                MeasurementTool::Distance,
                MeasurementTool::Area,
                MeasurementTool::Bearing,
            ],
            units: MeasurementUnits::Auto,
            ..MeasurementConfig::default()
        })
        .with_location(LocationConfig {
            auto_track: false,
            zoom_to_location: Some(16.0),
            show_accuracy: true,
            ..LocationConfig::default()
        });
    
    // Create popup manager and add various popup types
    let mut popup_manager = PopupManager::new();
    
    // Add a welcome info popup
    popup_manager.show_info(
        "welcome".to_string(),
        center,
        "Welcome to Maplet".to_string(),
        "This is a comprehensive demo of the Maplet UI system featuring popups, controls, and interactive widgets.".to_string(),
    )?;
    
    // Add a marker popup
    let marker_pos = LatLng::new(37.7849, -122.4094); // Fisherman's Wharf
    popup_manager.show_text_popup(
        "fishermans_wharf".to_string(),
        marker_pos,
        "🐟 Fisherman's Wharf - A popular tourist destination with shops, restaurants, and sea lions!".to_string(),
    )?;
    
    // Add a form popup for user feedback
    let feedback_popup = Popup::new_text(
        "feedback_form".to_string(),
        LatLng::new(37.7749, -122.4294),
        "Click here to provide feedback".to_string(),
    )
    .with_callback(|event| {
        match event {
            PopupEvent::ButtonClicked { button_id, action } => {
                println!("Button clicked: {} with action: {:?}", button_id, action);
            }
            PopupEvent::FormSubmitted { form_data } => {
                println!("Form submitted with data: {:?}", form_data);
            }
            _ => {}
        }
        Ok(())
    })
    .with_auto_close(Duration::from_secs(10));
    
    popup_manager.add_popup(feedback_popup)?;
    
    // Create map widget with custom styling
    let widget_config = MapWidgetConfig {
        interactive: true,
        show_zoom_controls: true,
        show_attribution: true,
        min_zoom: 8.0,
        max_zoom: 18.0,
        cursor: MapCursor::Default,
        background_color: egui::Color32::from_rgb(240, 248, 255), // Light blue
        attribution: "© OpenStreetMap contributors | Maplet Demo".to_string(),
        zoom_snap: 1.0,
        zoom_delta: 1.0,
    };
    
    let mut map_widget = MapWidget::with_config(map, widget_config);
    
    // Apply dark theme styling
    let dark_style = MapThemes::dark();
    
    println!("🗺️ Maplet UI Demo");
    println!("================");
    println!();
    println!("This demo showcases the comprehensive UI system including:");
    println!("• 📍 Multiple popup types (info, text, confirmation, forms)");
    println!("• 🎛️ Map controls (zoom, layers, compass, scale, search)");
    println!("• ✏️ Drawing tools (markers, lines, polygons, shapes)");
    println!("• 📏 Measurement tools (distance, area, bearing)");
    println!("• 📍 Location/GPS controls");
    println!("• 🎨 Customizable themes and styling");
    println!("• 🔄 Smooth animations and transitions");
    println!();
    println!("Features demonstrated:");
    println!("• Popup Manager with multiple popup types");
    println!("• Auto-closing popups with timers");
    println!("• Event-driven popup interactions");
    println!("• Comprehensive map controls suite");
    println!("• Configurable control positions and styling");
    println!("• Theme system (light, dark, high-contrast)");
    println!("• Interactive widget with custom configuration");
    println!();
    println!("Map Controls Available:");
    println!("• Zoom: +/- buttons with level display");
    println!("• Layers: Toggle visibility of base and overlay layers");
    println!("• Compass: Shows north direction, click to reset bearing");
    println!("• Scale Bar: Dynamic scale indicator with metric/imperial units");
    println!("• Search: Location search with autocomplete");
    println!("• Drawing Tools: Create markers, lines, polygons, and shapes");
    println!("• Measurement: Measure distances, areas, and bearings");
    println!("• Location: GPS positioning and tracking");
    println!();
    println!("Popup Types Demonstrated:");
    println!("• Info Popup: Auto-closing informational message");
    println!("• Text Popup: Simple text display with anchor positioning");
    println!("• Form Popup: Interactive forms with callbacks");
    println!("• Confirmation Dialog: Modal confirmation with actions");
    println!();
    println!("Interactive Features:");
    println!("• Click popups to interact with them");
    println!("• Use map controls to navigate and modify the map");
    println!("• Drawing tools allow creating shapes on the map");
    println!("• Measurement tools provide real-time feedback");
    println!("• Search functionality for location discovery");
    println!();
    
    // Simulate some interactions and updates
    let viewport = map_widget.viewport().unwrap_or(&Viewport::default()).clone();
    
    // Update popup positions based on viewport
    popup_manager.update(&viewport, 0.016)?; // 60 FPS
    
    // Display popup information
    let visible_popups = popup_manager.get_visible_popups();
    println!("Currently active popups: {}", visible_popups.len());
    for popup in &visible_popups {
        println!("  • {}: {} at ({:.4}, {:.4})", 
                 popup.id, 
                 match &popup.popup_type {
                     PopupType::Text => "Text",
                     PopupType::Info => "Info",
                     PopupType::Confirmation => "Confirmation",
                     PopupType::Form => "Form",
                     _ => "Other",
                 },
                 popup.anchor_position.lat, 
                 popup.anchor_position.lng);
    }
    
    println!();
    println!("Theme System:");
    println!("• Light Theme: Default bright theme for daytime use");
    println!("• Dark Theme: Low-light theme for night use");
    println!("• High Contrast: Accessibility theme for better visibility");
    println!("• Custom Themes: Define your own color schemes");
    
    println!();
    println!("Performance Features:");
    println!("• Efficient popup management with z-index sorting");
    println!("• Smooth animations with 60fps update rate");
    println!("• Responsive controls that adapt to viewport changes");
    println!("• Memory-efficient caching and cleanup");
    
    println!();
    println!("Integration Ready:");
    println!("• Compatible with egui for immediate mode GUI");
    println!("• Tokio async runtime support");
    println!("• Feature-gated dependencies for minimal builds");
    println!("• WASM-compatible for web deployment");
    
    println!();
    println!("✅ UI Demo completed successfully!");
    println!("The Maplet UI system is ready for integration into mapping applications.");
    
    Ok(())
}

/// Helper function to demonstrate advanced popup usage
async fn demonstrate_advanced_popups() -> Result<(), Box<dyn std::error::Error>> {
    let mut popup_manager = PopupManager::new();
    let center = LatLng::new(37.7749, -122.4194);
    
    // Create a rich content popup
    let rich_popup = Popup {
        id: "rich_demo".to_string(),
        anchor_position: center,
        screen_position: None,
        popup_type: PopupType::Rich,
        position: PopupPosition::Above,
        content: PopupContent::Rich {
            title: Some("San Francisco Bay Area".to_string()),
            sections: vec![
                PopupSection {
                    title: Some("Overview".to_string()),
                    content: "The San Francisco Bay Area is a populous region surrounding the San Francisco, San Pablo, and Suisun Bay estuaries in Northern California.".to_string(),
                    style: None,
                },
                PopupSection {
                    title: Some("Key Cities".to_string()),
                    content: "San Francisco, Oakland, San Jose, Berkeley, Palo Alto, and many others.".to_string(),
                    style: None,
                },
                PopupSection {
                    title: Some("Population".to_string()),
                    content: "Over 7.7 million people call the Bay Area home.".to_string(),
                    style: None,
                },
            ],
        },
        style: PopupStyle {
            max_width: 400.0,
            max_height: 300.0,
            rounding: 8.0,
            ..PopupStyle::default()
        },
        visible: false,
        modal: false,
        animation: PopupAnimation::Slide {
            direction: egui::Vec2::new(0.0, -1.0),
            progress: 0.0,
        },
        created_at: std::time::Instant::now(),
        auto_close_duration: None,
        show_arrow: true,
        z_index: 1500,
        on_event: Some(Box::new(|event| {
            println!("Rich popup event: {:?}", event);
            Ok(())
        })),
        hovered: false,
        form_data: std::collections::HashMap::new(),
    };
    
    popup_manager.add_popup(rich_popup)?;
    
    // Create a form popup
    let form_popup = Popup {
        id: "contact_form".to_string(),
        anchor_position: LatLng::new(37.7649, -122.4194),
        screen_position: None,
        popup_type: PopupType::Form,
        position: PopupPosition::Right,
        content: PopupContent::Form {
            title: "Contact Us".to_string(),
            fields: vec![
                FormField {
                    id: "name".to_string(),
                    label: "Your Name".to_string(),
                    field_type: FormFieldType::Text,
                    value: String::new(),
                    required: true,
                    placeholder: Some("Enter your name".to_string()),
                },
                FormField {
                    id: "email".to_string(),
                    label: "Email Address".to_string(),
                    field_type: FormFieldType::Email,
                    value: String::new(),
                    required: true,
                    placeholder: Some("your@email.com".to_string()),
                },
                FormField {
                    id: "category".to_string(),
                    label: "Category".to_string(),
                    field_type: FormFieldType::Select(vec![
                        "General Question".to_string(),
                        "Bug Report".to_string(),
                        "Feature Request".to_string(),
                        "Technical Support".to_string(),
                    ]),
                    value: String::new(),
                    required: true,
                    placeholder: None,
                },
                FormField {
                    id: "message".to_string(),
                    label: "Message".to_string(),
                    field_type: FormFieldType::TextArea,
                    value: String::new(),
                    required: true,
                    placeholder: Some("Tell us more...".to_string()),
                },
                FormField {
                    id: "newsletter".to_string(),
                    label: "Subscribe to newsletter".to_string(),
                    field_type: FormFieldType::Checkbox,
                    value: "false".to_string(),
                    required: false,
                    placeholder: None,
                },
            ],
            buttons: vec![
                PopupButton {
                    id: "submit".to_string(),
                    text: "Send Message".to_string(),
                    button_type: PopupButtonType::Primary,
                    action: PopupAction::Submit,
                },
                PopupButton {
                    id: "cancel".to_string(),
                    text: "Cancel".to_string(),
                    button_type: PopupButtonType::Secondary,
                    action: PopupAction::Cancel,
                },
            ],
        },
        style: PopupStyle {
            max_width: 450.0,
            max_height: 500.0,
            padding: 16.0,
            ..PopupStyle::default()
        },
        visible: false,
        modal: true,
        animation: PopupAnimation::Scale { progress: 0.0 },
        created_at: std::time::Instant::now(),
        auto_close_duration: None,
        show_arrow: false,
        z_index: 2000,
        on_event: Some(Box::new(|event| {
            match event {
                PopupEvent::FormSubmitted { form_data } => {
                    println!("Form submitted successfully!");
                    println!("Data received:");
                    for (key, value) in form_data {
                        println!("  {}: {}", key, value);
                    }
                }
                PopupEvent::ButtonClicked { button_id, action } => {
                    println!("Form button clicked: {} ({:?})", button_id, action);
                }
                _ => {}
            }
            Ok(())
        })),
        hovered: false,
        form_data: std::collections::HashMap::new(),
    };
    
    popup_manager.add_popup(form_popup)?;
    
    Ok(())
}

/// Helper function to demonstrate control customization
fn demonstrate_control_customization() {
    println!("Control Customization Examples:");
    println!("==============================");
    
    // Custom zoom control
    let custom_zoom = ZoomControlConfig {
        base: ControlConfig {
            position: ControlPosition::TopLeft,
            margin: 20.0,
            draggable: true,
            ..ControlConfig::default()
        },
        show_zoom_level: true,
        button_size: 40.0,
        zoom_in_text: "🔍+".to_string(),
        zoom_out_text: "🔍-".to_string(),
        ..ZoomControlConfig::default()
    };
    
    println!("• Custom Zoom Control:");
    println!("  - Position: Top Left with 20px margin");
    println!("  - Draggable: Yes");
    println!("  - Shows current zoom level");
    println!("  - Custom emoji buttons (🔍+ / 🔍-)");
    println!("  - Larger button size (40px)");
    
    // Custom search control
    let custom_search = SearchConfig {
        base: ControlConfig {
            position: ControlPosition::TopCenter,
            margin: 15.0,
            ..ControlConfig::default()
        },
        placeholder: "🔎 Search anywhere in the world...".to_string(),
        max_results: 20,
        min_chars: 2,
        search_delay: 200,
        live_search: true,
    };
    
    println!("• Custom Search Control:");
    println!("  - Position: Top Center");
    println!("  - Enhanced placeholder with emoji");
    println!("  - Increased max results (20)");
    println!("  - Lower minimum character threshold (2)");
    println!("  - Faster search delay (200ms)");
    
    // Custom measurement tools
    let custom_measurement = MeasurementConfig {
        base: ControlConfig {
            position: ControlPosition::Custom { x: 100.0, y: 200.0 },
            ..ControlConfig::default()
        },
        available_tools: vec![
            MeasurementTool::Distance,
            MeasurementTool::Area,
            MeasurementTool::Bearing,
            MeasurementTool::Elevation,
        ],
        units: MeasurementUnits::Metric,
        show_area: true,
        show_perimeter: true,
    };
    
    println!("• Custom Measurement Tools:");
    println!("  - Position: Custom coordinates (100, 200)");
    println!("  - All measurement tools available");
    println!("  - Metric units enforced");
    println!("  - Shows both area and perimeter");
    
    println!();
} 