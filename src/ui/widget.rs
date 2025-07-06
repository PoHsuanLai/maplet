use crate::prelude::{Arc, Mutex};
use crate::{
    core::{
        geo::{LatLng, Point},
        map::Map as CoreMap,
    },
    layers::tile::TileLayer,
    rendering::context::{DrawCommand, RenderContext},
};
use egui::{Color32, ColorImage, Rect, Response, Sense, Ui, Vec2, Widget};

#[derive(Clone)]
pub struct Map {
    pub center: LatLng,
    pub zoom: f64,
    pub size: Option<Vec2>,
    pub interactive: bool,
    pub show_controls: bool,
    pub show_attribution: bool,
    pub attribution: String,
    pub theme: MapTheme,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub map_id: Option<egui::Id>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapTheme {
    Light,
    Dark,
    Satellite,
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    pub fn new() -> Self {
        Self {
            center: LatLng::new(37.7749, -122.4194), // San Francisco
            zoom: 10.0,
            size: None,
            interactive: true,
            show_controls: true,
            show_attribution: true,
            attribution: "© OpenStreetMap contributors".to_string(),
            theme: MapTheme::Light,
            min_zoom: 0.0,
            max_zoom: 18.0,
            map_id: None,
        }
    }

    pub fn center(mut self, lat: f64, lng: f64) -> Self {
        self.center = LatLng::new(lat, lng);
        self
    }

    pub fn zoom(mut self, zoom: f64) -> Self {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn controls(mut self, show: bool) -> Self {
        self.show_controls = show;
        self
    }

    pub fn attribution(mut self, show: bool) -> Self {
        self.show_attribution = show;
        self
    }

    pub fn attribution_text(mut self, text: impl Into<String>) -> Self {
        self.attribution = text.into();
        self
    }

    pub fn theme(mut self, theme: MapTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn zoom_limits(mut self, min: f64, max: f64) -> Self {
        self.min_zoom = min;
        self.max_zoom = max;
        self.zoom = self.zoom.clamp(min, max);
        self
    }

    pub fn id(mut self, id: impl Into<egui::Id>) -> Self {
        self.map_id = Some(id.into());
        self
    }

    pub fn san_francisco() -> Self {
        Self::new().center(37.7749, -122.4194).zoom(12.0)
    }

    pub fn new_york() -> Self {
        Self::new().center(40.7128, -74.0060).zoom(10.0)
    }

    pub fn london() -> Self {
        Self::new().center(51.5074, -0.1278).zoom(10.0)
    }

    pub fn tokyo() -> Self {
        Self::new().center(35.6762, 139.6503).zoom(11.0)
    }

    pub fn sydney() -> Self {
        Self::new().center(-33.8688, 151.2093).zoom(11.0)
    }

    pub fn paris() -> Self {
        Self::new().center(48.8566, 2.3522).zoom(11.0)
    }
}

impl Widget for Map {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = self.size.unwrap_or_else(|| ui.available_size());
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        let map_id = get_map_id(&self);
        let core_map = get_or_create_core_map(ui.ctx(), &self, rect, map_id);

        if self.interactive {
            handle_map_input(ui, &mut response, &core_map, &self, rect);
        }

        render_map(ui, rect, &core_map);

        if self.show_controls {
            render_zoom_controls(ui, rect, &core_map, &self, &mut response);
        }

        if self.show_attribution && !self.attribution.is_empty() {
            render_attribution(ui, rect, &self.attribution);
        }

        response
    }
}

fn get_map_id(map: &Map) -> egui::Id {
    map.map_id.unwrap_or_else(|| {
        egui::Id::new("maplet_core_map").with((
            (map.center.lat * 1000.0) as i32,
            (map.center.lng * 1000.0) as i32,
            (map.zoom * 10.0) as i32,
        ))
    })
}

fn get_or_create_core_map(
    ctx: &egui::Context,
    map: &Map,
    rect: Rect,
    map_id: egui::Id,
) -> Arc<Mutex<CoreMap>> {
    // Try to get existing map first
    if let Some(core_map) = ctx.memory(|mem| mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)) {
        // Update size and center if needed
        if let Ok(mut core_map_guard) = core_map.try_lock() {
            let current_size = core_map_guard.viewport().size;
            let current_zoom = core_map_guard.viewport().zoom;

            let new_size = Point::new(rect.width() as f64, rect.height() as f64);

            // Handle center synchronization carefully:
            // - Set center on first creation (when sizes are very different)
            // - Allow programmatic center changes by checking if widget center changed from last time
            // - But preserve user drag-based center changes
            let is_initial_setup = (current_size.x - new_size.x).abs() > 100.0
                || (current_size.y - new_size.y).abs() > 100.0;

            // Track the widget's previous center to detect when it changes programmatically
            let widget_center_key = map_id.with("widget_center");
            let previous_widget_center =
                ctx.memory(|mem| mem.data.get_temp::<LatLng>(widget_center_key));

            let widget_center_changed = if let Some(prev_center) = previous_widget_center {
                (prev_center.lat - map.center.lat).abs() > 0.01
                    || (prev_center.lng - map.center.lng).abs() > 0.01
            } else {
                true // First time, so consider it changed
            };

            if is_initial_setup || widget_center_changed {
                // Only update center if this looks like initial setup or the widget's center changed programmatically
                let _ = core_map_guard.set_view(map.center, current_zoom);
                // Store the new widget center for next time
                ctx.memory_mut(|mem| mem.data.insert_temp(widget_center_key, map.center));
            }

            // Only update size if there's a significant change
            if (current_size.x - new_size.x).abs() > 2.0
                || (current_size.y - new_size.y).abs() > 2.0
            {
                core_map_guard.viewport_mut().set_size(new_size);
            }

            drop(core_map_guard);
        }
        return core_map;
    }

    // Create new map
    let size = Point::new(rect.width() as f64, rect.height() as f64);
    let mut new_map = if std::thread::current().name().unwrap_or("").contains("test") || cfg!(test)
    {
        CoreMap::for_testing(map.center, map.zoom, size)
    } else {
        CoreMap::new(map.center, map.zoom, size)
    };

    // Add default tile layer so the map has something to render
    let is_test = std::thread::current().name().unwrap_or("").contains("test") || cfg!(test);
    let tile_layer = if is_test {
        TileLayer::for_testing("default_tiles".to_string(), "OpenStreetMap".to_string())
    } else {
        TileLayer::openstreetmap("default_tiles".to_string(), "OpenStreetMap".to_string())
    };

    let _ = new_map.add_layer(Box::new(tile_layer));

    let core_map = Arc::new(Mutex::new(new_map));

    // Store in memory
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(map_id, core_map.clone());
    });

    core_map
}

