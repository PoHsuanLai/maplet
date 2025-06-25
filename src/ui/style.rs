use egui::{Color32, FontId, Stroke};

/// Style configuration for map elements
#[derive(Debug, Clone)]
pub struct MapStyle {
    /// Background color when no tiles are loaded
    pub background_color: Color32,
    /// Color for map borders and outlines
    pub border_color: Color32,
    /// Stroke style for borders
    pub border_stroke: Stroke,
    /// Colors for zoom controls
    pub zoom_controls: ZoomControlStyle,
    /// Style for attribution text
    pub attribution: AttributionStyle,
    /// Style for markers
    pub markers: MarkerStyle,
    /// Style for vector elements
    pub vectors: VectorStyle,
}

/// Style for zoom control buttons
#[derive(Debug, Clone)]
pub struct ZoomControlStyle {
    /// Background color for buttons
    pub background_color: Color32,
    /// Background color when hovered
    pub hover_color: Color32,
    /// Background color when pressed
    pub pressed_color: Color32,
    /// Text color
    pub text_color: Color32,
    /// Border stroke
    pub border_stroke: Stroke,
    /// Button size
    pub button_size: f32,
    /// Margin from edge
    pub margin: f32,
    /// Corner rounding
    pub rounding: f32,
}

/// Style for attribution text
#[derive(Debug, Clone)]
pub struct AttributionStyle {
    /// Text color
    pub text_color: Color32,
    /// Background color (transparent)
    pub background_color: Color32,
    /// Font to use
    pub font_id: FontId,
    /// Padding around text
    pub padding: f32,
    /// Position from bottom-left corner
    pub margin: f32,
}

/// Style for map markers
#[derive(Debug, Clone)]
pub struct MarkerStyle {
    /// Default marker color
    pub default_color: Color32,
    /// Marker size
    pub size: f32,
    /// Border color
    pub border_color: Color32,
    /// Border width
    pub border_width: f32,
    /// Selected marker color
    pub selected_color: Color32,
    /// Hover color
    pub hover_color: Color32,
}

/// Style for vector elements (lines, polygons)
#[derive(Debug, Clone)]
pub struct VectorStyle {
    /// Default stroke color for lines
    pub stroke_color: Color32,
    /// Default stroke width
    pub stroke_width: f32,
    /// Default fill color for polygons
    pub fill_color: Color32,
    /// Selected element stroke color
    pub selected_stroke_color: Color32,
    /// Selected element fill color
    pub selected_fill_color: Color32,
    /// Hover stroke color
    pub hover_stroke_color: Color32,
    /// Hover fill color
    pub hover_fill_color: Color32,
}

impl Default for MapStyle {
    fn default() -> Self {
        Self {
            background_color: Color32::from_rgb(200, 200, 200),
            border_color: Color32::GRAY,
            border_stroke: Stroke::new(1.0, Color32::GRAY),
            zoom_controls: ZoomControlStyle::default(),
            attribution: AttributionStyle::default(),
            markers: MarkerStyle::default(),
            vectors: VectorStyle::default(),
        }
    }
}

impl Default for ZoomControlStyle {
    fn default() -> Self {
        Self {
            background_color: Color32::WHITE,
            hover_color: Color32::LIGHT_GRAY,
            pressed_color: Color32::GRAY,
            text_color: Color32::BLACK,
            border_stroke: Stroke::new(1.0, Color32::GRAY),
            button_size: 30.0,
            margin: 10.0,
            rounding: 2.0,
        }
    }
}

impl Default for AttributionStyle {
    fn default() -> Self {
        Self {
            text_color: Color32::from_rgba_unmultiplied(0, 0, 0, 180),
            background_color: Color32::from_rgba_unmultiplied(255, 255, 255, 100),
            font_id: FontId::proportional(10.0),
            padding: 4.0,
            margin: 10.0,
        }
    }
}

impl Default for MarkerStyle {
    fn default() -> Self {
        Self {
            default_color: Color32::from_rgb(255, 0, 0),
            size: 10.0,
            border_color: Color32::WHITE,
            border_width: 2.0,
            selected_color: Color32::from_rgb(0, 255, 0),
            hover_color: Color32::from_rgb(255, 255, 0),
        }
    }
}

impl Default for VectorStyle {
    fn default() -> Self {
        Self {
            stroke_color: Color32::from_rgb(0, 0, 255),
            stroke_width: 2.0,
            fill_color: Color32::from_rgba_unmultiplied(0, 0, 255, 50),
            selected_stroke_color: Color32::from_rgb(255, 0, 0),
            selected_fill_color: Color32::from_rgba_unmultiplied(255, 0, 0, 50),
            hover_stroke_color: Color32::from_rgb(255, 255, 0),
            hover_fill_color: Color32::from_rgba_unmultiplied(255, 255, 0, 50),
        }
    }
}

/// Predefined themes for different map styles
pub struct MapThemes;

