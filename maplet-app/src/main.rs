use maplet::{
    core::{geo::LatLng, geo::Point},
    layers::tile::TileLayer,
    ui::widget::{MapWidget, MapWidgetConfig},
    Map,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Standalone map viewer application
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Set up graceful shutdown
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_clone = shutdown_flag.clone();

    // Handle Ctrl+C gracefully
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\nReceived Ctrl+C, shutting down gracefully...");
        shutdown_flag_clone.store(true, Ordering::Relaxed);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Maplet - Rust Map Viewer")
            .with_close_button(true),
        follow_system_theme: true,
        ..Default::default()
    };

    // Run the application with shutdown handling
    let app_result = eframe::run_native(
        "maplet-app",
        options,
        Box::new(move |cc| Box::new(MapletApp::new(cc, shutdown_flag))),
    );

    println!("Application shutdown complete.");
    app_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// The main application struct
struct MapletApp {
    map_widget: MapWidget,
    selected_preset: String,
    show_debug_panel: bool,
    show_layer_panel: bool,
    shutdown_flag: Arc<AtomicBool>,
}

impl MapletApp {
    fn new(_cc: &eframe::CreationContext<'_>, shutdown_flag: Arc<AtomicBool>) -> Self {
        // Create map centered on San Francisco
        let center = LatLng::new(37.7749, -122.4194);
        let zoom = 12.0;
        let size = Point::new(1200.0, 800.0);

        let mut map = Map::new(center, zoom, size);

        // Add OpenStreetMap tiles by default
        let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
        if let Err(e) = map.add_layer(Box::new(osm_layer)) {
            eprintln!("Failed to add OSM layer: {}", e);
        }

        let config = MapWidgetConfig {
            interactive: true,
            show_zoom_controls: true,
            show_attribution: true,
            min_zoom: 1.0,
            max_zoom: 18.0,
            cursor: maplet::ui::widget::MapCursor::Default,
            background_color: egui::Color32::from_rgb(230, 230, 230),
            attribution: "© OpenStreetMap contributors".to_string(),
            zoom_snap: 1.0,
            zoom_delta: 1.0,
        };

        let map_widget = MapWidget::with_config(map, config);

        Self {
            map_widget,
            selected_preset: "San Francisco".to_string(),
            show_debug_panel: true,
            show_layer_panel: true,
            shutdown_flag,
        }
    }

    fn location_presets(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Quick locations:");

            let presets = [
                ("San Francisco", LatLng::new(37.7749, -122.4194), 12.0),
                ("New York", LatLng::new(40.7128, -74.0060), 11.0),
                ("London", LatLng::new(51.5074, -0.1278), 11.0),
                ("Tokyo", LatLng::new(35.6762, 139.6503), 11.0),
                ("Sydney", LatLng::new(-33.8688, 151.2093), 11.0),
                ("Cape Town", LatLng::new(-33.9249, 18.4241), 11.0),
            ];

            for (name, center, zoom) in presets {
                if ui
                    .selectable_label(self.selected_preset == name, name)
                    .clicked()
                {
                    self.selected_preset = name.to_string();
                    let _ = self.map_widget.set_view(center, zoom);
                }
            }
        });
    }
}

impl eframe::App for MapletApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for shutdown signal
        if self.shutdown_flag.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_debug_panel, "Debug Panel");
                    ui.checkbox(&mut self.show_layer_panel, "Layer Panel");
                    ui.separator();
                    ui.menu_button("Performance", |ui| {
                        ui.label("Performance Profile:");
                        if ui.button("Balanced").clicked() {
                            // Apply balanced performance settings
                            if let Ok(mut map) = self.map_widget.map.lock() {
                                use maplet::core::config::MapPerformanceProfile;
                                map.set_performance(MapPerformanceProfile::Balanced.resolve());
                            }
                        }
                        if ui.button("High Quality").clicked() {
                            // Apply high quality settings
                            if let Ok(mut map) = self.map_widget.map.lock() {
                                use maplet::core::config::MapPerformanceProfile;
                                map.set_performance(MapPerformanceProfile::HighQuality.resolve());
                            }
                        }
                        if ui.button("Low Quality").clicked() {
                            // Apply low quality settings
                            if let Ok(mut map) = self.map_widget.map.lock() {
                                use maplet::core::config::MapPerformanceProfile;
                                map.set_performance(MapPerformanceProfile::LowQuality.resolve());
                            }
                        }
                    });
                });

                ui.separator();
                self.location_presets(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(viewport) = self.map_widget.viewport() {
                        ui.label(format!(
                            "Center: {:.4}, {:.4} | Zoom: {:.2}",
                            viewport.center.lat, viewport.center.lng, viewport.zoom
                        ));
                    }
                });
            });
        });

        // Debug panel
        if self.show_debug_panel {
            egui::SidePanel::left("debug_panel")
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Debug Info");
                    ui.separator();

                    ui.label(format!("Dragging: {}", self.map_widget.is_dragging()));
                    ui.label(format!("Has Focus: {}", self.map_widget.has_focus()));

                    ui.separator();
                    ui.heading("Performance");

                    // Add performance metrics here when available
                    ui.label("FPS: ~60");
                    ui.label("Tiles Loaded: N/A");
                    ui.label("Background Tasks: N/A");

                    ui.separator();
                    ui.heading("Configuration");

                    ui.label("Tile Configuration:");
                    ui.label("  • Prefetch Buffer: 2 tiles");
                    ui.label("  • Max Retries: 3");
                    ui.label("  • Cache Size: 1024 tiles");

                    ui.label("Animation Configuration:");
                    ui.label("  • Zoom to Cursor: Enabled");
                    ui.label("  • Smooth Wheel Zoom: Enabled");
                    ui.label("  • Transform Animations: Enabled");
                });
        }

        // Layer control panel
        if self.show_layer_panel {
            egui::SidePanel::right("layer_panel")
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Layers");
                    ui.separator();

                    ui.label("Base Layers:");
                    ui.radio_value(&mut (), (), "OpenStreetMap");

                    ui.separator();
                    ui.label("Overlay Layers:");
                    ui.checkbox(&mut false, "Traffic");
                    ui.checkbox(&mut false, "Transit");
                    ui.checkbox(&mut false, "Bicycle");

                    ui.separator();
                    ui.heading("Map Controls");

                    if ui.button("Fit World").clicked() {
                        let _ = self.map_widget.set_view(LatLng::new(0.0, 0.0), 2.0);
                    }

                    ui.separator();
                    ui.label("Quick Zoom:");
                    ui.horizontal(|ui| {
                        if ui.button("City").clicked() {
                            if let Some(viewport) = self.map_widget.viewport() {
                                let _ = self.map_widget.set_view(viewport.center, 12.0);
                            }
                        }
                        if ui.button("Country").clicked() {
                            if let Some(viewport) = self.map_widget.viewport() {
                                let _ = self.map_widget.set_view(viewport.center, 6.0);
                            }
                        }
                        if ui.button("World").clicked() {
                            if let Some(viewport) = self.map_widget.viewport() {
                                let _ = self.map_widget.set_view(viewport.center, 2.0);
                            }
                        }
                    });
                });
        }

        // Main map area
        egui::CentralPanel::default().show(ctx, |ui| {
            use maplet::ui::widget::MapWidgetExt;
            ui.map_widget(&mut self.map_widget);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        println!("Application is shutting down...");
        // Perform any cleanup here
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }
}