fn handle_map_input(
    ui: &mut Ui,
    response: &mut Response,
    core_map: &Arc<Mutex<CoreMap>>,
    _map: &Map,
    rect: Rect,
) {
    if let Ok(mut map_guard) = core_map.try_lock() {
        let mut all_events = Vec::new();

        // Get events from response (clicks, drags, etc.)
        let response_events = crate::input::events::EventConversion::from_egui_response(response);
        all_events.extend(response_events);

        // Get events from input state (scroll wheel, etc.)
        let input_events =
            crate::input::events::EventConversion::from_egui_input_state(ui.ctx(), rect);
        all_events.extend(input_events);

        // Process all events through unified handler
        if !all_events.is_empty() {
            for event in all_events {
                // Use the unified handle_input method
                if let Err(e) = map_guard.handle_input(event) {
                    eprintln!("Error handling input event: {}", e);
                }
            }
            response.mark_changed();
        }
    }
}

fn render_map(ui: &mut Ui, rect: Rect, core_map: &Arc<Mutex<CoreMap>>) {
    // Use a more robust locking mechanism to prevent rendering conflicts
    match core_map.try_lock() {
        Ok(mut map_guard) => {
            let width = rect.width().max(1.0) as u32;
            let height = rect.height().max(1.0) as u32;

            // Get the current viewport transform for animations
            let viewport_transform = *map_guard.viewport().get_transform();
            let has_active_transform = map_guard.viewport().has_active_transform();

            // Check if we're currently dragging (like Leaflet's continuous repaint during drag)
            let is_dragging = map_guard.viewport().is_dragging();

            // Request continuous repaints during animations AND dragging (like Leaflet)
            // More frequent repaints during drag for smoother tile loading
            if has_active_transform || is_dragging {
                ui.ctx().request_repaint();
                // During drag, request immediate repaint for smoother experience
                if is_dragging {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(16));
                    // ~60fps
                }
            }

            // Always try to render - the orchestrator was too restrictive
            if let Ok(mut render_ctx) = RenderContext::new(width, height) {
                // Perform the update and render
                match map_guard.update_and_render(&mut render_ctx) {
                    Ok(rendered) => {
                        if rendered {
                            let drawing_queue = render_ctx.get_drawing_queue();

                            // Process drawing commands with error handling
                            // Apply transforms during zoom animations (like Leaflet)
                            for cmd in drawing_queue.iter() {
                                match cmd {
                                    DrawCommand::Tile { data, bounds, .. } => {
                                        if has_active_transform {
                                            render_tile_with_transform(
                                                ui,
                                                rect,
                                                data,
                                                bounds,
                                                &viewport_transform,
                                            );
                                        } else {
                                            // CRITICAL FIX: Don't apply drag transform since tiles are already positioned correctly
                                            // The tile layer now calculates positions using effective center during drag
                                            render_tile(ui, rect, data, bounds);
                                        }
                                    }
                                    DrawCommand::TileTextured {
                                        texture_id, bounds, ..
                                    } => {
                                        if has_active_transform {
                                            render_textured_tile_with_transform(
                                                ui,
                                                rect,
                                                *texture_id,
                                                bounds,
                                                &viewport_transform,
                                            );
                                        } else {
                                            // CRITICAL FIX: Don't apply drag transform since tiles are already positioned correctly
                                            // The tile layer now calculates positions using effective center during drag
                                            render_textured_tile(ui, rect, *texture_id, bounds);
                                        }
                                    }
                                    _ => {
                                        // Handle other drawing commands if needed
                                    }
                                }
                            }
                        } else {
                            // If no rendering occurred, show a simple background
                            ui.painter()
                                .rect_filled(rect, 0.0, Color32::from_rgb(240, 240, 240));
                        }
                    }
                    Err(e) => {
                        // Log error but don't crash - show fallback
                        eprintln!("Map render error: {}", e);
                        render_fallback_map(ui, rect, "Render error");
                    }
                }
            } else {
                render_fallback_map(ui, rect, "Context creation failed");
            }
        }
        Err(_) => {
            // If we can't lock the map, show a loading state instead of crashing
            render_fallback_map(ui, rect, "Loading map...");
        }
    }
}

