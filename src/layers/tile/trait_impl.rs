//! LayerTrait implementation for TileLayer

use super::{TileLayer, TileLayerOptions};
use crate::{
    core::{geo::LatLngBounds, viewport::Viewport},
    layers::base::{LayerTrait, LayerType},
    rendering::context::RenderContext,
    Result,
};

impl LayerTrait for TileLayer {
    crate::impl_layer_trait!(TileLayer, properties);

    fn bounds(&self) -> Option<LatLngBounds> {
        self.options.bounds.clone()
    }

    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        if !self.is_visible() {
            return Ok(());
        }

        self.process_tile_results()?;

        self.update_tiles(viewport)?;

        // Check for zoom animation state and handle level transitions
        if let Some(ref mut animation_manager) = self.animation_manager {
            if let Some(animation_state) = animation_manager.update() {
                // Create animated viewport with proper transform
                let mut animated_viewport = viewport.clone();
                animated_viewport.set_transform(animation_state.transform);
                
                // Apply animation state to viewport center and zoom for tile calculations
                animated_viewport.center = animation_state.center;
                animated_viewport.zoom = animation_state.zoom;
                
                return self.render_tiles(context, &animated_viewport);
            }
        }

        // Also check if viewport has an active transform (set from map level)
        if viewport.has_active_transform() {
            // Use the viewport directly with its transform
            return self.render_tiles(context, viewport);
        }

        self.render_tiles(context, viewport)
    }

    fn update(&mut self, _delta_time: f64) -> Result<()> {
        self.process_tile_results()?;

        self.handle_tile_retries()?;

        if let Some(ref mut animation_manager) = self.animation_manager {
            animation_manager.update();
        }

        for level in self.levels.values_mut() {
            for tile in level.tiles.values_mut() {
                if tile.is_loaded() {
                    tile.opacity = 1.0;
                }
            }
        }

        if self
            .levels
            .values()
            .flat_map(|level| level.tiles.values())
            .filter(|tile| tile.error.is_some())
            .count()
            > 100
        {
            for level in self.levels.values_mut() {
                level.tiles.retain(|_, tile| tile.error.is_none());
            }
        }

        Ok(())
    }

    fn options(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn set_options(&mut self, options: serde_json::Value) -> Result<()> {
        if let Ok(tile_options) = serde_json::from_value::<TileLayerOptions>(options) {
            self.set_tile_options(tile_options);
        }
        Ok(())
    }
}

