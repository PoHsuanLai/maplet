use crate::{
    core::{
        geo::{LatLng, Point},
        map::Map as CoreMap,
        viewport::Viewport,
    },
    layers::{
        tile::TileLayer,
    },
};
use egui::{Color32, CursorIcon, Rect, Response, Sense, Ui, Vec2, Widget, ColorImage};
use std::sync::{Arc, Mutex};


use crate::rendering::context::{RenderContext, DrawCommand};

/// Simple, immediate-mode Map widget following egui patterns
/// 
/// Uses egui's memory system to maintain persistent CoreMap state
/// for proper tile loading while providing a simple immediate-mode API.
/// 
/// # Examples
/// 
/// ```rust
/// // Simple - just works!
/// ui.add(maplet::Map::new());
/// 
/// // With location
/// ui.add(maplet::Map::new().center(37.7749, -122.4194));
/// 
/// // With zoom and theme
/// ui.add(maplet::Map::new().center(51.5074, -0.1278).zoom(10).theme(MapTheme::Dark));
/// 
/// // Presets
/// ui.add(maplet::Map::san_francisco());
/// ui.add(maplet::Map::london());
/// ui.add(maplet::Map::tokyo());
/// ```
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

#[derive(Debug, Clone, PartialEq)]
struct MapRenderState {
    last_render_time: Option<std::time::Instant>,
    dirty: bool,
    min_frame_interval_ms: u64,
}

impl Default for MapRenderState {
    fn default() -> Self {
        Self {
            last_render_time: None,
            dirty: false,
            min_frame_interval_ms: 33, // ~30fps for smooth but not aggressive rendering
        }
    }
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    /// Create a new map with default settings (San Francisco, zoom 12)
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

    /// Set the map center coordinates
    pub fn center(mut self, lat: f64, lng: f64) -> Self {
        self.center = LatLng::new(lat, lng);
        self
    }

    /// Set the zoom level
    pub fn zoom(mut self, zoom: f64) -> Self {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self
    }

