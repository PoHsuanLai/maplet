use crate::ui::elements::Position;
use crate::ui::style::{MapStyle, ZoomControlStyle};
use crate::Result;
use egui::{Align2, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

/// Shared utilities for UI positioning and layout
pub struct PositionUtils;

impl PositionUtils {
    /// Calculate the actual screen position for a control based on its configuration
    pub fn calculate_control_rect(
        container_rect: Rect,
        position: &Position,
        size: Vec2,
        margin: f32,
    ) -> Rect {
        let pos = match position {
            Position::TopLeft => container_rect.min + Vec2::new(margin, margin),
            Position::TopRight => container_rect.max - Vec2::new(margin + size.x, margin + size.y),
            Position::TopCenter => {
                let x = container_rect.center().x - size.x / 2.0;
                Pos2::new(x, container_rect.min.y + margin)
            }
            Position::BottomLeft => {
                container_rect.min + Vec2::new(margin, container_rect.height() - margin - size.y)
            }
            Position::BottomRight => container_rect.max - Vec2::new(margin + size.x, margin),
            Position::BottomCenter => {
                let x = container_rect.center().x - size.x / 2.0;
                Pos2::new(x, container_rect.max.y - margin - size.y)
            }
            Position::LeftCenter => {
                let y = container_rect.center().y - size.y / 2.0;
                Pos2::new(container_rect.min.x + margin, y)
            }
            Position::RightCenter => {
                let y = container_rect.center().y - size.y / 2.0;
                Pos2::new(container_rect.max.x - margin - size.x, y)
            }
            Position::Custom { x, y } => container_rect.min + Vec2::new(*x, *y),
        };

        Rect::from_min_size(pos, size)
    }

    /// Calculate spacing between multiple controls in the same position
    pub fn calculate_control_offset(index: usize, control_size: Vec2, spacing: f32) -> Vec2 {
        match index {
            0 => Vec2::ZERO,
            _ => Vec2::new(0.0, (control_size.y + spacing) * index as f32),
        }
    }
}

/// Reusable button component with consistent styling
pub struct ButtonComponent;

impl ButtonComponent {
    /// Render a styled button with automatic hover/press states
    pub fn render_button(
        ui: &mut Ui,
        rect: Rect,
        text: &str,
        style: &ZoomControlStyle,
        font_id: Option<FontId>,
    ) -> Response {
        let response = ui.allocate_rect(rect, Sense::click());

        let bg_color = if response.clicked() {
            style.pressed_color
        } else if response.hovered() {
            style.hover_color
        } else {
            style.background_color
        };

        // Draw button background
        ui.painter().rect_filled(rect, style.rounding, bg_color);

        // Draw button border
        ui.painter()
            .rect_stroke(rect, style.rounding, style.border_stroke);

        // Draw button text
        let font = font_id.unwrap_or_default();
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            style.text_color,
        );

        response
    }

    /// Render a simple icon button (like + or -)
    pub fn render_icon_button(
        ui: &mut Ui,
        rect: Rect,
        icon: &str,
        style: &ZoomControlStyle,
    ) -> Response {
        Self::render_button(ui, rect, icon, style, Some(FontId::default()))
    }
}

/// Shared attribution component
pub struct AttributionComponent;

impl AttributionComponent {
    /// Render attribution text with consistent styling
    pub fn render(
        ui: &mut Ui,
        container_rect: Rect,
        text: &str,
        style: &crate::ui::style::AttributionStyle,
    ) {
        let text_pos =
            container_rect.min + Vec2::new(style.margin, container_rect.height() - style.margin);

        ui.painter().text(
            text_pos,
            Align2::LEFT_BOTTOM,
            text,
            style.font_id.clone(),
            style.text_color,
        );
    }
}

/// Integrated control panel that combines multiple controls
pub struct ControlPanel {
    controls: Vec<Box<dyn ControlRenderer>>,
}

impl ControlPanel {
    pub fn new() -> Self {
        Self {
            controls: Vec::new(),
        }
    }

    pub fn add_control(&mut self, control: Box<dyn ControlRenderer>) {
        self.controls.push(control);
    }

    pub fn render(&mut self, ui: &mut Ui, container_rect: Rect, style: &MapStyle) -> Result<()> {
        for control in &mut self.controls {
            control.render(ui, container_rect, style)?;
        }
        Ok(())
    }

    pub fn update_zoom_level(&mut self, zoom: f64) {
        for control in &mut self.controls {
            let any_ref = control.as_any_mut();
            if let Some(zoom_control) = any_ref.downcast_mut::<IntegratedZoomControl>() {
                zoom_control.update_zoom(zoom);
            }
        }
    }
}

/// Trait for renderable controls
pub trait ControlRenderer {
    fn render(&mut self, ui: &mut Ui, container_rect: Rect, style: &MapStyle) -> Result<()>;
    fn get_position(&self) -> &Position;
    fn is_visible(&self) -> bool;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Integrated zoom control that uses the sophisticated configuration system
pub struct IntegratedZoomControl {
    config: crate::ui::controls::ControlConfig,
    current_zoom: f64,
    on_zoom_in: Option<Box<dyn Fn() + Send + Sync>>,
    on_zoom_out: Option<Box<dyn Fn() + Send + Sync>>,
}

impl IntegratedZoomControl {
    pub fn new(config: crate::ui::controls::ControlConfig) -> Self {
        Self {
            config,
            current_zoom: 1.0,
            on_zoom_in: None,
            on_zoom_out: None,
        }
    }

    pub fn with_zoom_callbacks<F1, F2>(mut self, zoom_in: F1, zoom_out: F2) -> Self
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        self.on_zoom_in = Some(Box::new(zoom_in));
        self.on_zoom_out = Some(Box::new(zoom_out));
        self
    }

    pub fn update_zoom(&mut self, zoom: f64) {
        self.current_zoom = zoom;
    }
}

impl ControlRenderer for IntegratedZoomControl {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn render(&mut self, ui: &mut Ui, container_rect: Rect, style: &MapStyle) -> Result<()> {
        if !self.is_visible() {
            return Ok(());
        }

        let button_size = Vec2::new(30.0, 30.0);
        let spacing = 5.0;

        // Calculate positions for zoom in and zoom out buttons
        let zoom_in_rect = PositionUtils::calculate_control_rect(
            container_rect,
            &self.config.position,
            button_size,
            self.config.margin,
        );

        let zoom_out_rect = Rect::from_min_size(
            zoom_in_rect.min + Vec2::new(0.0, button_size.y + spacing),
            button_size,
        );

        // Render zoom in button
        let zoom_in_response =
            ButtonComponent::render_icon_button(ui, zoom_in_rect, "+", &style.zoom_controls);

        if zoom_in_response.clicked() {
            if let Some(ref callback) = self.on_zoom_in {
                callback();
            }
        }

        // Render zoom out button
        let zoom_out_response =
            ButtonComponent::render_icon_button(ui, zoom_out_rect, "−", &style.zoom_controls);

        if zoom_out_response.clicked() {
            if let Some(ref callback) = self.on_zoom_out {
                callback();
            }
        }

        Ok(())
    }

    fn get_position(&self) -> &Position {
        &self.config.position
    }

    fn is_visible(&self) -> bool {
        self.config.visible
    }
}

impl Default for ControlPanel {
    fn default() -> Self {
        Self::new()
    }
}
