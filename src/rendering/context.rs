use crate::{core::geo::Point, Result};
use egui::Color32;
use image;

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

/// Simplified rendering context for basic functionality
pub struct RenderContext {
    pub width: u32,
    pub height: u32,
    /// Drawing primitives queue (for now just stored, actual rendering would happen elsewhere)
    pub drawing_queue: Vec<DrawCommand>,
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
    pub fn render_tile(
        &mut self,
        tile_data: &[u8],
        screen_bounds: (Point, Point),
        opacity: f32,
    ) -> Result<()> {
        // Validate tile data before processing
        if tile_data.is_empty() {
            log::warn!("Empty tile data, skipping render");
            return Ok(());
        }
        
        // Check for minimum reasonable tile size (100 bytes for a tiny image)
        if tile_data.len() < 100 {
            log::warn!("Suspiciously small tile data ({} bytes), skipping render", tile_data.len());
            return Ok(());
        }

        // Validate image format by checking headers
        if !self.is_valid_image_format(tile_data) {
            log::warn!("Invalid or corrupted image format, skipping render");
            return Ok(());
        }

        // Add tile to drawing queue directly - validation is sufficient
        // The actual image decoding will happen in the UI thread
        self.drawing_queue.push(DrawCommand::Tile {
            data: tile_data.to_vec(),
            bounds: screen_bounds,
            opacity,
        });

        Ok(())
    }
    
    /// Validate image format by checking magic bytes
    fn is_valid_image_format(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // Check for PNG signature
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return true;
        }
        
        // Check for JPEG signature
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return true;
        }
        
        // Check for WebP signature
        if data.len() >= 12 && data[0..4] == [0x52, 0x49, 0x46, 0x46] && data[8..12] == [0x57, 0x45, 0x42, 0x50] {
            return true;
        }
        
        // Check for GIF signature
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return true;
        }
        
        false
    }
    
    /// Render a placeholder when tile data is corrupted
    fn render_placeholder_tile(
        &mut self,
        screen_bounds: (Point, Point),
        opacity: f32,
    ) -> Result<()> {
        // Create a simple placeholder (empty for now, could be a colored rectangle)
        log::trace!("Rendering placeholder tile at bounds {:?} with opacity {}", screen_bounds, opacity);
        Ok(())
    }

    /// Render a tile that already has a texture registered in egui
    pub fn render_tile_textured(
        &mut self,
        texture_id: egui::TextureId,
        bounds: (Point, Point),
        opacity: f32,
    ) -> Result<()> {
        self.drawing_queue.push(DrawCommand::TileTextured {
            texture_id,
            bounds,
            opacity,
        });
        Ok(())
    }

    /// Clear the drawing queue
    pub fn clear_queue(&mut self) {
        self.drawing_queue.clear();
    }
}