fn render_fallback_map(ui: &mut Ui, rect: Rect, message: &str) {
    ui.painter()
        .rect_filled(rect, 0.0, Color32::from_rgb(230, 230, 230));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        message,
        egui::FontId::proportional(16.0),
        Color32::from_gray(100),
    );
}

fn render_tile(ui: &mut Ui, rect: Rect, data: &[u8], bounds: &(Point, Point)) {
    if data.is_empty() {
        // Render a placeholder for empty tiles
        let (min_point, max_point) = *bounds;
        let tile_rect = Rect::from_two_pos(
            egui::Pos2::new(
                rect.min.x + min_point.x as f32,
                rect.min.y + min_point.y as f32,
            ),
            egui::Pos2::new(
                rect.min.x + max_point.x as f32,
                rect.min.y + max_point.y as f32,
            ),
        );
        ui.painter()
            .rect_filled(tile_rect, 0.0, Color32::from_rgb(200, 200, 200));
        return;
    }

    // Create a simple but stable texture key
    let texture_key = format!("tile_{}_{}", (bounds.0.x as i32), (bounds.0.y as i32));

    // Try to load the image
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba_img = img.to_rgba8();
            let (width, height) = rgba_img.dimensions();

            // Only proceed if we have valid dimensions
            if width > 0 && height > 0 {
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &rgba_img.into_raw(),
                );

                let texture =
                    ui.ctx()
                        .load_texture(texture_key, color_image, egui::TextureOptions::LINEAR);

                let texture_id = texture.id();

                // Render the tile using the texture
                let (min_point, max_point) = *bounds;
                let tile_rect = Rect::from_two_pos(
                    egui::Pos2::new(
                        rect.min.x + min_point.x as f32,
                        rect.min.y + min_point.y as f32,
                    ),
                    egui::Pos2::new(
                        rect.min.x + max_point.x as f32,
                        rect.min.y + max_point.y as f32,
                    ),
                );

                // Render the tile
                ui.painter().image(
                    texture_id,
                    tile_rect,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
                    Color32::WHITE,
                );
            } else {
                println!("❌ [RENDER] Invalid tile dimensions: {}x{}", width, height);
                render_error_tile(ui, rect, bounds, "Invalid dimensions");
            }
        }
        Err(e) => {
            println!(
                "❌ [RENDER] Failed to load tile image: {} ({} bytes), error: {}",
                texture_key,
                data.len(),
                e
            );

            // Debug: Show first few bytes of data
            if !data.is_empty() {
                let preview = data
                    .iter()
                    .take(16)
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("📋 [DEBUG] Tile data preview: {} (first 16 bytes)", preview);
            }

            render_error_tile(ui, rect, bounds, "Image decode error");
        }
    }
}