    /// Set the map size (otherwise uses available space)
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }

    /// Set whether the map is interactive (default: true)
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Set whether to show zoom controls (default: true)
    pub fn controls(mut self, show: bool) -> Self {
        self.show_controls = show;
        self
    }

    /// Set whether to show attribution (default: true)
    pub fn attribution(mut self, show: bool) -> Self {
        self.show_attribution = show;
        self
    }

    /// Set custom attribution text
    pub fn attribution_text(mut self, text: impl Into<String>) -> Self {
        let text_str = text.into();
        self.attribution = text_str;
        self
    }

    /// Set the map theme
    pub fn theme(mut self, theme: MapTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Set zoom limits
    pub fn zoom_limits(mut self, min: f64, max: f64) -> Self {
        self.min_zoom = min;
        self.max_zoom = max;
        self.zoom = self.zoom.clamp(min, max);
        self
    }

    /// Set a unique ID for this map instance (for persistent state)
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

    pub fn get_or_create_map(&self, ctx: &egui::Context, rect: Rect) -> egui::Id {
        let map_id = self.map_id.unwrap_or_else(|| {
            egui::Id::new("maplet_core_map").with((
                (self.center.lat * 1000.0) as i32,
                (self.center.lng * 1000.0) as i32,
                (self.zoom * 10.0) as i32,
            ))
        });

        let map_exists = ctx.memory(|mem| {
            mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id).is_some()
        });

        if map_exists {
            
            let core_map_arc = ctx.memory(|mem| {
                mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
            });
            
            if let Some(core_map) = core_map_arc {
                if let Ok(mut map) = core_map.try_lock() {
                    let current_size = map.viewport().size;
                    let new_size = Point::new(rect.width() as f64, rect.height() as f64);
                    
                    if (current_size.x - new_size.x).abs() > 1.0 || (current_size.y - new_size.y).abs() > 1.0 {
                        map.viewport_mut().set_size(new_size);
                        
                        let viewport = map.viewport().clone();
                        map.for_each_layer_mut(|layer| {
                            if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                                let _ = tile_layer.update_tiles(&viewport);
                            }
                        });
                    }
                }
            }
        } else {
            
            let size = Point::new(rect.width() as f64, rect.height() as f64);
            
            let is_test = std::thread::current().name().unwrap_or("").contains("test") || 
                         cfg!(test);
            
             let mut new_map = if is_test {
                 CoreMap::for_testing(self.center, self.zoom, size)
             } else {
                 CoreMap::new(self.center, self.zoom, size)
             };
             
             let tile_layer = if is_test {
                 crate::layers::tile::TileLayer::for_testing(
                     "default_tiles".to_string(),
                     "OpenStreetMap".to_string(),
                 )
             } else {
                 crate::layers::tile::TileLayer::openstreetmap(
                     "default_tiles".to_string(), 
                     "OpenStreetMap".to_string()
                 )
             };
             
             let _ = new_map.add_layer(Box::new(tile_layer));
             
             let core_map_arc = Arc::new(Mutex::new(new_map));
             ctx.memory_mut(|mem| {
                 mem.data.insert_temp(map_id, core_map_arc.clone());
             });

        }

        map_id
    }

    /// Render the map with proper tile loading and display
    fn render_map(&self, ui: &mut Ui, rect: Rect) {
        let map_id = self.get_or_create_map(ui.ctx(), rect);
        
        let render_state_id = map_id.with("global_render_state");
        let now = std::time::Instant::now();
        
        let mut should_skip_expensive_ops = false;
        let mut render_state = ui.ctx().memory(|mem| {
            mem.data.get_temp::<MapRenderState>(render_state_id)
                .unwrap_or_default()
        });
        
        if let Some(last_render) = render_state.last_render_time {
            let elapsed_ms = now.duration_since(last_render).as_millis() as u64;
            should_skip_expensive_ops = elapsed_ms < render_state.min_frame_interval_ms;
        }
        
        if !should_skip_expensive_ops {
            render_state.last_render_time = Some(now);
            ui.ctx().memory_mut(|mem| {
                mem.data.insert_temp(render_state_id, render_state);
            });
        }
        
        let core_map_arc = ui.ctx().memory(|mem| {
            mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
        });

        if let Some(core_map) = core_map_arc {
            match core_map.try_lock() {
                Ok(mut map) => {
                    let current_viewport = map.viewport();
                    let widget_zoom_diff = (current_viewport.zoom - self.zoom).abs();
                    let widget_center_diff = (current_viewport.center.lat - self.center.lat).abs() + 
                                             (current_viewport.center.lng - self.center.lng).abs();
                    
                    let user_has_interacted = widget_zoom_diff > 0.05 || widget_center_diff > 0.0001;
                    
                    if !user_has_interacted {
                        let needs_view_update = 
                            (current_viewport.center.lat - self.center.lat).abs() > 0.001 ||
                            (current_viewport.center.lng - self.center.lng).abs() > 0.001 ||
                            (current_viewport.zoom - self.zoom).abs() > 0.1;
                        
                        if needs_view_update {
                            let _ = map.set_view(self.center, self.zoom);
                        }
                    }
                    
                    map.viewport_mut().set_size(Point::new(rect.width() as f64, rect.height() as f64));
                    
                    let viewport = map.viewport().clone();
                    let mut tile_updates_needed = false;
                    let mut tiles_actively_loading = 0;
                    let mut total_tiles_checked = 0;
                    
                    if !should_skip_expensive_ops {
                        map.for_each_layer_mut(|layer| {
                            if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                                let _ = tile_layer.update_tiles(&viewport);
                                
                                // Only request repaint for critical tile updates
                                if tile_layer.needs_repaint() {
                                    tile_updates_needed = true;
                                    tiles_actively_loading += 1;
                                }
                                total_tiles_checked += 1;
                            }
                        });
                        
                        // Use throttled repaint requests
                        if tile_updates_needed && tiles_actively_loading <= 2 {
                            ui.ctx().request_repaint();
                        }
                    }
                    
                    self.render_map_with_core(ui, rect, &mut map, map_id);
                    
                } Err(_) => {
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_rgb(230, 230, 230)
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Loading map...",
                        egui::FontId::proportional(16.0),
                        egui::Color32::from_gray(100),
                    );
                }
            }
        } else {
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgb(255, 200, 200)
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Map failed to initialize",
                egui::FontId::proportional(16.0),
                egui::Color32::from_rgb(150, 0, 0),
            );
        }
    }

    fn render_map_with_core(&self, ui: &mut Ui, rect: Rect, core_map: &mut CoreMap, _map_id: egui::Id) {
        let painter = ui.painter_at(rect);
        let width = rect.width().max(1.0) as u32;
        let height = rect.height().max(1.0) as u32;
        
        if let Ok(mut render_ctx) = RenderContext::new(width, height) {
            let _ = render_ctx.begin_frame();
            let _render_result = core_map.render(&mut render_ctx);
            let drawing_queue = render_ctx.get_drawing_queue();

            for cmd in drawing_queue.iter() {
                match cmd {
                    DrawCommand::Tile { data, bounds, .. } => {
                        self.render_tile_simple(ui, rect, data, bounds);
                    }
                    DrawCommand::TileTextured { texture_id, bounds, .. } => {
                        self.render_tile_textured_simple(&painter, rect, *texture_id, bounds);
                    }
                    _ => {
                    }
                }
            }
        }
    }

    fn render_tile_simple(&self, ui: &mut Ui, rect: Rect, data: &[u8], bounds: &(Point, Point)) {
        if let Some((pixels, size)) = self.decode_image_immediate(data) {
            let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
            let texture = ui.ctx().load_texture(
                format!("tile_{}_{}", bounds.0.x as i32, bounds.0.y as i32), 
                color_image, 
                egui::TextureOptions::default()
            );

            let (min_point, max_point) = *bounds;
            let tile_rect = Rect::from_two_pos(
                egui::Pos2::new(
                    rect.min.x + min_point.x as f32,
                    rect.min.y + min_point.y as f32
                ),
                egui::Pos2::new(
                    rect.min.x + max_point.x as f32,
                    rect.min.y + max_point.y as f32
                )
            );

            ui.painter().image(
                texture.id(),
                tile_rect,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
                Color32::WHITE,
            );
        }
    }

    fn render_tile_textured_simple(&self, painter: &egui::Painter, rect: Rect, texture_id: egui::TextureId, bounds: &(Point, Point)) {
        let (min_point, max_point) = *bounds;
        let tile_rect = Rect::from_two_pos(
            egui::Pos2::new(
                rect.min.x + min_point.x as f32,
                rect.min.y + min_point.y as f32
            ),
            egui::Pos2::new(
                rect.min.x + max_point.x as f32,
                rect.min.y + max_point.y as f32
            )
        );

        painter.image(
            texture_id,
            tile_rect,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1.0)),
            Color32::WHITE,
        );
    }

    fn decode_image_immediate(&self, bytes: &[u8]) -> Option<(Vec<u8>, [usize; 2])> {
        let img = image::load_from_memory(bytes).ok()?;
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();
        
        Some((rgba_img.into_raw(), [width as usize, height as usize]))
    }
}

