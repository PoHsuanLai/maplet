use eframe::egui;
use map_rs::{
    core::{geo::LatLng, geo::Point, map::Map},
    layers::tile::TileLayer,
    ui::widget::{MapWidget, MapWidgetConfig, MapWidgetExt},
};

/// Example application using the map widget
struct MapApp {
    map_widget: MapWidget,
    show_controls: bool,
    show_attribution: bool,
    zoom_level: f64,
    center_lat: f64,
    center_lng: f64,
}

impl MapApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Create a map centered on New York City
        let center = LatLng::new(40.7128, -74.0060);
        let zoom = 10.0;
        let size = Point::new(800.0, 600.0);

        let mut map = Map::new(center, zoom, size);

        // Add OpenStreetMap tiles
        let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
        if let Err(e) = map.add_layer(Box::new(osm_layer)) {
            eprintln!("Failed to add OSM layer: {}", e);
        }

        // Create map widget with custom configuration
        let config = MapWidgetConfig {
            interactive: true,
            show_zoom_controls: true,
            show_attribution: true,
            min_zoom: 1.0,
            max_zoom: 18.0,
            cursor: map_rs::ui::widget::MapCursor::Default,
            background_color: egui::Color32::from_rgb(230, 230, 230),
            attribution: "© OpenStreetMap contributors".to_string(),
            zoom_snap: 1.0,
            zoom_delta: 1.0,
        };

        let map_widget = MapWidget::with_config(map, config)
            .on_click(|lat_lng| {
                println!("Map clicked at: {}, {}", lat_lng.lat, lat_lng.lng);
            })
            .on_double_click(|lat_lng| {
                println!("Map double-clicked at: {}, {}", lat_lng.lat, lat_lng.lng);
            })
            .on_zoom_changed(|zoom| {
                println!("Zoom changed to: {}", zoom);
            })
            .on_center_changed(|center| {
                println!("Center changed to: {}, {}", center.lat, center.lng);
            });

        Self {
            map_widget,
            show_controls: true,
            show_attribution: true,
            zoom_level: zoom,
            center_lat: center.lat,
            center_lng: center.lng,
        }
    }
}

impl eframe::App for MapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update current values from map
        if let Some(viewport) = self.map_widget.viewport() {
            self.zoom_level = viewport.zoom;
            self.center_lat = viewport.center.lat;
            self.center_lng = viewport.center.lng;
        }

        // Top panel with controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("Rust Leaflet Map Example");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Center:");
                ui.label(format!("{:.4}, {:.4}", self.center_lat, self.center_lng));

                ui.separator();

                ui.label("Zoom:");
                ui.label(format!("{:.2}", self.zoom_level));

                ui.separator();

                if ui.button("NYC").clicked() {
                    let _ = self
                        .map_widget
                        .set_view(LatLng::new(40.7128, -74.0060), 10.0);
                }

                if ui.button("London").clicked() {
                    let _ = self
                        .map_widget
                        .set_view(LatLng::new(51.5074, -0.1278), 10.0);
                }

                if ui.button("Tokyo").clicked() {
                    let _ = self
                        .map_widget
                        .set_view(LatLng::new(35.6762, 139.6503), 10.0);
                }
            });
        });

        // Left panel with settings
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Map Settings");
            ui.separator();

            ui.checkbox(&mut self.show_controls, "Show Zoom Controls");
            ui.checkbox(&mut self.show_attribution, "Show Attribution");

            // Apply settings to map widget
            self.map_widget.config.show_zoom_controls = self.show_controls;
            self.map_widget.config.show_attribution = self.show_attribution;

            ui.separator();

            ui.heading("Manual Controls");

            ui.horizontal(|ui| {
                if ui.button("Zoom In").clicked() {
                    let _ = self.map_widget.zoom_in(None);
                }
                if ui.button("Zoom Out").clicked() {
                    let _ = self.map_widget.zoom_out(None);
                }
            });

            ui.separator();

            // Manual center input
            ui.heading("Set Center");
            let mut lat = self.center_lat;
            let mut lng = self.center_lng;

            ui.horizontal(|ui| {
                ui.label("Lat:");
                if ui
                    .add(egui::DragValue::new(&mut lat).speed(0.001))
                    .changed()
                {
                    let _ = self.map_widget.set_center(LatLng::new(lat, lng));
                }
            });

            ui.horizontal(|ui| {
                ui.label("Lng:");
                if ui
                    .add(egui::DragValue::new(&mut lng).speed(0.001))
                    .changed()
                {
                    let _ = self.map_widget.set_center(LatLng::new(lat, lng));
                }
            });

            ui.separator();

            // Manual zoom input
            ui.heading("Set Zoom");
            let mut zoom = self.zoom_level;
            ui.horizontal(|ui| {
                ui.label("Zoom:");
                if ui.add(egui::Slider::new(&mut zoom, 1.0..=18.0)).changed() {
                    let _ = self.map_widget.set_zoom(zoom);
                }
            });

            ui.separator();

            ui.heading("Info");
            ui.label(format!("Dragging: {}", self.map_widget.is_dragging()));
            ui.label(format!("Has Focus: {}", self.map_widget.has_focus()));
        });

        // Main map area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Show the map widget using the extension trait
            ui.map_widget(&mut self.map_widget);
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Initialize logging

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Leaflet Map",
        options,
        Box::new(|cc| Box::new(MapApp::new(cc))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        // Test that we can create the app without panicking
        // Note: This doesn't test the full eframe integration
        let center = LatLng::new(40.7128, -74.0060);
        let map = Map::new(center, 10.0, Point::new(800.0, 600.0));
        let widget = MapWidget::new(map);

        assert_eq!(widget.size, egui::Vec2::new(800.0, 600.0));
    }
}
