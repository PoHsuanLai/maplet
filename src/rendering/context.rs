use crate::{core::geo::Point, Result};
use egui::Color32;

/// Unified style conversion trait to eliminate duplicate conversion patterns
pub trait StyleConversion<T> {
    fn to_render_style(&self, opacity_multiplier: f32) -> T;
}

/// Styles for different rendering primitives
#[derive(Debug, Clone)]
pub struct PointRenderStyle {
    pub fill_color: Color32,
    pub stroke_color: Color32,
    pub stroke_width: f32,
    pub radius: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub struct LineRenderStyle {
    pub color: Color32,
    pub width: f32,
    pub opacity: f32,
    pub dash_pattern: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct PolygonRenderStyle {
    pub fill_color: Color32,
    pub stroke_color: Color32,
    pub stroke_width: f32,
    pub fill_opacity: f32,
    pub stroke_opacity: f32,
}

// Implement unified style conversion for vector layer styles
impl StyleConversion<PointRenderStyle> for crate::layers::vector::PointStyle {
    fn to_render_style(&self, opacity_multiplier: f32) -> PointRenderStyle {
        PointRenderStyle {
            fill_color: self.fill_color.into(),
            stroke_color: self.stroke_color.into(),
            stroke_width: self.stroke_width,
            radius: self.radius,
            opacity: self.opacity * opacity_multiplier,
        }
    }
}

impl StyleConversion<LineRenderStyle> for crate::layers::vector::LineStyle {
    fn to_render_style(&self, opacity_multiplier: f32) -> LineRenderStyle {
        LineRenderStyle {
            color: self.color.into(),
            width: self.width,
            opacity: self.opacity * opacity_multiplier,
            dash_pattern: self.dash_pattern.clone(),
        }
    }
}

impl StyleConversion<PolygonRenderStyle> for crate::layers::vector::PolygonStyle {
    fn to_render_style(&self, opacity_multiplier: f32) -> PolygonRenderStyle {
        PolygonRenderStyle {
            fill_color: self.fill_color.into(),
            stroke_color: self.stroke_color.into(),
            stroke_width: self.stroke_width,
            fill_opacity: self.fill_opacity * opacity_multiplier,
            stroke_opacity: self.stroke_opacity * opacity_multiplier,
        }
    }
}

/// Simplified rendering context for basic functionality
pub struct RenderContext {
    pub width: u32,
    pub height: u32,
    /// Drawing primitives queue (for now just stored, actual rendering would happen elsewhere)
    pub drawing_queue: Vec<DrawCommand>,
    /// Viewport clipping bounds (min, max) in screen coordinates
    pub clip_bounds: Option<(Point, Point)>,
    /// Whether clipping is enabled
    pub clipping_enabled: bool,
}

/// Commands that can be issued to the render context
#[derive(Debug, Clone)]
pub enum DrawCommand {
    Point {
        position: Point,
        style: PointRenderStyle,
    },
    Line {
        points: Vec<Point>,
        style: LineRenderStyle,
    },
    Polygon {
        exterior: Vec<Point>,
        holes: Vec<Vec<Point>>,
        style: PolygonRenderStyle,
    },
    Tile {
        data: Vec<u8>,
        bounds: (Point, Point), // min, max screen coordinates
        opacity: f32,
    },
    /// Tile that already lives in an egui texture atlas
    TileTextured {
        texture_id: egui::TextureId,
        bounds: (Point, Point),
        opacity: f32,
    },
}

impl RenderContext {
    /// Create a new render context
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Ok(Self {
            width,
            height,
            drawing_queue: Vec::new(),
            clip_bounds: None,
            clipping_enabled: false,
        })
    }

    /// Begin a frame
    pub fn begin_frame(&mut self) -> Result<()> {
        self.drawing_queue.clear();
        Ok(())
    }

    /// Render a point at the given position with the given style
    pub fn render_point(&mut self, position: &Point, style: &PointRenderStyle) -> Result<()> {
        self.drawing_queue.push(DrawCommand::Point {
            position: *position,
            style: style.clone(),
        });
        Ok(())
    }

    /// Render a line with the given points and style
    pub fn render_line(&mut self, points: &[Point], style: &LineRenderStyle) -> Result<()> {
        self.drawing_queue.push(DrawCommand::Line {
            points: points.to_vec(),
            style: style.clone(),
        });
        Ok(())
    }

    /// Render a polygon with exterior ring, holes, and style
    pub fn render_polygon(
        &mut self,
        exterior: &[Point],
        holes: &[Vec<Point>],
        style: &PolygonRenderStyle,
    ) -> Result<()> {
        self.drawing_queue.push(DrawCommand::Polygon {
            exterior: exterior.to_vec(),
            holes: holes.to_vec(),
            style: style.clone(),
        });
        Ok(())
    }

    /// Get the current drawing queue
    pub fn get_drawing_queue(&self) -> &[DrawCommand] {
        &self.drawing_queue
    }

    /// Render a tile to the screen with proper error handling and validation
    pub fn render_tile(&mut self, data: &[u8], bounds: (Point, Point), opacity: f32) -> Result<()> {
        // Validate bounds
        if bounds.0.x >= bounds.1.x || bounds.0.y >= bounds.1.y {
            return Err("Invalid tile bounds".into());
        }

        // Validate opacity
        if !(0.0..=1.0).contains(&opacity) {
            return Err("Opacity must be between 0.0 and 1.0".into());
        }

        // Apply clipping if enabled
        let final_bounds = if self.clipping_enabled {
            self.clip_bounds_to_viewport(bounds)
        } else {
            Some(bounds)
        };

        if let Some(clipped_bounds) = final_bounds {
            // For now, just queue the tile for rendering
            self.drawing_queue.push(DrawCommand::Tile {
                data: data.to_vec(),
                bounds: clipped_bounds,
                opacity,
            });
        }
        // If clipped_bounds is None, the tile is completely outside viewport and shouldn't be rendered
        Ok(())
    }

    /// Render a tile that already has a texture registered in egui
    pub fn render_tile_textured(
        &mut self,
        texture_id: egui::TextureId,
        bounds: (Point, Point),
        opacity: f32,
    ) -> Result<()> {
        // Apply clipping if enabled
        let final_bounds = if self.clipping_enabled {
            self.clip_bounds_to_viewport(bounds)
        } else {
            Some(bounds)
        };

        if let Some(clipped_bounds) = final_bounds {
            self.drawing_queue.push(DrawCommand::TileTextured {
                texture_id,
                bounds: clipped_bounds,
                opacity,
            });
        }
        // If clipped_bounds is None, the tile is completely outside viewport and shouldn't be rendered
        Ok(())
    }

    /// Set viewport clipping bounds (like Leaflet's clip rectangle)
    pub fn set_clip_bounds(&mut self, min: Point, max: Point) {
        self.clip_bounds = Some((min, max));
        self.clipping_enabled = true;
    }

    /// Enable or disable clipping
    pub fn set_clipping_enabled(&mut self, enabled: bool) {
        self.clipping_enabled = enabled;
    }

    /// Clear clipping bounds
    pub fn clear_clip_bounds(&mut self) {
        self.clip_bounds = None;
        self.clipping_enabled = false;
    }

    /// Clip bounds to viewport (returns None if completely outside)
    fn clip_bounds_to_viewport(&self, bounds: (Point, Point)) -> Option<(Point, Point)> {
        if let Some((clip_min, clip_max)) = self.clip_bounds {
            let (tile_min, tile_max) = bounds;

            // Check if tile is completely outside clipping area
            if tile_max.x < clip_min.x
                || tile_min.x > clip_max.x
                || tile_max.y < clip_min.y
                || tile_min.y > clip_max.y
            {
                return None; // Completely outside, don't render
            }

            // Clip the bounds to the viewport
            let clipped_min = Point::new(tile_min.x.max(clip_min.x), tile_min.y.max(clip_min.y));
            let clipped_max = Point::new(tile_max.x.min(clip_max.x), tile_max.y.min(clip_max.y));

            Some((clipped_min, clipped_max))
        } else {
            Some(bounds) // No clipping bounds set
        }
    }

    /// Clear the drawing queue
    pub fn clear_queue(&mut self) {
        self.drawing_queue.clear();
    }
}
