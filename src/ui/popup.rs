use crate::core::geo::{LatLng, Point};
use crate::core::viewport::Viewport;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[cfg(feature = "egui")]
use egui::{Color32, FontId, Pos2, Rect, Vec2};

/// Different types of popups
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PopupType {
    /// Simple text popup
    Text, 
    /// Rich HTML-like content popup
    Rich,
    /// Form popup with inputs
    Form,
    /// Confirmation dialog
    Confirmation,
    /// Information dialog
    Info,
    /// Warning dialog
    Warning,
    /// Error dialog
    Error,
    /// Custom popup with user-defined content
    Custom,
}

/// Popup positioning relative to anchor point
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PopupPosition {
    /// Above the anchor point
    Above,
    /// Below the anchor point
    Below,
    /// To the left of the anchor point
    Left,
    /// To the right of the anchor point
    Right,
    /// Centered on the anchor point
    Center,
    /// Auto-position based on available space
    Auto,
}

/// Animation state for popups
#[derive(Debug, Clone)]
pub enum PopupAnimation {
    /// No animation
    None,
    /// Fade in/out
    Fade { progress: f32 },
    /// Scale in/out
    Scale { progress: f32 },
    /// Slide in/out from a direction
    Slide { direction: Vec2, progress: f32 },
    /// Bounce effect
    Bounce { progress: f32 },
}

/// Style configuration for popups
#[derive(Debug, Clone)]
pub struct PopupStyle {
    /// Background color
    pub background_color: Color32,
    /// Border color
    pub border_color: Color32,
    /// Border width
    pub border_width: f32,
    /// Corner rounding
    pub rounding: f32,
    /// Shadow color
    pub shadow_color: Color32,
    /// Shadow offset
    pub shadow_offset: Vec2,
    /// Padding inside popup
    pub padding: f32,
    /// Font for text
    pub font_id: FontId,
    /// Text color
    pub text_color: Color32,
    /// Maximum width
    pub max_width: f32,
    /// Maximum height
    pub max_height: f32,
    /// Arrow size (for popups with arrows)
    pub arrow_size: f32,
}

impl Default for PopupStyle {
    fn default() -> Self {
        Self {
            background_color: Color32::WHITE,
            border_color: Color32::GRAY,
            border_width: 1.0,
            rounding: 4.0,
            shadow_color: Color32::from_rgba_unmultiplied(0, 0, 0, 50),
            shadow_offset: Vec2::new(2.0, 2.0),
            padding: 8.0,
            font_id: FontId::proportional(12.0),
            text_color: Color32::BLACK,
            max_width: 300.0,
            max_height: 200.0,
            arrow_size: 8.0,
        }
    }
}

/// Content that can be displayed in a popup
#[derive(Debug, Clone)]
pub enum PopupContent {
    /// Simple text content
    Text(String),
    /// Rich content with multiple sections
    Rich {
        title: Option<String>,
        sections: Vec<PopupSection>,
    },
    /// Form with input fields
    Form {
        title: String,
        fields: Vec<FormField>,
        buttons: Vec<PopupButton>,
    },
    /// Simple confirmation dialog
    Confirmation {
        title: String,
        message: String,
        confirm_text: String,
        cancel_text: String,
    },
}

/// A section within rich popup content
#[derive(Debug, Clone)]
pub struct PopupSection {
    pub title: Option<String>,
    pub content: String,
    pub style: Option<PopupStyle>,
}

/// Form field for popup forms
#[derive(Debug, Clone)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub field_type: FormFieldType,
    pub value: String,
    pub required: bool,
    pub placeholder: Option<String>,
}

/// Types of form fields
#[derive(Debug, Clone)]
pub enum FormFieldType {
    Text,
    TextArea,
    Number,
    Email,
    Password,
    Select(Vec<String>),
    Checkbox,
    Radio(Vec<String>),
}

/// Button in a popup
#[derive(Debug, Clone)]
pub struct PopupButton {
    pub id: String,
    pub text: String,
    pub button_type: PopupButtonType,
    pub action: PopupAction,
}

