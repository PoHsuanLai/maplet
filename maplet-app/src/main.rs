use maplet::{Map, MapTheme};

/// Standalone map viewer application demonstrating simple maplet usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("🚀 [DEBUG] Starting Maplet app...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Maplet - Simple Map Demo")
            .with_close_button(true),
        follow_system_theme: true,
        ..Default::default()
    };

    println!("🖼️ [DEBUG] Creating eframe app...");
    
    // Run the app with proper shutdown handling
    let app_result = eframe::run_native(
        "Maplet Demo",
        options,
        Box::new(|_cc| {
            println!("🎯 [DEBUG] eframe app creation callback called");
            Box::new(MapletApp::new())
        }),
    );

    if let Err(err) = app_result {
        eprintln!("❌ [DEBUG] Application error: {:?}", err);
    }

    println!("👋 [DEBUG] Main function completed");
    Ok(())
}

struct MapletApp {
    selected_location: (f64, f64), // Just lat, lng - no complex types needed
    theme: MapTheme,
    show_controls: bool,
    show_debug_panel: bool,
    shutdown_requested: bool,
    /// Zed-inspired performance mode selection
    performance_mode: maplet::core::config::MapPerformanceProfile,
    /// Animation style selection
    animation_style: maplet::layers::animation::EasingType,
}

impl MapletApp {
    fn new() -> Self {
        println!("🎯 [DEBUG] MapletApp::new() - Creating new app instance");
        Self {
            selected_location: (37.7749, -122.4194), // San Francisco
            theme: MapTheme::Light,
            show_controls: true,
            show_debug_panel: false,
            shutdown_requested: false,
            performance_mode: maplet::core::config::MapPerformanceProfile::Balanced, 
            animation_style: maplet::layers::animation::EasingType::UltraSmooth,
        }
    }

    /// Show location preset buttons - much cleaner UI
    fn location_presets(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("🏙️ San Francisco").clicked() {
                println!("🏙️ [DEBUG] San Francisco location selected");
                self.selected_location = (37.7749, -122.4194);
            }
            if ui.button("🗽 New York").clicked() {
                println!("🗽 [DEBUG] New York location selected");
                self.selected_location = (40.7128, -74.0060);
            }
            if ui.button("🌉 London").clicked() {
                println!("🌉 [DEBUG] London location selected");
                self.selected_location = (51.5074, -0.1278);
            }
            if ui.button("🗼 Tokyo").clicked() {
                println!("🗼 [DEBUG] Tokyo location selected");
                self.selected_location = (35.6762, 139.6503);
            }
            if ui.button("🦘 Sydney").clicked() {
                println!("🦘 [DEBUG] Sydney location selected");
                self.selected_location = (-33.8688, 151.2093);
            }
        });
    }
}