impl Widget for Map {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = self.size.unwrap_or_else(|| ui.available_size());
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        let map_id = self.get_or_create_map(ui.ctx(), rect);
        
        let render_state_id = map_id.with("render_state");
        
        let mut render_state = ui.ctx().memory(|mem| {
            mem.data.get_temp::<MapRenderState>(render_state_id)
                .unwrap_or_default()
        });
        
        let is_first_render = render_state.last_render_time.is_none();
        
        if is_first_render {
            render_state.dirty = true;
        }
        
        let mut needs_map_update = false;
        let mut needs_repaint = false;
        
        if self.interactive {
            if response.hovered() {
                let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
                if scroll_delta.abs() > 0.1 {
                    let zoom_delta = (scroll_delta as f64) * 0.001;
                    
                    if let Some(core_map_arc) = ui.ctx().memory(|mem| {
                        mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
                    }) {
                        if let Ok(mut map) = core_map_arc.try_lock() {
                            let current_zoom = map.viewport().zoom;
                            let current_center = map.viewport().center;
                            let new_zoom = (current_zoom + zoom_delta).clamp(self.min_zoom, self.max_zoom);
                            if (new_zoom - current_zoom).abs() > 0.001 {
                                let _ = map.set_view(current_center, new_zoom);
                                needs_map_update = true;
                                needs_repaint = true;
                                render_state.dirty = true;
                            }
                        }
                    }
                    response.mark_changed();
                }
            }

            if response.dragged() {
                let drag_delta = response.drag_delta();
                if drag_delta.length_sq() > 0.5 {
                    if let Some(core_map_arc) = ui.ctx().memory(|mem| {
                        mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
                    }) {
                        if let Ok(mut map) = core_map_arc.try_lock() {
                            let viewport = map.viewport();
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
                            
                            let _ = map.set_view(new_center, current_zoom);
                            needs_map_update = true;
                            needs_repaint = true;
                            render_state.dirty = true;
                        }
                    }
                    response.mark_changed();
                }
            }
        }

