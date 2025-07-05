use crate::{
    ui::{
        style::{AttributionStyle, MapStyle, ZoomControlStyle},
        traits::{Positionable, Renderable},
    },
    Result,
};
use egui::{Align2, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

/// Simple position type for UI elements
#[derive(Debug, Clone, PartialEq)]
pub enum Position {
    TopLeft,
    TopRight,
    TopCenter,
    BottomLeft,
    BottomRight,
    BottomCenter,
    LeftCenter,
    RightCenter,
    Custom { x: f32, y: f32 },
}

impl Position {
    pub fn calculate_rect(&self, container: Rect, size: Vec2, margin: f32) -> Rect {
        let pos = match self {
            Position::TopLeft => container.min + Vec2::new(margin, margin),
            Position::TopRight => container.max - Vec2::new(margin + size.x, margin + size.y),
            Position::TopCenter => {
                let x = container.center().x - size.x / 2.0;
                Pos2::new(x, container.min.y + margin)
            }
            Position::BottomLeft => {
                container.min + Vec2::new(margin, container.height() - margin - size.y)
            }
            Position::BottomRight => container.max - Vec2::new(margin + size.x, margin),
            Position::BottomCenter => {
                let x = container.center().x - size.x / 2.0;
                Pos2::new(x, container.max.y - margin - size.y)
            }
            Position::LeftCenter => {
                let y = container.center().y - size.y / 2.0;
                Pos2::new(container.min.x + margin, y)
            }
            Position::RightCenter => {
                let y = container.center().y - size.y / 2.0;
                Pos2::new(container.max.x - margin - size.x, y)
            }
            Position::Custom { x, y } => container.min + Vec2::new(*x, *y),
        };
        Rect::from_min_size(pos, size)
    }
}

/// Simple button component - replaces all the duplicated button logic
pub struct Button {
    text: String,
    position: Position,
    size: Vec2,
    margin: f32,
    visible: bool,
    on_click: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Button {
    pub fn new(text: String, position: Position) -> Self {
        Self {
            text,
            position,
            size: Vec2::new(30.0, 30.0),
            margin: 10.0,
            visible: true,
            on_click: None,
        }
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_click = Some(Box::new(callback));
        self
    }

    pub fn render_with_style(
        &mut self,
        ui: &mut Ui,
        container: Rect,
        style: &ZoomControlStyle,
    ) -> Response {
        if !self.visible {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }

        let rect = self
            .position
            .calculate_rect(container, self.size, self.margin);
        let response = ui.allocate_rect(rect, Sense::click());

        if response.clicked() {
            if let Some(ref callback) = self.on_click {
                callback();
            }
        }

        let bg_color = if response.clicked() {
            style.pressed_color
        } else if response.hovered() {
            style.hover_color
        } else {
            style.background_color
        };

        ui.painter().rect_filled(rect, style.rounding, bg_color);
        ui.painter()
            .rect_stroke(rect, style.rounding, style.border_stroke);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            &self.text,
            FontId::default(),
            style.text_color,
        );

        response
    }
}

impl Renderable for Button {
    fn render(&mut self, ui: &mut Ui, rect: Rect) -> Result<Response> {
        // Use default style if no specific style provided
        let default_style = ZoomControlStyle::default();
        Ok(self.render_with_style(ui, rect, &default_style))
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Positionable for Button {
    type Position = Position;

    fn get_position(&self) -> &Self::Position {
        &self.position
    }

    fn set_position(&mut self, position: Self::Position) {
        self.position = position;
    }

    fn calculate_rect(&self, container_rect: Rect, size: Vec2) -> Rect {
        self.position
            .calculate_rect(container_rect, size, self.margin)
    }
}

/// Zoom control component - much simpler than before
pub struct ZoomControl {
    zoom_in_btn: Button,
    zoom_out_btn: Button,
    show_level: bool,
    current_zoom: f64,
    visible: bool,
}

impl ZoomControl {
    pub fn new(position: Position) -> Self {
        let zoom_in_btn = Button::new("+".to_string(), position.clone());
        let mut zoom_out_pos = position;
        // Offset zoom out button below zoom in
        if let Position::Custom { x: _, y } = &mut zoom_out_pos {
            *y += 35.0;
        }
        let zoom_out_btn = Button::new("−".to_string(), zoom_out_pos);

        Self {
            zoom_in_btn,
            zoom_out_btn,
            show_level: false,
            current_zoom: 1.0,
            visible: true,
        }
    }

    pub fn with_callbacks<F1, F2>(mut self, zoom_in: F1, zoom_out: F2) -> Self
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        self.zoom_in_btn = self.zoom_in_btn.with_callback(zoom_in);
        self.zoom_out_btn = self.zoom_out_btn.with_callback(zoom_out);
        self
    }

    pub fn update_zoom(&mut self, zoom: f64) {
        self.current_zoom = zoom;
    }

    pub fn render_with_style(
        &mut self,
        ui: &mut Ui,
        container: Rect,
        style: &ZoomControlStyle,
    ) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        self.zoom_in_btn.render_with_style(ui, container, style);

        // Calculate zoom out position relative to zoom in
        let zoom_in_rect = self.zoom_in_btn.position.calculate_rect(
            container,
            self.zoom_in_btn.size,
            self.zoom_in_btn.margin,
        );
        let zoom_out_rect = Rect::from_min_size(
            zoom_in_rect.min + Vec2::new(0.0, zoom_in_rect.height() + 5.0),
            self.zoom_out_btn.size,
        );

        let response = ui.allocate_rect(zoom_out_rect, Sense::click());
        if response.clicked() {
            if let Some(ref callback) = self.zoom_out_btn.on_click {
                callback();
            }
        }

        let bg_color = if response.clicked() {
            style.pressed_color
        } else if response.hovered() {
            style.hover_color
        } else {
            style.background_color
        };

        ui.painter()
            .rect_filled(zoom_out_rect, style.rounding, bg_color);
        ui.painter()
            .rect_stroke(zoom_out_rect, style.rounding, style.border_stroke);
        ui.painter().text(
            zoom_out_rect.center(),
            Align2::CENTER_CENTER,
            &self.zoom_out_btn.text,
            FontId::default(),
            style.text_color,
        );

        // Show zoom level if enabled
        if self.show_level {
            let zoom_text_rect = Rect::from_min_size(
                zoom_out_rect.min + Vec2::new(0.0, zoom_out_rect.height() + 5.0),
                Vec2::new(zoom_out_rect.width(), 20.0),
            );
            ui.painter().text(
                zoom_text_rect.center(),
                Align2::CENTER_CENTER,
                format!("{:.1}", self.current_zoom),
                FontId::proportional(10.0),
                style.text_color,
            );
        }

        Ok(())
    }
}

impl Renderable for ZoomControl {
    fn render(&mut self, ui: &mut Ui, rect: Rect) -> Result<Response> {
        let default_style = ZoomControlStyle::default();
        self.render_with_style(ui, rect, &default_style)?;
        Ok(ui.allocate_response(Vec2::ZERO, Sense::hover()))
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

pub struct Attribution {
    text: String,
    visible: bool,
}

impl Attribution {
    pub fn new(text: String) -> Self {
        Self {
            text,
            visible: true,
        }
    }

    pub fn render_with_style(&mut self, ui: &mut Ui, container: Rect, style: &AttributionStyle) {
        if !self.visible {
            return;
        }

        let text_pos = container.min + Vec2::new(style.margin, container.height() - style.margin);
        ui.painter().text(
            text_pos,
            Align2::LEFT_BOTTOM,
            &self.text,
            style.font_id.clone(),
            style.text_color,
        );
    }
}

impl Renderable for Attribution {
    fn render(&mut self, ui: &mut Ui, rect: Rect) -> Result<Response> {
        let default_style = AttributionStyle::default();
        self.render_with_style(ui, rect, &default_style);
        Ok(ui.allocate_response(Vec2::ZERO, Sense::hover()))
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

/// Simple UI manager - replaces the complex control panel
pub struct UiManager {
    zoom_control: Option<ZoomControl>,
    attribution: Option<Attribution>,
}

impl UiManager {
    pub fn new() -> Self {
        Self {
            zoom_control: None,
            attribution: None,
        }
    }

    pub fn with_zoom_control(mut self, zoom_control: ZoomControl) -> Self {
        self.zoom_control = Some(zoom_control);
        self
    }

    pub fn with_attribution(mut self, attribution: Attribution) -> Self {
        self.attribution = Some(attribution);
        self
    }

    pub fn update_zoom(&mut self, zoom: f64) {
        if let Some(ref mut zoom_control) = self.zoom_control {
            zoom_control.update_zoom(zoom);
        }
    }

    pub fn render(&mut self, ui: &mut Ui, container: Rect, style: &MapStyle) -> Result<()> {
        if let Some(ref mut zoom_control) = self.zoom_control {
            zoom_control.render_with_style(ui, container, &style.zoom_controls)?;
        }

        if let Some(ref mut attribution) = self.attribution {
            attribution.render_with_style(ui, container, &style.attribution);
        }

        Ok(())
    }
}

impl Default for UiManager {
    fn default() -> Self {
        Self::new()
    }
}