fn render_error_tile(ui: &mut Ui, rect: Rect, bounds: &(Point, Point), error_msg: &str) {
    let (min_point, max_point) = *bounds;
    let tile_rect = Rect::from_two_pos(
        egui::Pos2::new(
            rect.min.x + min_point.x as f32,
            rect.min.y + min_point.y as f32,
        ),
        egui::Pos2::new(
            rect.min.x + max_point.x as f32,
            rect.min.y + max_point.y as f32,
        ),
    );

    // Render a distinctive error tile
    ui.painter()
        .rect_filled(tile_rect, 0.0, Color32::from_rgb(255, 200, 200));
    ui.painter()
        .rect_stroke(tile_rect, 0.0, egui::Stroke::new(1.0, Color32::RED));

    // Add error text if tile is large enough
    if tile_rect.width() > 50.0 && tile_rect.height() > 50.0 {
        ui.painter().text(
            tile_rect.center(),
            egui::Align2::CENTER_CENTER,
            error_msg,
            egui::FontId::monospace(8.0),
            Color32::RED,
        );
    }
}

fn render_tile_with_transform(
    ui: &mut Ui,
    rect: Rect,
    data: &[u8],
    bounds: &(Point, Point),
    transform: &crate::core::viewport::Transform,
) {
    if data.is_empty() {
        // Render a placeholder for empty tiles with transform applied
        let (min_point, max_point) = *bounds;
        let tile_rect = apply_transform_to_rect(rect, min_point, max_point, transform);
        ui.painter()
            .rect_filled(tile_rect, 0.0, Color32::from_rgb(200, 200, 200));
        return;
    }

    // Create a simple but stable texture key
    let texture_key = format!("tile_{}_{}", (bounds.0.x as i32), (bounds.0.y as i32));

    // Try to load the image
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba_img = img.to_rgba8();
            let (width, height) = rgba_img.dimensions();

            // Only proceed if we have valid dimensions
            if width > 0 && height > 0 {
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &rgba_img.into_raw(),
                );

                let texture =
                    ui.ctx()
                        .load_texture(texture_key, color_image, egui::TextureOptions::LINEAR);

                let texture_id = texture.id();

                // Apply transform to tile positioning (like Leaflet's CSS transforms)
                let (min_point, max_point) = *bounds;
                let tile_rect = apply_transform_to_rect(rect, min_point, max_point, transform);

                // Render the tile with transform applied
                ui.painter().image(
                    texture_id,
                    tile_rect,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
                    Color32::WHITE,
                );

                // Debug: Log successful tile rendering with transform
                if data.len() < 1000 {
                    println!(
                        "🖼️ [RENDER] Rendered transformed tile: {} bytes, {}x{}, scale: {:.2}",
                        data.len(),
                        width,
                        height,
                        transform.scale
                    );
                }
            } else {
                println!("❌ [RENDER] Invalid tile dimensions: {}x{}", width, height);
                render_error_tile_with_transform(ui, rect, bounds, "Invalid dimensions", transform);
            }
        }
        Err(e) => {
            println!(
                "❌ [RENDER] Failed to load tile image: {} ({} bytes), error: {}",
                texture_key,
                data.len(),
                e
            );

            render_error_tile_with_transform(ui, rect, bounds, "Image decode error", transform);
        }
    }
}