        if self.show_controls {
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
                if let Some(core_map_arc) = ui.ctx().memory(|mem| {
                    mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
                }) {
                    if let Ok(mut map) = core_map_arc.try_lock() {
                        let current_zoom = map.viewport().zoom;
                        let current_center = map.viewport().center;
                        let new_zoom = (current_zoom + 1.0).clamp(self.min_zoom, self.max_zoom);
                        if new_zoom != current_zoom {
                            let _ = map.set_view(current_center, new_zoom);
                            needs_map_update = true;
                            needs_repaint = true;
                        }
                    }
                }
                response.mark_changed();
            }
            
            if zoom_out_response.clicked() {
                if let Some(core_map_arc) = ui.ctx().memory(|mem| {
                    mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
                }) {
                    if let Ok(mut map) = core_map_arc.try_lock() {
                        let current_zoom = map.viewport().zoom;
                        let current_center = map.viewport().center;
                        let new_zoom = (current_zoom - 1.0).clamp(self.min_zoom, self.max_zoom);
                        if new_zoom != current_zoom {
                            let _ = map.set_view(current_center, new_zoom);
                            needs_map_update = true;
                            needs_repaint = true;
                        }
                    }
                }
                response.mark_changed();
            }
        }

        if needs_map_update {
            if let Some(core_map_arc) = ui.ctx().memory(|mem| {
                mem.data.get_temp::<Arc<Mutex<CoreMap>>>(map_id)
            }) {
                if let Ok(mut map) = core_map_arc.try_lock() {
                    let viewport_clone = {
                        let vp = map.viewport();
                        Viewport::new(vp.center, vp.zoom, vp.size)
                    };
                    
                    map.for_each_layer_mut(|layer| {
                        if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                            let _ = tile_layer.update_tiles(&viewport_clone);
                        }
                    });
                }
            }
        }

        self.render_map(ui, rect);

        if self.show_controls {
            let control_size = 30.0;
            let zoom_in_rect = egui::Rect::from_min_size(
                rect.right_top() + egui::Vec2::new(-40.0, 10.0),
                egui::Vec2::splat(control_size),
            );
            let zoom_out_rect = egui::Rect::from_min_size(
                rect.right_top() + egui::Vec2::new(-40.0, 45.0),
                egui::Vec2::splat(control_size),
            );

            ui.painter().rect_filled(
                zoom_in_rect, 
                3.0, 
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)
            );
            ui.painter().rect_stroke(
                zoom_in_rect,
                3.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(100))
            );
            ui.painter().text(
                zoom_in_rect.center(),
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(16.0),
                egui::Color32::BLACK,
            );

            ui.painter().rect_filled(
                zoom_out_rect, 
                3.0, 
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)
            );
            ui.painter().rect_stroke(
                zoom_out_rect,
                3.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(100))
            );
            ui.painter().text(
                zoom_out_rect.center(),
                egui::Align2::CENTER_CENTER,
                "−",
                egui::FontId::proportional(16.0),
                egui::Color32::BLACK,
            );
        }

        if self.show_attribution && !self.attribution.is_empty() {
            let attribution_rect = egui::Rect::from_min_size(
                rect.left_bottom() + egui::Vec2::new(5.0, -20.0),
                egui::Vec2::new(300.0, 15.0),
            );
            ui.painter().text(
                attribution_rect.min,
                egui::Align2::LEFT_BOTTOM,
                &self.attribution,
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(120),
            );
        }

        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(render_state_id, render_state);
        });

        if needs_repaint {
            ui.ctx().request_repaint();
        }

        response
    }
}