/// Types of popup buttons
#[derive(Debug, Clone, PartialEq)]
pub enum PopupButtonType {
    Primary,
    Secondary,
    Success,
    Warning,
    Danger,
}

/// Actions that can be triggered by popup buttons
#[derive(Debug, Clone)]
pub enum PopupAction {
    Close,
    Submit,
    Cancel,
    Custom(String),
}

/// Events that can be emitted by popups
#[derive(Debug, Clone)]
pub enum PopupEvent {
    Opened,
    Closed,
    ButtonClicked { button_id: String, action: PopupAction },
    FormSubmitted { form_data: std::collections::HashMap<String, String> },
    Dismissed,
}

/// A popup instance
pub struct Popup {
    /// Unique identifier
    pub id: String,
    /// Geographic position anchor
    pub anchor_position: LatLng,
    /// Screen position (computed from anchor)
    pub screen_position: Option<Point>,
    /// Type of popup
    pub popup_type: PopupType,
    /// Position relative to anchor
    pub position: PopupPosition,
    /// Content to display
    pub content: PopupContent,
    /// Style configuration
    pub style: PopupStyle,
    /// Whether the popup is visible
    pub visible: bool,
    /// Whether the popup can be closed by clicking outside
    pub modal: bool,
    /// Animation state
    pub animation: PopupAnimation,
    /// When the popup was created
    pub created_at: Instant,
    /// Duration to auto-close (None for manual close)
    pub auto_close_duration: Option<Duration>,
    /// Whether to show an arrow pointing to the anchor
    pub show_arrow: bool,
    /// Z-index for layering
    pub z_index: i32,
    /// Event callback function
    pub on_event: Option<Box<dyn Fn(PopupEvent) -> Result<()> + Send + Sync>>,
    /// Whether the popup is currently being hovered
    pub hovered: bool,
    /// Current form data (for form popups)
    pub form_data: std::collections::HashMap<String, String>,
}

impl Popup {
    /// Create a new simple text popup
    pub fn new_text(id: String, position: LatLng, text: String) -> Self {
        Self {
            id,
            anchor_position: position,
            screen_position: None,
            popup_type: PopupType::Text,
            position: PopupPosition::Auto,
            content: PopupContent::Text(text),
            style: PopupStyle::default(),
            visible: false,
            modal: false,
            animation: PopupAnimation::Fade { progress: 0.0 },
            created_at: Instant::now(),
            auto_close_duration: None,
            show_arrow: true,
            z_index: 1000,
            on_event: None,
            hovered: false,
            form_data: std::collections::HashMap::new(),
        }
    }

    /// Create a new confirmation popup
    pub fn new_confirmation(
        id: String,
        position: LatLng,
        title: String,
        message: String,
    ) -> Self {
        Self {
            id,
            anchor_position: position,
            screen_position: None,
            popup_type: PopupType::Confirmation,
            position: PopupPosition::Center,
            content: PopupContent::Confirmation {
                title,
                message,
                confirm_text: "OK".to_string(),
                cancel_text: "Cancel".to_string(),
            },
            style: PopupStyle::default(),
            visible: false,
            modal: true,
            animation: PopupAnimation::Scale { progress: 0.0 },
            created_at: Instant::now(),
            auto_close_duration: None,
            show_arrow: false,
            z_index: 2000,
            on_event: None,
            hovered: false,
            form_data: std::collections::HashMap::new(),
        }
    }

    /// Create a new info popup
    pub fn new_info(id: String, position: LatLng, title: String, message: String) -> Self {
        let mut popup = Self::new_confirmation(id, position, title, message);
        popup.popup_type = PopupType::Info;
        popup.content = PopupContent::Rich {
            title: Some(
                if let PopupContent::Confirmation { title, .. } = &popup.content {
                    title.clone()
                } else {
                    "Info".to_string()
                }
            ),
            sections: vec![PopupSection {
                title: None,
                content: if let PopupContent::Confirmation { message, .. } = &popup.content {
                    message.clone()
                } else {
                    "".to_string()
                },
                style: None,
            }],
        };
        popup
    }