/// Apply Leaflet-style transform to a rectangle (translate + scale from origin)
fn apply_transform_to_rect(
    container_rect: Rect,
    min_point: Point,
    max_point: Point,
    transform: &crate::core::viewport::Transform,
) -> Rect {
    // Get the container center for transform origin
    let container_center = container_rect.center();

    // Transform origin is the center of the container (like Leaflet's default)
    let origin = egui::Pos2::new(container_center.x, container_center.y);

    // Calculate original tile position
    let original_min = egui::Pos2::new(
        container_rect.min.x + min_point.x as f32,
        container_rect.min.y + min_point.y as f32,
    );
    let original_max = egui::Pos2::new(
        container_rect.min.x + max_point.x as f32,
        container_rect.min.y + max_point.y as f32,
    );

    // Apply scale around origin (like CSS transform-origin)
    let scale = transform.scale as f32;

    // Transform min point
    let min_offset_from_origin = original_min - origin;
    let scaled_min_offset = min_offset_from_origin * scale;
    let transformed_min = origin
        + scaled_min_offset
        + egui::Vec2::new(transform.translate.x as f32, transform.translate.y as f32);

    // Transform max point
    let max_offset_from_origin = original_max - origin;
    let scaled_max_offset = max_offset_from_origin * scale;
    let transformed_max = origin
        + scaled_max_offset
        + egui::Vec2::new(transform.translate.x as f32, transform.translate.y as f32);

    Rect::from_two_pos(transformed_min, transformed_max)
}

fn render_error_tile_with_transform(
    ui: &mut Ui,
    rect: Rect,
    bounds: &(Point, Point),
    error_msg: &str,
    transform: &crate::core::viewport::Transform,
) {
    let (min_point, max_point) = *bounds;
    let tile_rect = apply_transform_to_rect(rect, min_point, max_point, transform);

    // Render a distinctive error tile
    ui.painter()
        .rect_filled(tile_rect, 0.0, Color32::from_rgb(255, 200, 200));
    ui.painter()
        .rect_stroke(tile_rect, 0.0, egui::Stroke::new(1.0, Color32::RED));

    // Add error text if tile is large enough
    if tile_rect.width() > 50.0 && tile_rect.height() > 50.0 {
        ui.painter().text(
            tile_rect.center(),
            egui::Align2::CENTER_CENTER,
            error_msg,
            egui::FontId::monospace(8.0),
            Color32::RED,
        );
    }
}

fn render_textured_tile(
    ui: &mut Ui,
    rect: Rect,
    texture_id: egui::TextureId,
    bounds: &(Point, Point),
) {
    let (min_point, max_point) = *bounds;
    let tile_rect = Rect::from_two_pos(
        egui::Pos2::new(
            rect.min.x + min_point.x as f32,
            rect.min.y + min_point.y as f32,
        ),
        egui::Pos2::new(
            rect.min.x + max_point.x as f32,
            rect.min.y + max_point.y as f32,
        ),
    );

    // Render the tile
    ui.painter().image(
        texture_id,
        tile_rect,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
        Color32::WHITE,
    );
}

fn render_textured_tile_with_transform(
    ui: &mut Ui,
    rect: Rect,
    texture_id: egui::TextureId,
    bounds: &(Point, Point),
    transform: &crate::core::viewport::Transform,
) {
    let (min_point, max_point) = *bounds;
    let tile_rect = apply_transform_to_rect(rect, min_point, max_point, transform);

    // Render the tile with transform applied
    ui.painter().image(
        texture_id,
        tile_rect,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
        Color32::WHITE,
    );
}