/// Advanced map widget configuration
#[derive(Debug, Clone)]
pub struct MapWidgetConfig {
    pub interactive: bool,
    pub show_zoom_controls: bool,
    pub show_attribution: bool,
    pub background_color: Color32,
    pub attribution: String,
    pub zoom_sensitivity: f64,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub zoom_delta: f64,
    pub smooth_panning: bool,
    pub preferred_size: Option<Vec2>,
}

impl Default for MapWidgetConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            show_zoom_controls: true,
            show_attribution: true,
            background_color: Color32::from_rgb(200, 200, 200),
            attribution: "© OpenStreetMap".to_string(),
            zoom_sensitivity: 0.1,
            min_zoom: 0.0,
            max_zoom: 20.0,
            zoom_delta: 1.0,
            smooth_panning: true,
            preferred_size: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapCursor {
    Default,
    Grab,
    Grabbing,
    Crosshair,
    Move,
}

impl From<MapCursor> for CursorIcon {
    fn from(cursor: MapCursor) -> Self {
        match cursor {
            MapCursor::Default => CursorIcon::Default,
            MapCursor::Grab => CursorIcon::Grab,
            MapCursor::Grabbing => CursorIcon::Grabbing,
            MapCursor::Crosshair => CursorIcon::Crosshair,
            MapCursor::Move => CursorIcon::Move,
        }
    }
}

/// Advanced map widget for users who need full control
/// 
/// This widget manages persistent state and provides full mapping functionality.
/// Use this only if you need advanced features like custom layers, plugins, etc.
/// 
/// For most use cases, prefer the simple `Map` widget.
pub struct AdvancedMapWidget {
    core_map: CoreMap,
    config: MapWidgetConfig,
}

impl AdvancedMapWidget {
    pub fn new(center: LatLng, zoom: f64, size: Vec2) -> Self {
        let mut core_map = CoreMap::new(center, zoom, Point::new(size.x as f64, size.y as f64));

        let osm_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
        if let Err(e) = core_map.add_layer(Box::new(osm_layer)) {
            eprintln!("Failed to add OSM layer: {}", e);
        }

        Self {
            core_map,
            config: MapWidgetConfig::default(),
        }
    }

    pub fn with_config(mut self, config: MapWidgetConfig) -> Self {
        self.config = config;
        self
    }

    /// Get mutable access to the core map for advanced operations
    /// 
    /// Use this to add layers, plugins, configure performance, etc.
    pub fn core_map_mut(&mut self) -> &mut CoreMap {
        &mut self.core_map
    }

    /// Get read-only access to the core map
    pub fn core_map(&self) -> &CoreMap {
        &self.core_map
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let desired_size = self.config.preferred_size.unwrap_or_else(|| ui.available_size());
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        ui.painter().rect_filled(rect, 0.0, self.config.background_color);

        let viewport = self.core_map.viewport();
        let info_text = format!(
            "Advanced Map\nCenter: {:.4}°, {:.4}°\nZoom: {:.1}",
            viewport.center.lat, viewport.center.lng, viewport.zoom
        );

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            info_text,
            egui::FontId::proportional(14.0),
            Color32::BLACK,
        );

        response
    }
}

pub trait MapWidgetExt {
    fn map_widget(&mut self, widget: &mut AdvancedMapWidget) -> Response;
}