impl eframe::App for MapletApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        static mut UPDATE_COUNTER: u64 = 0;
        unsafe {
            UPDATE_COUNTER += 1;
            if UPDATE_COUNTER % 60 == 0 {
                println!("🔄 [DEBUG] App update() called {} times", UPDATE_COUNTER);
            }
        }
        
        // Handle window close button properly 
        if ctx.input(|i| i.viewport().close_requested()) {
            println!("❌ [DEBUG] Window close button clicked, shutting down...");
            self.shutdown_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            std::process::exit(0);
        }
        
        // Handle graceful shutdown
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            println!("⚠️ [DEBUG] Escape key pressed, requesting close");
            self.shutdown_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            std::process::exit(0);
        }

        // Don't update UI if shutdown is requested
        if self.shutdown_requested {
            return;
        }

        // Top panel with simple controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label("🗺️ Maplet Demo");
                
                ui.separator();
                
                // Location presets
                self.location_presets(ui);
                
                ui.separator();
                
                // Theme selector
                ui.label("Theme:");
                egui::ComboBox::from_id_source("theme")
                    .selected_text(format!("{:?}", self.theme))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.theme, MapTheme::Light, "Light");
                        ui.selectable_value(&mut self.theme, MapTheme::Dark, "Dark");
                        ui.selectable_value(&mut self.theme, MapTheme::Satellite, "Satellite");
                    });
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.show_debug_panel, "Debug");
                    ui.checkbox(&mut self.show_controls, "Controls");
                    
                    // Add quit button for testing
                    if ui.button("Quit").clicked() {
                        println!("🚪 [DEBUG] Quit button clicked");
                        self.shutdown_requested = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        std::process::exit(0);
                    }
                });
            });
        });

        // 1. Performance & Debug Panel (collapsible)
        egui::SidePanel::left("control_panel")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("🎛️ Map Controls");
                
                ui.separator();
                
                ui.label("📍 Location:");
                ui.horizontal(|ui| {
                    if ui.button("🌉 San Francisco").clicked() {
                        self.selected_location = (37.7749, -122.4194);
                    }
                    if ui.button("🏰 London").clicked() {
                        self.selected_location = (51.5074, -0.1278);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("🗼 Tokyo").clicked() {
                        self.selected_location = (35.6762, 139.6503);
                    }
                    if ui.button("🗽 New York").clicked() {
                        self.selected_location = (40.7128, -74.0060);
                    }
                });
                
                ui.separator();
                
                ui.label("🎨 Theme:");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.theme, MapTheme::Light, "☀️ Light");
                    ui.selectable_value(&mut self.theme, MapTheme::Dark, "🌙 Dark");
                });
                
                ui.separator();
                
                ui.label("⚡ Performance (Zed-inspired):");
                ui.vertical(|ui| {
                    ui.selectable_value(&mut self.performance_mode, 
                        maplet::core::config::MapPerformanceProfile::LowQuality, 
                        "🔋 Battery (30fps)");
                    ui.selectable_value(&mut self.performance_mode, 
                        maplet::core::config::MapPerformanceProfile::Balanced, 
                        "⚖️ Balanced (60fps)");
                    ui.selectable_value(&mut self.performance_mode, 
                        maplet::core::config::MapPerformanceProfile::HighQuality, 
                        "🚀 High Quality (60fps)");
                });
                
                ui.separator();
                
                ui.label("🎭 Animation Style:");
                ui.vertical(|ui| {
                    ui.selectable_value(&mut self.animation_style, 
                        maplet::layers::animation::EasingType::Linear, 
                        "📐 Linear");
                    ui.selectable_value(&mut self.animation_style, 
                        maplet::layers::animation::EasingType::EaseOut, 
                        "📉 Ease Out");
                    ui.selectable_value(&mut self.animation_style, 
                        maplet::layers::animation::EasingType::Smooth, 
                        "✨ Zed Smooth");
                    ui.selectable_value(&mut self.animation_style, 
                        maplet::layers::animation::EasingType::UltraSmooth, 
                        "🌟 Ultra Smooth");
                });
                
                ui.separator();
                
                ui.checkbox(&mut self.show_controls, "🔧 Show map controls");
                ui.checkbox(&mut self.show_debug_panel, "🐛 Show debug info");
                
                if ui.button("🚪 Exit").clicked() {
                    self.shutdown_requested = true;
                }
            });

        // 2. Main map area
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_debug_panel {
                ui.horizontal(|ui| {
                    ui.label(format!("📍 Location: ({:.4}, {:.4})", 
                        self.selected_location.0, self.selected_location.1));
                    ui.label(format!("🎨 Theme: {:?}", self.theme));
                    ui.label(format!("⚡ Performance: {:?}", self.performance_mode));
                });
                ui.separator();
            }
            
            ui.add(
                Map::new()
                    .center(self.selected_location.0, self.selected_location.1)
                    .zoom(12.0)
                    .theme(self.theme)
                    .controls(self.show_controls)
                    .attribution_text("© Maplet Demo - Zed-Inspired Smoothness")
                    // TODO: Add these methods to the Map widget
                    // .performance_profile(self.performance_mode)
                    // .animation_style(self.animation_style)
            );
        });

        // Handle shutdown
        if self.shutdown_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        println!("🚪 [DEBUG] MapletApp::on_exit() called - Application shutting down gracefully");
    }
} 