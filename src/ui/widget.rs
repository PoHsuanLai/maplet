use crate::{
    animation::TransitionManager,
    core::{
        geo::{LatLng, Point},
        map::Map,
        viewport::Viewport,
    },
    input::events::{EventHandled, InputEvent, KeyCode, KeyModifiers},
    Result,
};
use egui::{
    Align2,
    Color32,
    FontId,
    Pos2,
    Rect,
    Response,
    Sense,
    Stroke,
    Ui,
    Vec2,
};
use egui::epaint::{Mesh, Vertex};
use std::sync::{Arc, Mutex};
use std::num::NonZeroUsize;
use lru::LruCache;
use pollster;

/// Configuration for the map widget
#[derive(Debug, Clone)]
pub struct MapWidgetConfig {
    /// Whether the map should be interactive
    pub interactive: bool,
    /// Whether to show zoom controls
    pub show_zoom_controls: bool,
    /// Whether to show attribution
    pub show_attribution: bool,
    /// Minimum zoom level
    pub min_zoom: f64,
    /// Maximum zoom level
    pub max_zoom: f64,
    /// Zoom snap value
    pub zoom_snap: f64,
    /// Zoom delta per discrete action
    pub zoom_delta: f64,
    /// Default cursor over the map
    pub cursor: MapCursor,
    /// Background color when no tiles are loaded
    pub background_color: Color32,
    /// Attribution text
    pub attribution: String,
}

impl Default for MapWidgetConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            show_zoom_controls: true,
            show_attribution: true,
            min_zoom: 0.0,
            max_zoom: 18.0,
            zoom_snap: 1.0,
            zoom_delta: 1.0,
            cursor: MapCursor::Default,
            background_color: Color32::from_rgb(200, 200, 200),
            attribution: "© OpenStreetMap contributors".to_string(),
        }
    }
}

/// Cursor types for the map
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapCursor {
    Default,
    Pointer,
    Crosshair,
    Grabbing,
    Grab,
    Move,
    ZoomIn,
    ZoomOut,
}

impl From<MapCursor> for egui::CursorIcon {
    fn from(cursor: MapCursor) -> Self {
        match cursor {
            MapCursor::Default => egui::CursorIcon::Default,
            MapCursor::Pointer => egui::CursorIcon::PointingHand,
            MapCursor::Crosshair => egui::CursorIcon::Crosshair,
            MapCursor::Grabbing => egui::CursorIcon::Grabbing,
            MapCursor::Grab => egui::CursorIcon::Grab,
            MapCursor::Move => egui::CursorIcon::Move,
            MapCursor::ZoomIn => egui::CursorIcon::ZoomIn,
            MapCursor::ZoomOut => egui::CursorIcon::ZoomOut,
        }
    }
}

/// State for drag operations
#[derive(Debug, Clone)]
struct DragState {
    is_dragging: bool,
    start_position: Pos2,
    last_position: Pos2,
    start_viewport: Option<Viewport>,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            is_dragging: false,
            start_position: Pos2::ZERO,
            last_position: Pos2::ZERO,
            start_viewport: None,
        }
    }
}

/// The main map widget that can be embedded in egui applications
pub struct MapWidget {
    /// The map instance
    pub map: Arc<Mutex<Map>>,
    /// Widget configuration
    pub config: MapWidgetConfig,
    /// Current widget size
    pub size: Vec2,
    /// Drag state for pan operations
    drag_state: DragState,
    /// Transition manager for animations
    transition_manager: TransitionManager,
    /// Whether the widget has focus
    has_focus: bool,
    /// Event callbacks
    pub on_click: Option<Box<dyn Fn(LatLng) + Send + Sync>>,
    pub on_double_click: Option<Box<dyn Fn(LatLng) + Send + Sync>>,
    pub on_zoom_changed: Option<Box<dyn Fn(f64) + Send + Sync>>,
    pub on_center_changed: Option<Box<dyn Fn(LatLng) + Send + Sync>>,
    pub on_viewport_changed: Option<Box<dyn Fn(&Viewport) + Send + Sync>>,
    /// Last frame time for delta calculations
    last_frame_time: Option<std::time::Instant>,
    /// Cached tile textures with LRU eviction
    tile_textures: LruCache<String, egui::TextureHandle>,
    /// Whether a repaint is currently required (set during `update`)            
    needs_repaint: bool,
}

