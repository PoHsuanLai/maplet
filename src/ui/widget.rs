use crate::{
    core::{
        geo::{LatLng, Point},
        map::Map as CoreMap,
    },
    layers::tile::TileLayer,
    rendering::context::{RenderContext, DrawCommand},
};
use egui::{Color32, Rect, Response, Sense, Ui, Vec2, Widget, ColorImage};
use std::sync::{Arc, Mutex};

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
    if let Some(core_map) = ctx.memory(|mem| mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)) {
        if let Ok(mut core_map_guard) = core_map.try_lock() {
            let current_size = core_map_guard.viewport().size;
            let new_size = Point::new(rect.width() as f64, rect.height() as f64);
            
            if (current_size.x - new_size.x).abs() > 1.0 || (current_size.y - new_size.y).abs() > 1.0 {
                core_map_guard.viewport_mut().set_size(new_size);
            }
        }
        return core_map;
    }

    let size = Point::new(rect.width() as f64, rect.height() as f64);
    let is_test = std::thread::current().name().unwrap_or("").contains("test") || cfg!(test);
    
    let mut new_map = if is_test {
        CoreMap::for_testing(map.center, map.zoom, size)
    } else {
        CoreMap::new(map.center, map.zoom, size)
    };
    
    let tile_layer = if is_test {
        TileLayer::for_testing("default_tiles".to_string(), "OpenStreetMap".to_string())
    } else {
        TileLayer::openstreetmap("default_tiles".to_string(), "OpenStreetMap".to_string())
    };
    
    let _ = new_map.add_layer(Box::new(tile_layer));
    
    let core_map_arc = Arc::new(Mutex::new(new_map));
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(map_id, core_map_arc.clone());
    });
    
    core_map_arc
}

fn handle_map_input(
    _ui: &mut Ui,
    response: &mut Response,
    core_map: &Arc<Mutex<CoreMap>>,
    _map: &Map,
    _rect: Rect,
) {
    if response.dragged() {
        let drag_delta = response.drag_delta();
        if drag_delta.length_sq() > 0.5 {
            if let Ok(mut map_guard) = core_map.try_lock() {
                let viewport = map_guard.viewport();
                let current_center = viewport.center;
                let current_zoom = viewport.zoom;
                
                let map_size = 256.0 * 2_f64.powf(current_zoom);
                let lng_per_pixel = 360.0 / map_size;
                let lat_per_pixel = 180.0 / map_size * (1.0 / viewport.center.lat.to_radians().cos());
                
                let lng_delta = -drag_delta.x as f64 * lng_per_pixel;
                let lat_delta = drag_delta.y as f64 * lat_per_pixel;
                
                let new_center = LatLng::new(
                    (current_center.lat + lat_delta).clamp(-85.0511, 85.0511),
                    current_center.lng + lng_delta,
                );
                
                let _ = map_guard.set_view(new_center, current_zoom);
            }
            response.mark_changed();
        }
    }
}

fn render_map(ui: &mut Ui, rect: Rect, core_map: &Arc<Mutex<CoreMap>>) {
    if let Ok(mut map_guard) = core_map.try_lock() {
        let width = rect.width().max(1.0) as u32;
        let height = rect.height().max(1.0) as u32;
        
        if let Ok(mut render_ctx) = RenderContext::new(width, height) {
            let _ = map_guard.update_and_render(&mut render_ctx);
            let drawing_queue = render_ctx.get_drawing_queue();

            for cmd in drawing_queue.iter() {
                match cmd {
                    DrawCommand::Tile { data, bounds, .. } => {
                        render_tile(ui, rect, data, bounds);
                    }
                    DrawCommand::TileTextured { texture_id, bounds, .. } => {
                        render_textured_tile(ui, rect, *texture_id, bounds);
                    }
                    _ => {}
                }
            }
        }
    } else {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(230, 230, 230));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Loading map...",
            egui::FontId::proportional(16.0),
            Color32::from_gray(100),
        );
    }
}

fn render_tile(ui: &mut Ui, rect: Rect, data: &[u8], bounds: &(Point, Point)) {
    if data.is_empty() {
        return;
    }

    if let Ok(img) = image::load_from_memory(data) {
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();
        let color_image = ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &rgba_img.into_raw(),
        );

        let texture_key = format!("tile_{}_{}", bounds.0.x as i32, bounds.0.y as i32);
        let texture = ui.ctx().load_texture(texture_key, color_image, egui::TextureOptions::default());

        let (min_point, max_point) = *bounds;
        let tile_rect = Rect::from_two_pos(
            egui::Pos2::new(rect.min.x + min_point.x as f32, rect.min.y + min_point.y as f32),
            egui::Pos2::new(rect.min.x + max_point.x as f32, rect.min.y + max_point.y as f32),
        );

        ui.painter().image(
            texture.id(),
            tile_rect,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
            Color32::WHITE,
        );
    }
}

fn render_textured_tile(ui: &mut Ui, rect: Rect, texture_id: egui::TextureId, bounds: &(Point, Point)) {
    let (min_point, max_point) = *bounds;
    let tile_rect = Rect::from_two_pos(
        egui::Pos2::new(rect.min.x + min_point.x as f32, rect.min.y + min_point.y as f32),
        egui::Pos2::new(rect.min.x + max_point.x as f32, rect.min.y + max_point.y as f32),
    );

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
            let current_center = map_guard.viewport().center;
            let new_zoom = (current_zoom + 1.0).clamp(map.min_zoom, map.max_zoom);
            if new_zoom != current_zoom {
                let _ = map_guard.set_view(current_center, new_zoom);
            }
        }
        response.mark_changed();
    }

    if zoom_out_response.clicked() {
        if let Ok(mut map_guard) = core_map.try_lock() {
            let current_zoom = map_guard.viewport().zoom;
            let current_center = map_guard.viewport().center;
            let new_zoom = (current_zoom - 1.0).clamp(map.min_zoom, map.max_zoom);
            if new_zoom != current_zoom {
                let _ = map_guard.set_view(current_center, new_zoom);
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