impl MapThemes {
    /// Light theme (default)
    pub fn light() -> MapStyle {
        MapStyle::default()
    }

    /// Dark theme for night mode
    pub fn dark() -> MapStyle {
        MapStyle {
            background_color: Color32::from_rgb(40, 40, 40),
            border_color: Color32::from_rgb(80, 80, 80),
            border_stroke: Stroke::new(1.0, Color32::from_rgb(80, 80, 80)),
            zoom_controls: ZoomControlStyle {
                background_color: Color32::from_rgb(60, 60, 60),
                hover_color: Color32::from_rgb(80, 80, 80),
                pressed_color: Color32::from_rgb(100, 100, 100),
                text_color: Color32::WHITE,
                border_stroke: Stroke::new(1.0, Color32::from_rgb(120, 120, 120)),
                ..ZoomControlStyle::default()
            },
            attribution: AttributionStyle {
                text_color: Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                background_color: Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                ..AttributionStyle::default()
            },
            markers: MarkerStyle {
                default_color: Color32::from_rgb(255, 100, 100),
                border_color: Color32::from_rgb(200, 200, 200),
                selected_color: Color32::from_rgb(100, 255, 100),
                hover_color: Color32::from_rgb(255, 255, 100),
                ..MarkerStyle::default()
            },
            vectors: VectorStyle {
                stroke_color: Color32::from_rgb(100, 100, 255),
                fill_color: Color32::from_rgba_unmultiplied(100, 100, 255, 50),
                selected_stroke_color: Color32::from_rgb(255, 100, 100),
                selected_fill_color: Color32::from_rgba_unmultiplied(255, 100, 100, 50),
                hover_stroke_color: Color32::from_rgb(255, 255, 100),
                hover_fill_color: Color32::from_rgba_unmultiplied(255, 255, 100, 50),
                ..VectorStyle::default()
            },
        }
    }

    /// High contrast theme for accessibility
    pub fn high_contrast() -> MapStyle {
        MapStyle {
            background_color: Color32::WHITE,
            border_color: Color32::BLACK,
            border_stroke: Stroke::new(2.0, Color32::BLACK),
            zoom_controls: ZoomControlStyle {
                background_color: Color32::WHITE,
                hover_color: Color32::LIGHT_GRAY,
                pressed_color: Color32::GRAY,
                text_color: Color32::BLACK,
                border_stroke: Stroke::new(2.0, Color32::BLACK),
                ..ZoomControlStyle::default()
            },
            attribution: AttributionStyle {
                text_color: Color32::BLACK,
                background_color: Color32::WHITE,
                ..AttributionStyle::default()
            },
            markers: MarkerStyle {
                default_color: Color32::RED,
                border_color: Color32::BLACK,
                border_width: 3.0,
                selected_color: Color32::GREEN,
                hover_color: Color32::YELLOW,
                size: 12.0,
            },
            vectors: VectorStyle {
                stroke_color: Color32::BLUE,
                stroke_width: 3.0,
                fill_color: Color32::from_rgba_unmultiplied(0, 0, 255, 100),
                selected_stroke_color: Color32::RED,
                selected_fill_color: Color32::from_rgba_unmultiplied(255, 0, 0, 100),
                hover_stroke_color: Color32::GREEN,
                hover_fill_color: Color32::from_rgba_unmultiplied(0, 255, 0, 100),
            },
        }
    }
}

/// Extension trait for applying styles to egui elements
pub trait StyleExt {
    /// Apply zoom control style to a button response
    fn style_zoom_button(&self, response: &egui::Response, style: &ZoomControlStyle) -> Color32;

    /// Apply attribution style
    fn style_attribution(&self, style: &AttributionStyle) -> (Color32, FontId);
}

impl StyleExt for egui::Ui {
    fn style_zoom_button(&self, response: &egui::Response, style: &ZoomControlStyle) -> Color32 {
        if response.clicked() {
            style.pressed_color
        } else if response.hovered() {
            style.hover_color
        } else {
            style.background_color
        }
    }

    fn style_attribution(&self, style: &AttributionStyle) -> (Color32, FontId) {
        (style.text_color, style.font_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_styles() {
        let style = MapStyle::default();
        assert_eq!(style.background_color, Color32::from_rgb(200, 200, 200));
        assert_eq!(style.zoom_controls.button_size, 30.0);
    }

    #[test]
    fn test_dark_theme() {
        let dark = MapThemes::dark();
        assert_eq!(dark.background_color, Color32::from_rgb(40, 40, 40));
        assert_eq!(dark.zoom_controls.text_color, Color32::WHITE);
    }

    #[test]
    fn test_high_contrast_theme() {
        let high_contrast = MapThemes::high_contrast();
        assert_eq!(high_contrast.background_color, Color32::WHITE);
        assert_eq!(high_contrast.markers.border_width, 3.0);
    }
}