impl MapWidget {
    /// Create a new map widget
    pub fn new(map: Map) -> Self {
        Self {
            map: Arc::new(Mutex::new(map)),
            config: MapWidgetConfig::default(),
            size: Vec2::new(800.0, 600.0),
            drag_state: DragState::default(),
            transition_manager: TransitionManager::new(),
            has_focus: false,
            on_click: None,
            on_double_click: None,
            on_zoom_changed: None,
            on_center_changed: None,
            on_viewport_changed: None,
            last_frame_time: None,
            tile_textures: LruCache::new(NonZeroUsize::new(512).unwrap()),
            needs_repaint: false,
        }
    }

    /// Create a new map widget with configuration
    pub fn with_config(map: Map, config: MapWidgetConfig) -> Self {
        let mut widget = Self::new(map);
        widget.config = config;
        widget
    }

    /// Set the widget size
    pub fn set_size(&mut self, size: Vec2) {
        self.size = size;
        if let Ok(mut map) = self.map.lock() {
            map.viewport_mut()
                .set_size(Point::new(size.x as f64, size.y as f64));
        }
    }

    /// Get the current viewport
    pub fn viewport(&self) -> Option<Viewport> {
        self.map.lock().ok().map(|map| map.viewport().clone())
    }

    /// Set click callback
    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: Fn(LatLng) + Send + Sync + 'static,
    {
        self.on_click = Some(Box::new(callback));
        self
    }

    /// Set double click callback
    pub fn on_double_click<F>(mut self, callback: F) -> Self
    where
        F: Fn(LatLng) + Send + Sync + 'static,
    {
        self.on_double_click = Some(Box::new(callback));
        self
    }