    /// Show the popup with optional animation
    pub fn show(&mut self, animate: bool) -> Result<()> {
        self.visible = true;
        if animate {
            match &mut self.animation {
                PopupAnimation::Fade { progress } => *progress = 0.0,
                PopupAnimation::Scale { progress } => *progress = 0.0,
                PopupAnimation::Slide { progress, .. } => *progress = 0.0,
                PopupAnimation::Bounce { progress } => *progress = 0.0,
                _ => {}
            }
        } else {
            self.animation = PopupAnimation::None;
        }

        if let Some(callback) = &self.on_event {
            callback(PopupEvent::Opened)?;
        }
        Ok(())
    }

    /// Hide the popup
    pub fn hide(&mut self) -> Result<()> {
        self.visible = false;
        if let Some(callback) = &self.on_event {
            callback(PopupEvent::Closed)?;
        }
        Ok(())
    }

    /// Toggle popup visibility
    pub fn toggle(&mut self) -> Result<()> {
        if self.visible {
            self.hide()
        } else {
            self.show(true)
        }
    }

    /// Update the popup's screen position based on viewport
    pub fn update_screen_position(&mut self, viewport: &Viewport) {
        // Convert lat/lng to screen coordinates
        let screen_pos = viewport.lat_lng_to_pixel(&self.anchor_position);
        self.screen_position = Some(screen_pos);
    }

    /// Update popup animation
    pub fn update_animation(&mut self, delta_time: f32) {
        match &mut self.animation {
            PopupAnimation::Fade { progress } => {
                *progress = (*progress + delta_time * 4.0).min(1.0);
            }
            PopupAnimation::Scale { progress } => {
                *progress = (*progress + delta_time * 6.0).min(1.0);
            }
            PopupAnimation::Slide { progress, .. } => {
                *progress = (*progress + delta_time * 5.0).min(1.0);
            }
            PopupAnimation::Bounce { progress } => {
                *progress = (*progress + delta_time * 8.0).min(1.0);
            }
            _ => {}
        }
    }

    /// Check if popup should auto-close
    pub fn should_auto_close(&self) -> bool {
        if let Some(duration) = self.auto_close_duration {
            self.created_at.elapsed() >= duration
        } else {
            false
        }
    }

    /// Set auto-close duration
    pub fn with_auto_close(mut self, duration: Duration) -> Self {
        self.auto_close_duration = Some(duration);
        self
    }

    /// Set event callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(PopupEvent) -> Result<()> + Send + Sync + 'static,
    {
        self.on_event = Some(Box::new(callback));
        self
    }

    /// Set position type
    pub fn with_position(mut self, position: PopupPosition) -> Self {
        self.position = position;
        self
    }

    /// Set style
    pub fn with_style(mut self, style: PopupStyle) -> Self {
        self.style = style;
        self
    }

    /// Set modal behavior
    pub fn with_modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Get animation alpha for rendering
    pub fn get_animation_alpha(&self) -> f32 {
        match &self.animation {
            PopupAnimation::Fade { progress } => *progress,
            PopupAnimation::Scale { progress } => *progress,
            PopupAnimation::Slide { progress, .. } => *progress,
            PopupAnimation::Bounce { progress } => {
                let bounce = (progress * std::f32::consts::PI * 2.0).sin().abs();
                *progress * (1.0 + bounce * 0.2)
            }
            PopupAnimation::None => 1.0,
        }
    }

    /// Get animation scale for rendering
    pub fn get_animation_scale(&self) -> f32 {
        match &self.animation {
            PopupAnimation::Scale { progress } => {
                let scale = 0.3 + *progress * 0.7;
                scale
            }
            PopupAnimation::Bounce { progress } => {
                let bounce = (progress * std::f32::consts::PI * 4.0).sin();
                1.0 + bounce * 0.1 * (1.0 - progress)
            }
            _ => 1.0,
        }
    }
}

