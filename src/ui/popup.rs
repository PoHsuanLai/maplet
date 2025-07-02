use crate::prelude::HashMap;
use crate::{
    core::{geo::LatLng, viewport::Viewport},
    ui::traits::Renderable,
    Result,
};
use egui::{Color32, FontId, Rect, Response, Ui, Vec2};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum PopupPosition {
    Above,
    Below,
    Left,
    Right,
    Center,
    Auto,
}

#[derive(Debug, Clone)]
pub struct PopupStyle {
    pub background_color: Color32,
    pub border_color: Color32,
    pub border_width: f32,
    pub rounding: f32,
    pub padding: f32,
    pub font_id: FontId,
    pub text_color: Color32,
    pub max_width: f32,
    pub max_height: f32,
}

impl Default for PopupStyle {
    fn default() -> Self {
        Self {
            background_color: Color32::WHITE,
            border_color: Color32::GRAY,
            border_width: 1.0,
            rounding: 4.0,
            padding: 8.0,
            font_id: FontId::proportional(12.0),
            text_color: Color32::BLACK,
            max_width: 300.0,
            max_height: 200.0,
        }
    }
}

pub struct Popup {
    pub id: String,
    pub position: LatLng,
    pub content: String,
    pub visible: bool,
    pub style: PopupStyle,
    pub created_at: Instant,
    pub auto_close_duration: Option<Duration>,
}

impl Popup {
    pub fn new(id: String, position: LatLng, content: String) -> Self {
        Self {
            id,
            position,
            content,
            visible: false,
            style: PopupStyle::default(),
            created_at: Instant::now(),
            auto_close_duration: None,
        }
    }

    pub fn with_auto_close(mut self, duration: Duration) -> Self {
        self.auto_close_duration = Some(duration);
        self
    }

    pub fn with_style(mut self, style: PopupStyle) -> Self {
        self.style = style;
        self
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.created_at = Instant::now();
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn should_auto_close(&self) -> bool {
        if let Some(duration) = self.auto_close_duration {
            self.created_at.elapsed() > duration
        } else {
            false
        }
    }

    pub fn render_at_screen_pos(
        &mut self,
        ui: &mut Ui,
        screen_pos: egui::Pos2,
    ) -> Result<Response> {
        if !self.visible {
            return Ok(ui.allocate_response(Vec2::ZERO, egui::Sense::hover()));
        }

        let text_size = ui
            .fonts(|f| {
                f.layout_no_wrap(
                    self.content.clone(),
                    self.style.font_id.clone(),
                    self.style.text_color,
                )
            })
            .size();

        let popup_size = Vec2::new(
            (text_size.x + self.style.padding * 2.0).min(self.style.max_width),
            (text_size.y + self.style.padding * 2.0).min(self.style.max_height),
        );

        let popup_rect = Rect::from_min_size(screen_pos, popup_size);

        ui.painter()
            .rect_filled(popup_rect, self.style.rounding, self.style.background_color);
        ui.painter().rect_stroke(
            popup_rect,
            self.style.rounding,
            (self.style.border_width, self.style.border_color),
        );

        let text_rect = popup_rect.shrink(self.style.padding);
        ui.painter().text(
            text_rect.min,
            egui::Align2::LEFT_TOP,
            &self.content,
            self.style.font_id.clone(),
            self.style.text_color,
        );

        let response = ui.allocate_rect(popup_rect, egui::Sense::click());
        if response.clicked() {
            self.hide();
        }

        Ok(response)
    }
}

impl Renderable for Popup {
    fn render(&mut self, ui: &mut Ui, _rect: Rect) -> Result<Response> {
        let screen_pos = egui::Pos2::new(100.0, 100.0);
        self.render_at_screen_pos(ui, screen_pos)
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.created_at = Instant::now();
        }
    }
}

pub struct PopupManager {
    popups: HashMap<String, Popup>,
}

impl PopupManager {
    pub fn new() -> Self {
        Self {
            popups: HashMap::default(),
        }
    }

    pub fn add_popup(&mut self, popup: Popup) {
        self.popups.insert(popup.id.clone(), popup);
    }

    pub fn remove_popup(&mut self, id: &str) {
        self.popups.remove(id);
    }

    pub fn show_popup(&mut self, id: &str) {
        if let Some(popup) = self.popups.get_mut(id) {
            popup.show();
        }
    }

    pub fn hide_popup(&mut self, id: &str) {
        if let Some(popup) = self.popups.get_mut(id) {
            popup.hide();
        }
    }

    pub fn show_text_popup(&mut self, id: String, position: LatLng, text: String) {
        let mut popup = Popup::new(id.clone(), position, text);
        popup.show();
        self.popups.insert(id, popup);
    }

    pub fn update(&mut self, _viewport: &Viewport) -> Result<()> {
        let mut to_remove = Vec::new();
        for (id, popup) in &mut self.popups {
            if popup.should_auto_close() {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            self.remove_popup(&id);
        }

        Ok(())
    }

    pub fn render(&mut self, ui: &mut Ui, _rect: Rect, viewport: &Viewport) -> Result<()> {
        for popup in self.popups.values_mut() {
            if popup.visible {
                let screen_pos = viewport.lat_lng_to_pixel(&popup.position);
                let ui_pos = egui::Pos2::new(screen_pos.x as f32, screen_pos.y as f32);
                popup.render_at_screen_pos(ui, ui_pos)?;
            }
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.popups.clear();
    }

    pub fn visible_count(&self) -> usize {
        self.popups.values().filter(|p| p.visible).count()
    }
}

impl Default for PopupManager {
    fn default() -> Self {
        Self::new()
    }
}