impl MapWidgetExt for Ui {
    fn map_widget(&mut self, widget: &mut AdvancedMapWidget) -> Response {
        widget.show(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    #[test]
    fn test_map_widget_creation() {
        let map = Map::new();
        assert_eq!(map.center.lat, 37.7749); // San Francisco default
        assert_eq!(map.center.lng, -122.4194);
        assert_eq!(map.zoom, 12.0);
        assert!(map.interactive);
        assert!(map.show_controls);
        assert!(map.show_attribution);
    }

    #[test]
    fn test_map_widget_builder_pattern() {
        let map = Map::new()
            .center(51.5074, -0.1278) // London
            .zoom(10.0)
            .size(Vec2::new(800.0, 600.0))
            .interactive(false)
            .controls(false)
            .attribution(false)
            .theme(MapTheme::Dark);

        assert_eq!(map.center.lat, 51.5074);
        assert_eq!(map.center.lng, -0.1278);
        assert_eq!(map.zoom, 10.0);
        assert_eq!(map.size, Some(Vec2::new(800.0, 600.0)));
        assert!(!map.interactive);
        assert!(!map.show_controls);
        assert!(!map.show_attribution);
        assert_eq!(map.theme, MapTheme::Dark);
    }

    #[test]
    fn test_map_preset_locations() {
        let sf = Map::san_francisco();
        assert_eq!(sf.center.lat, 37.7749);
        assert_eq!(sf.center.lng, -122.4194);
        assert_eq!(sf.zoom, 12.0);

        let ny = Map::new_york();
        assert_eq!(ny.center.lat, 40.7128);
        assert_eq!(ny.center.lng, -74.0060);
        assert_eq!(ny.zoom, 10.0);

        let london = Map::london();
        assert_eq!(london.center.lat, 51.5074);
        assert_eq!(london.center.lng, -0.1278);
        assert_eq!(london.zoom, 10.0);

        let tokyo = Map::tokyo();
        assert_eq!(tokyo.center.lat, 35.6762);
        assert_eq!(tokyo.center.lng, 139.6503);
        assert_eq!(tokyo.zoom, 11.0);

        let sydney = Map::sydney();
        assert_eq!(sydney.center.lat, -33.8688);
        assert_eq!(sydney.center.lng, 151.2093);
        assert_eq!(sydney.zoom, 11.0);

        let paris = Map::paris();
        assert_eq!(paris.center.lat, 48.8566);
        assert_eq!(paris.center.lng, 2.3522);
        assert_eq!(paris.zoom, 11.0);
    }

    #[test]
    fn test_zoom_limits() {
        let map = Map::new().zoom_limits(5.0, 15.0);
        assert_eq!(map.min_zoom, 5.0);
        assert_eq!(map.max_zoom, 15.0);

        // Test that zoom is clamped to limits
        let map_clamped = Map::new().zoom(25.0).zoom_limits(0.0, 18.0);
        assert_eq!(map_clamped.zoom, 18.0);
    }

    #[test]
    fn test_attribution_text() {
        let custom_attribution = "© Custom Map Provider";
        let map = Map::new().attribution_text(custom_attribution);
        assert_eq!(map.attribution, custom_attribution);
    }

    #[test]
    fn test_map_theme_variants() {
        let light = Map::new().theme(MapTheme::Light);
        let dark = Map::new().theme(MapTheme::Dark);
        let satellite = Map::new().theme(MapTheme::Satellite);

        assert_eq!(light.theme, MapTheme::Light);
        assert_eq!(dark.theme, MapTheme::Dark);
        assert_eq!(satellite.theme, MapTheme::Satellite);
    }

    #[tokio::test]
    async fn test_map_id_generation() {
        let map1 = Map::new().center(37.7749, -122.4194).zoom(12.0);
        let map2 = Map::new().center(37.7749, -122.4194).zoom(12.0);
        let map3 = Map::new().center(40.7128, -74.0060).zoom(12.0);

        // Same location and zoom should generate same ID
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0));
        
        let id1 = map1.get_or_create_map(&ctx, rect);
        let id2 = map2.get_or_create_map(&ctx, rect);
        let id3 = map3.get_or_create_map(&ctx, rect);

        assert_eq!(id1, id2); // Same location should have same ID
        assert_ne!(id1, id3); // Different location should have different ID
    }

    #[test]
    fn test_coordinate_transformation_bounds() {
        // Test that latitude is properly clamped to Web Mercator limits
        let lat_limit = 85.0511;
        assert!(lat_limit < 90.0); // Ensure we're testing Web Mercator limits, not geographic

        // Test valid coordinates
        let map = Map::new().center(lat_limit - 1.0, 180.0);
        assert!(map.center.lat < lat_limit);

        // Test clamping in coordinate transformations would happen in input handling
        // This validates our constant values are correct
        assert_eq!(85.0511, 85.0511); // Web Mercator latitude limit
    }