    /// Set zoom changed callback
    pub fn on_zoom_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.on_zoom_changed = Some(Box::new(callback));
        self
    }

    /// Set center changed callback
    pub fn on_center_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(LatLng) + Send + Sync + 'static,
    {
        self.on_center_changed = Some(Box::new(callback));
        self
    }

    /// Set viewport changed callback
    pub fn on_viewport_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Viewport) + Send + Sync + 'static,
    {
        self.on_viewport_changed = Some(Box::new(callback));
        self
    }

    /// Show the map widget in the UI
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let desired_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        // Update size if changed
        if desired_size != self.size {
            self.set_size(desired_size);
        }

        // Handle input events
        self.handle_ui_events(ui, &response, rect);

        // Update the map
        let now = std::time::Instant::now();
        let delta_time = if let Some(last_time) = self.last_frame_time {
            now.duration_since(last_time).as_secs_f64()
        } else {
            0.0
        };
        self.last_frame_time = Some(now);

        if let Err(e) = self.update(delta_time) {
            log::error!("Failed to update map: {}", e);
        }

        // Draw the map
        self.paint_map(ui, rect);

        // Draw UI overlays
        if self.config.show_zoom_controls {
            self.draw_zoom_controls(ui, rect);
        }

        if self.config.show_attribution {
            self.draw_attribution(ui, rect);
        }

        // Ask egui for a repaint only if something actually changed (dragging, animation, tiles loading)
        if self.needs_repaint {
            ui.ctx().request_repaint();
            // Reset flag so that we only repaint continuously while it is needed
            self.needs_repaint = false;
        }

        response
    }

    /// Handle UI events from egui
    fn handle_ui_events(&mut self, ui: &mut Ui, response: &Response, rect: Rect) {
        if !self.config.interactive {
            return;
        }

        // Handle hover and focus
        if response.hovered() {
            ui.ctx().set_cursor_icon(self.config.cursor.into());
            self.has_focus = true;
        } else {
            self.has_focus = false;
        }

        // Handle clicks
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let relative_pos = pos - rect.min;
                if let Err(e) =
                    self.handle_click(Point::new(relative_pos.x as f64, relative_pos.y as f64))
                {
                    log::error!("Failed to handle click: {}", e);
                }
            }
        }

        // Handle double clicks
        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let relative_pos = pos - rect.min;
                if let Err(e) = self
                    .handle_double_click(Point::new(relative_pos.x as f64, relative_pos.y as f64))
                {
                    log::error!("Failed to handle double click: {}", e);
                }
            }
        }

        // Handle dragging
        if response.dragged() {
            if !self.drag_state.is_dragging {
                // Start drag
                if let Some(pos) = response.interact_pointer_pos() {
                    self.drag_state.is_dragging = true;
                    self.drag_state.start_position = pos;
                    self.drag_state.last_position = pos;
                    self.drag_state.start_viewport = self.viewport();
                }
            } else {
                // Continue drag
                if let Some(pos) = response.interact_pointer_pos() {
                    let delta = pos - self.drag_state.last_position;
                    if let Err(e) = self.handle_drag(Point::new(delta.x as f64, delta.y as f64)) {
                        log::error!("Failed to handle drag: {}", e);
                    }
                    self.drag_state.last_position = pos;
                }
            }
        } else if self.drag_state.is_dragging {
            // End drag
            self.drag_state.is_dragging = false;
            if let Err(e) = self.handle_drag_end() {
                log::error!("Failed to handle drag end: {}", e);
            }
        }

        // Handle scroll for zoom
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0
            && rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
        {
            let mouse_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default()) - rect.min;
            if let Err(e) = self.handle_scroll(
                scroll_delta as f64,
                Point::new(mouse_pos.x as f64, mouse_pos.y as f64),
            ) {
                log::error!("Failed to handle scroll: {}", e);
            }
        }

        // Handle keyboard input if focused
        if self.has_focus {
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::Key {
                        key,
                        pressed,
                        modifiers,
                        ..
                    } = event
                    {
                        if *pressed {
                            let key_code = match key {
                                egui::Key::ArrowUp => KeyCode::ArrowUp,
                                egui::Key::ArrowDown => KeyCode::ArrowDown,
                                egui::Key::ArrowLeft => KeyCode::ArrowLeft,
                                egui::Key::ArrowRight => KeyCode::ArrowRight,
                                egui::Key::Plus => KeyCode::Plus,
                                egui::Key::Minus => KeyCode::Minus,
                                _ => return,
                            };

                            let key_modifiers = KeyModifiers {
                                shift: modifiers.shift,
                                ctrl: modifiers.ctrl,
                                alt: modifiers.alt,
                                meta: modifiers.mac_cmd,
                            };

                            if let Err(e) = self.handle_key_press(key_code, key_modifiers) {
                                log::error!("Failed to handle key press: {}", e);
                            }
                        }
                    }
                }
            });
        }
    }

    /// Paint the map onto the UI
    fn paint_map(&mut self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter_at(rect);

        // Fill background
        painter.rect_filled(rect, 0.0, self.config.background_color);

        // Render map layers via RenderContext
        if let Ok(mut map) = self.map.lock() {
            // Create a render context matching widget size
            let width = rect.width().max(1.0) as u32;
            let height = rect.height().max(1.0) as u32;

            if let Ok(mut render_ctx) = crate::rendering::context::RenderContext::new(width, height) {
                let _ = render_ctx.begin_frame();

                // Call map.render (async)
                let _ = pollster::block_on(map.render(&mut render_ctx));

                // Build meshes grouped by texture for efficient batching
                let mut mesh_map: std::collections::HashMap<egui::TextureId, Mesh> = std::collections::HashMap::new();

                log::debug!("drawing queue has {} commands", render_ctx.get_drawing_queue().len());

                // Helper closure to obtain a texture handle (async decode + upload on first use)
                let mut get_texture = |key: &str, bytes: &[u8]| -> Option<egui::TextureHandle> {
                    log::trace!("get_texture lookup key={}", key);
                    if let Some(handle) = self.tile_textures.get(key) {
                        log::trace!("texture cache hit: {}", key);
                        return Some(handle.clone());
                    }

                    // Decode on a background thread using std::thread to avoid requiring a Tokio runtime.
                    let bytes_owned = bytes.to_vec();
                    let handle_join = std::thread::spawn(move || {
                        // Attempt raw RGBA first
                        let pixel_count = bytes_owned.len() / 4;
                        let side = (pixel_count as f32).sqrt() as usize;
                        if side * side * 4 == bytes_owned.len() {
                            return Some((bytes_owned, [side, side]));
                        }

                        // PNG decode fallback
                        match image::load_from_memory(&bytes_owned) {
                            Ok(img) => {
                                let img_rgba = img.to_rgba8();
                                let size = [img_rgba.width() as usize, img_rgba.height() as usize];
                                let pixels = img_rgba.into_raw();
                                Some((pixels, size))
                            }
                            Err(_) => None,
                        }
                    });

                    let decode_result = handle_join.join().unwrap_or(None);

                    let (pixels, size) = decode_result?;

                    log::debug!("decoded texture {} bytes={} size={}x{}", key, pixels.len(), size[0], size[1]);

                    // Upload to egui texture registry (on this thread)
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                    let handle = ui.ctx().load_texture(key.to_owned(), color_image, egui::TextureOptions::default());

                    self.tile_textures.put(key.to_owned(), handle.clone());
                    log::debug!("uploaded texture {} ({}x{})", key, size[0], size[1]);
                    Some(handle)
                };

                for cmd in render_ctx.get_drawing_queue() {
                    match cmd {
                        crate::rendering::context::DrawCommand::Tile { data, bounds, .. } => {
                            let key = format!("tile-{}-{}-{}-{}", bounds.0.x, bounds.0.y, bounds.1.x, bounds.1.y);

                            let tex_handle = match get_texture(&key, data) {
                                Some(h) => h,
                                None => continue,
                            };

                            let (min_point, max_point) = *bounds;
                            let dest_min_vec = rect.min + Vec2::new(min_point.x as f32, min_point.y as f32);
                            let dest_max_vec = rect.min + Vec2::new(max_point.x as f32, max_point.y as f32);

                            let tl = Pos2::new(dest_min_vec.x, dest_min_vec.y);
                            let tr = Pos2::new(dest_max_vec.x, dest_min_vec.y);
                            let br = Pos2::new(dest_max_vec.x, dest_max_vec.y);
                            let bl = Pos2::new(dest_min_vec.x, dest_max_vec.y);

                            let mesh = mesh_map.entry(tex_handle.id()).or_insert_with(|| {
                                let mut m = Mesh::default();
                                m.texture_id = tex_handle.id();
                                m
                            });

                            let base_idx = mesh.vertices.len() as u32;

                            mesh.vertices.push(Vertex { pos: tl, uv: Pos2::new(0.0, 0.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: tr, uv: Pos2::new(1.0, 0.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: br, uv: Pos2::new(1.0, 1.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: bl, uv: Pos2::new(0.0, 1.0), color: Color32::WHITE });

                            mesh.indices.extend_from_slice(&[
                                base_idx,
                                base_idx + 1,
                                base_idx + 2,
                                base_idx,
                                base_idx + 2,
                                base_idx + 3,
                            ]);
                        }
                        crate::rendering::context::DrawCommand::TileTextured { texture_id, bounds, .. } => {
                            let mesh = mesh_map.entry(*texture_id).or_insert_with(|| {
                                let mut m = Mesh::default();
                                m.texture_id = *texture_id;
                                m
                            });

                            let (min_point, max_point) = *bounds;
                            let dest_min_vec = rect.min + Vec2::new(min_point.x as f32, min_point.y as f32);
                            let dest_max_vec = rect.min + Vec2::new(max_point.x as f32, max_point.y as f32);

                            let tl = Pos2::new(dest_min_vec.x, dest_min_vec.y);
                            let tr = Pos2::new(dest_max_vec.x, dest_min_vec.y);
                            let br = Pos2::new(dest_max_vec.x, dest_max_vec.y);
                            let bl = Pos2::new(dest_min_vec.x, dest_max_vec.y);

                            let base_idx = mesh.vertices.len() as u32;
                            mesh.vertices.push(Vertex { pos: tl, uv: Pos2::new(0.0, 0.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: tr, uv: Pos2::new(1.0, 0.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: br, uv: Pos2::new(1.0, 1.0), color: Color32::WHITE });
                            mesh.vertices.push(Vertex { pos: bl, uv: Pos2::new(0.0, 1.0), color: Color32::WHITE });

                            mesh.indices.extend_from_slice(&[
                                base_idx,
                                base_idx + 1,
                                base_idx + 2,
                                base_idx,
                                base_idx + 2,
                                base_idx + 3,
                            ]);
                        }
                        _ => {}
                    }
                }

                // Log mesh statistics for debugging rendering issues
                log::debug!("mesh_map has {} texture entries", mesh_map.len());
                for (tex_id, mesh) in &mesh_map {
                    log::debug!(
                        "mesh for tex {:?}: verts={} indices={}",
                        tex_id,
                        mesh.vertices.len(),
                        mesh.indices.len()
                    );
                }

                // Submit all meshes to the painter
                for mesh in mesh_map.values() {
                    painter.add(egui::Shape::mesh(mesh.clone()));
                }
            }

            {
                let vp = map.viewport().clone();
                log::debug!(
                    "viewport center lat={:.6} lng={:.6} zoom={:.2}",
                    vp.center.lat,
                    vp.center.lng,
                    vp.zoom
                );
            }
        }

        // Draw viewport info for debugging
        if let Some(viewport) = self.viewport() {
            let text = format!(
                "Center: {:.4}, {:.4}\nZoom: {:.2}",
                viewport.center.lat, viewport.center.lng, viewport.zoom
            );
            painter.text(
                rect.min + Vec2::new(10.0, 10.0),
                Align2::LEFT_TOP,
                text,
                FontId::monospace(12.0),
                Color32::BLACK,
            );
        }
    }

    /// Draw zoom controls
    fn draw_zoom_controls(&mut self, ui: &mut Ui, rect: Rect) {
        let button_size = Vec2::new(30.0, 30.0);
        let margin = 10.0;

        // Position zoom controls in top right
        let zoom_in_rect = Rect::from_min_size(
            rect.max - Vec2::new(margin + button_size.x, margin + button_size.y),
            button_size,
        );
        let zoom_out_rect = Rect::from_min_size(
            zoom_in_rect.min + Vec2::new(0.0, button_size.y + 5.0),
            button_size,
        );

        // Draw zoom in button
        let zoom_in_response = ui.allocate_rect(zoom_in_rect, Sense::click());
        if zoom_in_response.clicked() {
            if let Err(e) = self.zoom_in(None) {
                log::error!("Failed to zoom in: {}", e);
            }
        }

        let zoom_in_color = if zoom_in_response.hovered() {
            Color32::LIGHT_GRAY
        } else {
            Color32::WHITE
        };

        ui.painter().rect_filled(zoom_in_rect, 2.0, zoom_in_color);
        ui.painter()
            .rect_stroke(zoom_in_rect, 2.0, Stroke::new(1.0, Color32::GRAY));
        ui.painter().text(
            zoom_in_rect.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::default(),
            Color32::BLACK,
        );

        // Draw zoom out button
        let zoom_out_response = ui.allocate_rect(zoom_out_rect, Sense::click());
        if zoom_out_response.clicked() {
            if let Err(e) = self.zoom_out(None) {
                log::error!("Failed to zoom out: {}", e);
            }
        }

        let zoom_out_color = if zoom_out_response.hovered() {
            Color32::LIGHT_GRAY
        } else {
            Color32::WHITE
        };

        ui.painter().rect_filled(zoom_out_rect, 2.0, zoom_out_color);
        ui.painter()
            .rect_stroke(zoom_out_rect, 2.0, Stroke::new(1.0, Color32::GRAY));
        ui.painter().text(
            zoom_out_rect.center(),
            Align2::CENTER_CENTER,
            "−",
            FontId::default(),
            Color32::BLACK,
        );
    }

    /// Draw attribution text
    fn draw_attribution(&mut self, ui: &mut Ui, rect: Rect) {
        let text_pos = rect.min + Vec2::new(10.0, rect.height() - 20.0);
        ui.painter().text(
            text_pos,
            Align2::LEFT_BOTTOM,
            &self.config.attribution,
            FontId::proportional(10.0),
            Color32::from_rgba_unmultiplied(0, 0, 0, 180),
        );
    }

    /// Handle click events
    fn handle_click(&mut self, position: Point) -> Result<EventHandled> {
        if let Some(viewport) = self.viewport() {
            let lat_lng = viewport.pixel_to_lat_lng(&position);

            if let Some(callback) = &self.on_click {
                callback(lat_lng);
            }

            // Create input event and pass to map
            let input_event = InputEvent::Click { position };
            if let Ok(mut map) = self.map.lock() {
                map.handle_input(input_event)?;
            }

            return Ok(EventHandled::Handled);
        }
        Ok(EventHandled::NotHandled)
    }

    /// Handle double click events
    fn handle_double_click(&mut self, position: Point) -> Result<EventHandled> {
        if let Some(viewport) = self.viewport() {
            let lat_lng = viewport.pixel_to_lat_lng(&position);

            if let Some(callback) = &self.on_double_click {
                callback(lat_lng);
            }

            // Zoom in on double click
            self.zoom_in(Some(position))?;

            return Ok(EventHandled::Handled);
        }
        Ok(EventHandled::NotHandled)
    }

    /// Handle drag events
    fn handle_drag(&mut self, delta: Point) -> Result<EventHandled> {
        if let Ok(mut map) = self.map.lock() {
            // Pan the map
            map.pan(Point::new(-delta.x, -delta.y))?;

            // Trigger viewport changed callback
            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }

            return Ok(EventHandled::Handled);
        }
        Ok(EventHandled::NotHandled)
    }

    /// Handle drag end
    fn handle_drag_end(&mut self) -> Result<EventHandled> {
        // Drag ended, nothing special to do
        Ok(EventHandled::Handled)
    }

    /// Handle scroll events for zooming
    fn handle_scroll(&mut self, delta: f64, position: Point) -> Result<EventHandled> {
        // We treat any wheel event as one discrete step.
        // Positive delta (scroll up) -> zoom in.
        let direction = if delta < 0.0 { 1.0 } else { -1.0 };
        let step = direction * self.config.zoom_delta;

        if let Ok(mut map) = self.map.lock() {
            let current_zoom = map.viewport().zoom;
            let new_zoom = (current_zoom + step).clamp(self.config.min_zoom, self.config.max_zoom);

            // Zoom around the mouse position
            map.zoom_to(new_zoom, Some(position))?;

            if let Some(callback) = &self.on_zoom_changed {
                callback(new_zoom);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }

            return Ok(EventHandled::Handled);
        }
        Ok(EventHandled::NotHandled)
    }

    /// Handle key press events
    fn handle_key_press(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<EventHandled> {
        if let Ok(mut map) = self.map.lock() {
            let pan_distance = 50.0;
            let zoom_step = self.config.zoom_delta;

            match key {
                KeyCode::ArrowUp => {
                    map.pan(Point::new(0.0, -pan_distance))?;
                }
                KeyCode::ArrowDown => {
                    map.pan(Point::new(0.0, pan_distance))?;
                }
                KeyCode::ArrowLeft => {
                    map.pan(Point::new(-pan_distance, 0.0))?;
                }
                KeyCode::ArrowRight => {
                    map.pan(Point::new(pan_distance, 0.0))?;
                }
                KeyCode::Plus => {
                    let new_zoom = (map.viewport().zoom + zoom_step)
                        .clamp(self.config.min_zoom, self.config.max_zoom);
                    map.zoom_to(new_zoom, None)?;
                    if let Some(callback) = &self.on_zoom_changed {
                        callback(new_zoom);
                    }
                }
                KeyCode::Minus => {
                    let new_zoom = (map.viewport().zoom - zoom_step)
                        .clamp(self.config.min_zoom, self.config.max_zoom);
                    map.zoom_to(new_zoom, None)?;
                    if let Some(callback) = &self.on_zoom_changed {
                        callback(new_zoom);
                    }
                }
                _ => return Ok(EventHandled::NotHandled),
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }

            return Ok(EventHandled::Handled);
        }
        Ok(EventHandled::NotHandled)
    }

    /// Zoom in
    pub fn zoom_in(&mut self, focus_point: Option<Point>) -> Result<()> {
        if let Ok(mut map) = self.map.lock() {
            let new_zoom =
                (map.viewport().zoom + self.config.zoom_delta).clamp(self.config.min_zoom, self.config.max_zoom);
            map.zoom_to(new_zoom, focus_point)?;

            if let Some(callback) = &self.on_zoom_changed {
                callback(new_zoom);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }
        }
        Ok(())
    }

    /// Zoom out
    pub fn zoom_out(&mut self, focus_point: Option<Point>) -> Result<()> {
        if let Ok(mut map) = self.map.lock() {
            let new_zoom =
                (map.viewport().zoom - self.config.zoom_delta).clamp(self.config.min_zoom, self.config.max_zoom);
            map.zoom_to(new_zoom, focus_point)?;

            if let Some(callback) = &self.on_zoom_changed {
                callback(new_zoom);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }
        }
        Ok(())
    }

    /// Set the map center
    pub fn set_center(&mut self, center: LatLng) -> Result<()> {
        if let Ok(mut map) = self.map.lock() {
            let current_zoom = map.viewport().zoom;
            map.set_view(center, current_zoom)?;

            if let Some(callback) = &self.on_center_changed {
                callback(center);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }
        }
        Ok(())
    }

    /// Set the map zoom
    pub fn set_zoom(&mut self, zoom: f64) -> Result<()> {
        if let Ok(mut map) = self.map.lock() {
            let center = map.viewport().center;
            map.set_view(center, zoom)?;

            if let Some(callback) = &self.on_zoom_changed {
                callback(zoom);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }
        }
        Ok(())
    }

    /// Set both center and zoom
    pub fn set_view(&mut self, center: LatLng, zoom: f64) -> Result<()> {
        if let Ok(mut map) = self.map.lock() {
            map.set_view(center, zoom)?;

            if let Some(callback) = &self.on_center_changed {
                callback(center);
            }

            if let Some(callback) = &self.on_zoom_changed {
                callback(zoom);
            }

            if let Some(callback) = &self.on_viewport_changed {
                callback(map.viewport());
            }
        }
        Ok(())
    }

    /// Update the widget
    pub fn update(&mut self, delta_time: f64) -> Result<()> {
        // Update transition manager
        self.transition_manager.update(delta_time);

        // Update map
        if let Ok(mut map) = self.map.lock() {
            map.update(delta_time)?;

            // Load tiles for visible tile layers
            let viewport = map.viewport().clone();
            map.for_each_layer_mut(|layer| {
                if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                    let _ = pollster::block_on(tile_layer.update_tiles(&viewport));
                }
            });
        }

        // Determine if another repaint is necessary.
        let mut tiles_loading = false;
        if let Ok(mut map) = self.map.lock() {
            map.for_each_layer_mut(|layer| {
                if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                    if tile_layer.is_loading() {
                        tiles_loading = true;
                    }
                }
            });
        }

        self.needs_repaint = self.drag_state.is_dragging
            || self.transition_manager.has_active_transition()
            || tiles_loading;

        Ok(())
    }

    /// Get access to the map
    pub fn map(&self) -> &Arc<Mutex<Map>> {
        &self.map
    }

    /// Get the transition manager
    pub fn transition_manager(&mut self) -> &mut TransitionManager {
        &mut self.transition_manager
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_dragging
    }

    /// Check if widget has focus
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }
}

/// Extension trait for easier integration with egui
pub trait MapWidgetExt {
    /// Show a map widget
    fn map_widget(&mut self, widget: &mut MapWidget) -> Response;
}

impl MapWidgetExt for Ui {
    fn map_widget(&mut self, widget: &mut MapWidget) -> Response {
        widget.show(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geo::LatLng;

    #[test]
    fn test_map_widget_creation() {
        let map = Map::new(
            LatLng::new(40.7128, -74.0060),
            10.0,
            Point::new(800.0, 600.0),
        );
        let widget = MapWidget::new(map);
        assert_eq!(widget.size, Vec2::new(800.0, 600.0));
        assert!(!widget.is_dragging());
    }

    #[test]
    fn test_cursor_conversion() {
        assert_eq!(egui::CursorIcon::Default, MapCursor::Default.into());
        assert_eq!(egui::CursorIcon::PointingHand, MapCursor::Pointer.into());
        assert_eq!(egui::CursorIcon::Grab, MapCursor::Grab.into());
    }
}
