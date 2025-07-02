use crate::core::map::Map;
use crate::{
    core::viewport::Viewport,
    ui::{
        elements::{Attribution, Position, ZoomControl},
        style::MapStyle,
        traits::Renderable,
    },
    Result,
};
use egui::{Rect, Ui};
use std::sync::{Arc, Mutex};

/// Simplified control configuration
#[derive(Debug, Clone)]
pub struct ControlConfig {
    pub visible: bool,
    pub position: Position,
    pub margin: f32,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            visible: true,
            position: Position::TopRight,
            margin: 10.0,
        }
    }
}

/// Control manager that handles all map controls
pub struct ControlManager {
    zoom_control: Option<ZoomControl>,
    attribution: Option<Attribution>,
    map_ref: Option<Arc<Mutex<Map>>>,
}

impl ControlManager {
    pub fn new() -> Self {
        Self {
            zoom_control: None,
            attribution: None,
            map_ref: None,
        }
    }

    /// Add a zoom control
    pub fn with_zoom_control(mut self, control: ZoomControl) -> Self {
        self.zoom_control = Some(control);
        self
    }

    /// Add attribution
    pub fn with_attribution(mut self, attribution: Attribution) -> Self {
        self.attribution = Some(attribution);
        self
    }

    /// Set map reference for controls that need it
    pub fn set_map_ref(&mut self, map: Arc<Mutex<Map>>) {
        self.map_ref = Some(map);
    }

    /// Update all controls with viewport changes
    pub fn update_viewport(&mut self, viewport: &Viewport) -> Result<()> {
        if let Some(zoom_control) = &mut self.zoom_control {
            zoom_control.update_zoom(viewport.zoom);
        }
        Ok(())
    }

    /// Render all controls
    pub fn render(&mut self, ui: &mut Ui, rect: Rect, _style: &MapStyle) -> Result<()> {
        if let Some(zoom_control) = &mut self.zoom_control {
            if zoom_control.is_visible() {
                zoom_control.render(ui, rect)?;
            }
        }

        if let Some(attribution) = &mut self.attribution {
            if attribution.is_visible() {
                attribution.render(ui, rect)?;
            }
        }

        Ok(())
    }

    /// Check if any control is visible
    pub fn has_visible_controls(&self) -> bool {
        self.zoom_control.as_ref().is_some_and(|c| c.is_visible())
            || self.attribution.as_ref().is_some_and(|c| c.is_visible())
    }

    /// Set visibility of all controls
    pub fn set_all_visible(&mut self, visible: bool) {
        if let Some(zoom_control) = &mut self.zoom_control {
            zoom_control.set_visible(visible);
        }
        if let Some(attribution) = &mut self.attribution {
            attribution.set_visible(visible);
        }
    }
}

impl Default for ControlManager {
    fn default() -> Self {
        Self::new()
    }
}