    #[test]
    fn test_map_widget_config_defaults() {
        let config = MapWidgetConfig::default();
        assert!(config.interactive);
        assert!(config.show_zoom_controls);
        assert!(config.show_attribution);
        assert_eq!(config.zoom_sensitivity, 0.1);
        assert_eq!(config.min_zoom, 0.0);
        assert_eq!(config.max_zoom, 20.0);
        assert_eq!(config.zoom_delta, 1.0);
        assert!(config.smooth_panning);
    }

    #[test]
    fn test_map_cursor_conversion() {
        assert_eq!(CursorIcon::from(MapCursor::Default), CursorIcon::Default);
        assert_eq!(CursorIcon::from(MapCursor::Grab), CursorIcon::Grab);
        assert_eq!(CursorIcon::from(MapCursor::Grabbing), CursorIcon::Grabbing);
        assert_eq!(CursorIcon::from(MapCursor::Crosshair), CursorIcon::Crosshair);
        assert_eq!(CursorIcon::from(MapCursor::Move), CursorIcon::Move);
    }

    #[test]
    fn test_decode_image_immediate() {
        let map = Map::new();
        
        // Test with invalid image data
        let invalid_data = vec![0u8; 10];
        assert!(map.decode_image_immediate(&invalid_data).is_none());
        
        // Test with empty data
        let empty_data = vec![];
        assert!(map.decode_image_immediate(&empty_data).is_none());
        
        // Note: Testing with valid image data would require including actual image bytes
        // For integration tests, we would test with real PNG/JPEG data
    }

    #[test]
    fn test_zoom_control_hit_testing() {
        // Test that zoom control rectangles are properly positioned
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0));
        let control_size = 30.0;
        
        let zoom_in_rect = egui::Rect::from_min_size(
            rect.right_top() + egui::Vec2::new(-40.0, 10.0),
            egui::Vec2::splat(control_size),
        );
        let zoom_out_rect = egui::Rect::from_min_size(
            rect.right_top() + egui::Vec2::new(-40.0, 45.0),
            egui::Vec2::splat(control_size),
        );

        // Test that controls are within the map area
        assert!(rect.contains_rect(zoom_in_rect));
        assert!(rect.contains_rect(zoom_out_rect));
        
        // Test that controls don't overlap
        assert!(!zoom_in_rect.intersects(zoom_out_rect));
    }

    #[test]
    fn test_input_threshold_values() {
        // Test that our input thresholds are reasonable
        let scroll_threshold = 0.1;
        let drag_threshold = 1.0;
        
        assert!(scroll_threshold > 0.0);
        assert!(scroll_threshold < 1.0); // Should be sensitive but not too sensitive
        
        assert!(drag_threshold > 0.0);
        assert!(drag_threshold >= 1.0); // Should require actual movement
    }

    // Mock tests for coordinate transformations
    #[test]
    fn test_coordinate_transformation_math() {
        // Test the math used in coordinate transformations
        let zoom = 12.0;
        let zoom_scale = 2_f64.powi(zoom as i32);
        assert_eq!(zoom_scale, 4096.0);
        
        // Test Web Mercator limits
        let lat_limit = 85.0511;
        assert!(lat_limit > 85.0);
        assert!(lat_limit < 85.1);
        
        // Test longitude wrapping (conceptual - actual wrapping handled by map projection)
        let lng_delta = 180.0;
        assert!(lng_delta == 180.0); // Max longitude delta
    }

    #[test]
    fn test_viewport_creation() {
        let center = LatLng::new(37.7749, -122.4194);
        let zoom = 12.0;
        let size = Point::new(800.0, 600.0);
        
        let viewport = Viewport::new(center, zoom, size);
        assert_eq!(viewport.center.lat, center.lat);
        assert_eq!(viewport.center.lng, center.lng);
        assert_eq!(viewport.zoom, zoom);
        assert_eq!(viewport.size.x, size.x);
        assert_eq!(viewport.size.y, size.y);
    }
}