fn render_zoom_controls(
    ui: &mut Ui,
    rect: Rect,
    core_map: &Arc<Mutex<CoreMap>>,
    map: &Map,
    response: &mut Response,
) {
    let control_size = 30.0;
    let zoom_in_rect = egui::Rect::from_min_size(
        rect.right_top() + egui::Vec2::new(-40.0, 10.0),
        egui::Vec2::splat(control_size),
    );
    let zoom_out_rect = egui::Rect::from_min_size(
        rect.right_top() + egui::Vec2::new(-40.0, 45.0),
        egui::Vec2::splat(control_size),
    );

    let zoom_in_response = ui.allocate_rect(zoom_in_rect, egui::Sense::click());
    let zoom_out_response = ui.allocate_rect(zoom_out_rect, egui::Sense::click());

    if zoom_in_response.clicked() {
        if let Ok(mut map_guard) = core_map.try_lock() {
            let current_zoom = map_guard.viewport().zoom;
            let new_zoom = (current_zoom + 1.0).clamp(map.min_zoom, map.max_zoom);
            if new_zoom != current_zoom {
                // Use zoom_to to trigger animations
                let _ = map_guard.zoom_to(new_zoom, None);
            }
        }
        response.mark_changed();
    }

    if zoom_out_response.clicked() {
        if let Ok(mut map_guard) = core_map.try_lock() {
            let current_zoom = map_guard.viewport().zoom;
            let new_zoom = (current_zoom - 1.0).clamp(map.min_zoom, map.max_zoom);
            if new_zoom != current_zoom {
                // Use zoom_to to trigger animations
                let _ = map_guard.zoom_to(new_zoom, None);
            }
        }
        response.mark_changed();
    }

    draw_zoom_button(ui, zoom_in_rect, "+");
    draw_zoom_button(ui, zoom_out_rect, "−");
}

fn draw_zoom_button(ui: &mut Ui, rect: egui::Rect, text: &str) {
    ui.painter().rect_filled(
        rect,
        3.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
    );
    ui.painter().rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(16.0),
        egui::Color32::BLACK,
    );
}

fn render_attribution(ui: &mut Ui, rect: Rect, attribution: &str) {
    let attribution_rect = egui::Rect::from_min_size(
        rect.left_bottom() + egui::Vec2::new(5.0, -20.0),
        egui::Vec2::new(300.0, 15.0),
    );
    ui.painter().text(
        attribution_rect.min,
        egui::Align2::LEFT_BOTTOM,
        attribution,
        egui::FontId::proportional(10.0),
        egui::Color32::from_gray(120),
    );
}

pub trait MapWidgetExt {
    fn map(&mut self) -> Response;
    fn map_at(&mut self, lat: f64, lng: f64) -> Response;
    fn map_at_zoom(&mut self, lat: f64, lng: f64, zoom: f64) -> Response;
}

impl MapWidgetExt for Ui {
    fn map(&mut self) -> Response {
        self.add(Map::new())
    }

    fn map_at(&mut self, lat: f64, lng: f64) -> Response {
        self.add(Map::new().center(lat, lng))
    }

    fn map_at_zoom(&mut self, lat: f64, lng: f64, zoom: f64) -> Response {
        self.add(Map::new().center(lat, lng).zoom(zoom))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_creation() {
        let map = Map::new();
        assert_eq!(map.center.lat, 37.7749);
        assert_eq!(map.zoom, 10.0);
        assert!(map.interactive);
    }

    #[test]
    fn test_builder_pattern() {
        let map = Map::new()
            .center(51.5074, -0.1278)
            .zoom(12.0)
            .interactive(false);

        assert_eq!(map.center.lat, 51.5074);
        assert_eq!(map.zoom, 12.0);
        assert!(!map.interactive);
    }

    #[test]
    fn test_presets() {
        let london = Map::london();
        assert_eq!(london.center.lat, 51.5074);
        assert_eq!(london.center.lng, -0.1278);
        assert_eq!(london.zoom, 10.0);
    }
}