/// Manager for handling multiple popups
pub struct PopupManager {
    /// All active popups
    popups: std::collections::HashMap<String, Popup>,
    /// Popup display order (z-index sorting)
    display_order: Vec<String>,
    /// Next available z-index
    next_z_index: i32,
}

impl PopupManager {
    /// Create a new popup manager
    pub fn new() -> Self {
        Self {
            popups: std::collections::HashMap::new(),
            display_order: Vec::new(),
            next_z_index: 1000,
        }
    }

    /// Add a popup to the manager
    pub fn add_popup(&mut self, mut popup: Popup) -> Result<()> {
        popup.z_index = self.next_z_index;
        self.next_z_index += 1;

        let id = popup.id.clone();
        self.popups.insert(id.clone(), popup);
        self.display_order.push(id);
        self.sort_display_order();
        Ok(())
    }

    /// Remove a popup by ID
    pub fn remove_popup(&mut self, id: &str) -> Result<()> {
        if let Some(mut popup) = self.popups.remove(id) {
            popup.hide()?;
            self.display_order.retain(|popup_id| popup_id != id);
        }
        Ok(())
    }

    /// Get a popup by ID
    pub fn get_popup(&self, id: &str) -> Option<&Popup> {
        self.popups.get(id)
    }

    /// Get a mutable popup by ID
    pub fn get_popup_mut(&mut self, id: &str) -> Option<&mut Popup> {
        self.popups.get_mut(id)
    }

    /// Update all popups
    pub fn update(&mut self, viewport: &Viewport, delta_time: f32) -> Result<()> {
        let mut popups_to_remove = Vec::new();

        for popup in self.popups.values_mut() {
            popup.update_screen_position(viewport);
            popup.update_animation(delta_time);

            if popup.should_auto_close() {
                popups_to_remove.push(popup.id.clone());
            }
        }

        for id in popups_to_remove {
            self.remove_popup(&id)?;
        }

        Ok(())
    }

    /// Get all visible popups in display order
    pub fn get_visible_popups(&self) -> Vec<&Popup> {
        self.display_order
            .iter()
            .filter_map(|id| self.popups.get(id))
            .filter(|popup| popup.visible)
            .collect()
    }

    /// Clear all popups
    pub fn clear(&mut self) -> Result<()> {
        for popup in self.popups.values_mut() {
            popup.hide()?;
        }
        self.popups.clear();
        self.display_order.clear();
        Ok(())
    }

    /// Show a simple text popup
    pub fn show_text_popup(
        &mut self,
        id: String,
        position: LatLng,
        text: String,
    ) -> Result<()> {
        let mut popup = Popup::new_text(id, position, text);
        popup.show(true)?;
        self.add_popup(popup)
    }

    /// Show a confirmation dialog
    pub fn show_confirmation(
        &mut self,
        id: String,
        position: LatLng,
        title: String,
        message: String,
        callback: impl Fn(PopupEvent) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        let mut popup = Popup::new_confirmation(id, position, title, message)
            .with_callback(callback);
        popup.show(true)?;
        self.add_popup(popup)
    }

    /// Show an info popup
    pub fn show_info(
        &mut self,
        id: String,
        position: LatLng,
        title: String,
        message: String,
    ) -> Result<()> {
        let mut popup = Popup::new_info(id, position, title, message)
            .with_auto_close(Duration::from_secs(3));
        popup.show(true)?;
        self.add_popup(popup)
    }

    fn sort_display_order(&mut self) {
        self.display_order.sort_by(|a, b| {
            let z_a = self.popups.get(a).map(|p| p.z_index).unwrap_or(0);
            let z_b = self.popups.get(b).map(|p| p.z_index).unwrap_or(0);
            z_a.cmp(&z_b)
        });
    }
}

impl Default for PopupManager {
    fn default() -> Self {
        Self::new()
    }
}
