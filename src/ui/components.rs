use crate::ui::elements::{Attribution, ZoomControl};
use crate::ui::style::MapStyle;
use crate::Result;
use egui::{Rect, Ui};

/// Integrated control panel that combines multiple controls
pub struct ControlPanel {
    zoom_control: Option<ZoomControl>,
    attribution: Option<Attribution>,
}

impl ControlPanel {
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

    pub fn render(&mut self, ui: &mut Ui, container_rect: Rect, style: &MapStyle) -> Result<()> {
        if let Some(ref mut zoom_control) = self.zoom_control {
            zoom_control.render_with_style(ui, container_rect, &style.zoom_controls)?;
        }

        if let Some(ref mut attribution) = self.attribution {
            attribution.render_with_style(ui, container_rect, &style.attribution);
        }

        Ok(())
    }

    pub fn update_zoom_level(&mut self, zoom: f64) {
        if let Some(ref mut zoom_control) = self.zoom_control {
            zoom_control.update_zoom(zoom);
        }
    }
}

impl Default for ControlPanel {
    fn default() -> Self {
        Self::new()
    }
}
