//! Leaflet-style tile management methods
//! 
//! This module contains methods that replicate Leaflet's tile loading,
//! retention, and pruning strategies for maximum compatibility.

use super::{TileLayer, TileState, TileLevel, loader::TilePriority};
use crate::{
    core::{geo::{LatLng, Point, TileCoord}, viewport::Viewport},
    layers::animation::ZoomAnimationState,
    Result,
};

#[cfg(feature = "debug")]
use log;

impl TileLayer {
    /// Main update method that consolidates all tile operations
    /// This is the unified entry point for all tile loading, similar to Leaflet's _update method
    pub fn update_tiles(&mut self, viewport: &Viewport) -> Result<()> {
        let now = std::time::Instant::now();
        let time_since_last_update = now.duration_since(self.last_update_time).as_millis() as u64;
        
        // Throttle updates unless pending or in test mode
        if !self.test_mode && time_since_last_update < self.update_interval_ms && !self.pending_update {
            return Ok(());
        }
        
        self.last_update_time = now;
        self.pending_update = false;
        
        let target_zoom = viewport.zoom.floor() as u8;
        let clamped_zoom = target_zoom.clamp(self.options.min_zoom, self.options.max_zoom);

        // Test mode - minimal tile setup
        if self.test_mode {
            self.levels
                .entry(clamped_zoom)
                .or_insert_with(|| TileLevel::new(clamped_zoom));
            return Ok(());
        }

        // Process any completed tile results first
        self.process_tile_results()?;

        // Handle zoom animation updates (CSS-style transforms)
        if let Some(ref mut animation_manager) = self.animation_manager {
            if let Some(animation_state) = animation_manager.update() {
                self.apply_zoom_animation_transforms(animation_state, viewport);
            }
        }

        // Calculate current view requirements
        let tiled_pixel_bounds = self.get_tiled_pixel_bounds(viewport, clamped_zoom);
        let tile_range = self.pixel_bounds_to_tile_range(&tiled_pixel_bounds, clamped_zoom);
        let visible_tiles = self.tile_range_to_coords(&tile_range, clamped_zoom);

        // Expanded range for prefetching (Leaflet's keepBuffer)
        let buffered_range = self.expand_tile_range(&tile_range, self.keep_buffer as i32);
        let prefetch_tiles = self.tile_range_to_coords(&buffered_range, clamped_zoom);

        // Ensure level exists for current zoom
        self.levels
            .entry(clamped_zoom)
            .or_insert_with(|| TileLevel::new(clamped_zoom));

        // Mark tiles for retention within buffer area (Leaflet's noPruneRange)
        self.mark_tiles_for_retention(&buffered_range, clamped_zoom);

        // Load visible tiles with highest priority
        self.load_tiles_batch(&visible_tiles, TilePriority::Visible, clamped_zoom)?;

        // Load prefetch tiles (excluding already visible ones)
        let prefetch_only: Vec<_> = prefetch_tiles
            .into_iter()
            .filter(|coord| !visible_tiles.contains(coord))
            .collect();
        self.load_tiles_batch(&prefetch_only, TilePriority::Prefetch, clamped_zoom)?;

        // Unified parent/child tile retention (consolidates setup_parent_tile_data logic)
        self.retain_parent_and_child_tiles(clamped_zoom)?;

        // Clean up old tiles
        self.prune_tiles(clamped_zoom);

        // Update loading state
        self.tile_zoom = Some(clamped_zoom);
        let current_loading_count = self.count_loading_tiles();
        self.loading_state_changed = current_loading_count != self.last_loading_count;
        self.last_loading_count = current_loading_count;
        self.tiles_loading_count = current_loading_count;
        self.loading = current_loading_count > 0;

        Ok(())
    }

    /// Apply CSS-style zoom animation transforms to tile levels
    /// This mimics Leaflet's _setZoomTransforms method
    fn apply_zoom_animation_transforms(&mut self, animation_state: ZoomAnimationState, _viewport: &Viewport) {
        let scale = animation_state.transform.scale;
        let translate = animation_state.transform.translate;
        
        // Apply transforms to all levels
        for (zoom_level, level) in &mut self.levels {
            if level.tiles.is_empty() {
                continue;
            }
            
            level.animating = true;
            
            // Calculate level-specific transform based on zoom difference
            let zoom_diff = animation_state.zoom - *zoom_level as f64;
            let level_scale = 2_f64.powf(zoom_diff) * scale;
            
            // Apply transform origin and translation
            let origin = animation_state.transform.origin;
            let scaled_translate = Point::new(
                translate.x + origin.x * (level_scale - 1.0),
                translate.y + origin.y * (level_scale - 1.0),
            );
            
            level.scale = level_scale;
            level.translation = scaled_translate;
            
            #[cfg(feature = "debug")]
            log::debug!(
                "Applied zoom animation to level {}: scale={:.3}, translate=({:.1}, {:.1})",
                zoom_level, level_scale, scaled_translate.x, scaled_translate.y
            );
        }
    }

    /// Get tiled pixel bounds for a specific zoom level
    /// This matches Leaflet's _getTiledPixelBounds method
    pub fn get_tiled_pixel_bounds(&self, viewport: &Viewport, zoom: u8) -> (Point, Point) {
        let map_zoom = viewport.zoom;
        let scale = 2_f64.powf(map_zoom - zoom as f64);
        let pixel_center = viewport.project(&viewport.center, Some(zoom as f64));
        let half_size = Point::new(viewport.size.x / (scale * 2.0), viewport.size.y / (scale * 2.0));
        
        (
            pixel_center.subtract(&half_size),
            pixel_center.add(&half_size),
        )
    }

    /// Convert pixel bounds to tile coordinate range
    /// This matches Leaflet's _pxBoundsToTileRange method
    pub fn pixel_bounds_to_tile_range(&self, bounds: &(Point, Point), _zoom: u8) -> (Point, Point) {
        let tile_size = self.options.tile_size as f64;
        let min = Point::new(
            (bounds.0.x / tile_size).floor(),
            (bounds.0.y / tile_size).floor(),
        );
        let max = Point::new(
            (bounds.1.x / tile_size).ceil(),
            (bounds.1.y / tile_size).ceil(),
        );
        (min, max)
    }

    /// Convert tile range to coordinate list with boundary checking
    /// Enhanced with improved boundary validation
    pub fn tile_range_to_coords(&self, range: &(Point, Point), zoom: u8) -> Vec<TileCoord> {
        let mut coords = Vec::new();
        let max_coord = (1u32 << zoom) as i32;

        for y in (range.0.y as i32)..=(range.1.y as i32) {
            for x in (range.0.x as i32)..=(range.1.x as i32) {
                // Validate Y coordinate
                if y < 0 || y >= max_coord {
                    continue;
                }
                
                // Wrap X coordinate for spherical mercator
                let wrapped_x = ((x % max_coord) + max_coord) % max_coord;
                
                let coord = TileCoord {
                    x: wrapped_x as u32,
                    y: y as u32,
                    z: zoom,
                };

                // Enhanced boundary checking
                if self.is_valid_tile(&coord) && self.is_tile_within_boundary(&coord) {
                    coords.push(coord);
                }
            }
        }

        coords
    }

    /// Expand tile range by buffer amount (Leaflet's keepBuffer)
    fn expand_tile_range(&self, range: &(Point, Point), buffer: i32) -> (Point, Point) {
        (
            Point::new(range.0.x - buffer as f64, range.0.y - buffer as f64),
            Point::new(range.1.x + buffer as f64, range.1.y + buffer as f64),
        )
    }

    /// Mark tiles for retention within buffer area
    /// This prevents tiles from being pruned during panning
    fn mark_tiles_for_retention(&mut self, buffer_range: &(Point, Point), zoom: u8) {
        if let Some(level) = self.levels.get_mut(&zoom) {
            for (coord, tile) in &mut level.tiles {
                let in_buffer = coord.x as f64 >= buffer_range.0.x
                    && coord.x as f64 <= buffer_range.1.x
                    && coord.y as f64 >= buffer_range.0.y
                    && coord.y as f64 <= buffer_range.1.y;
                
                tile.current = in_buffer;
                if in_buffer {
                    tile.retain = true;
                }
            }
        }
    }

    /// Load a batch of tiles with the specified priority
    fn load_tiles_batch(&self, coords: &[TileCoord], priority: TilePriority, zoom: u8) -> Result<()> {
        if coords.is_empty() {
            return Ok(());
        }

        // Filter out tiles that are already loaded or loading
        let tiles_to_load: Vec<_> = coords
            .iter()
            .filter(|coord| {
                // Check cache first
                if self.tile_cache.get(coord).is_some() {
                    return false;
                }
                
                // Check if already loaded or loading
                if let Some(level) = self.levels.get(&zoom) {
                    if let Some(tile_state) = level.tiles.get(coord) {
                        return !tile_state.is_loaded() && !tile_state.loading;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if !tiles_to_load.is_empty() {
            self.tile_loader.queue_tiles_batch(
                self.tile_source.as_ref(),
                tiles_to_load,
                priority,
            )?;
        }

        Ok(())
    }

    /// Unified parent and child tile retention
    /// This consolidates and replaces setup_parent_tile_data and preload_next_zoom_level
    fn retain_parent_and_child_tiles(&mut self, current_zoom: u8) -> Result<()> {
        // Retain parent tiles for smoother zoom-out experience
        if current_zoom > self.options.min_zoom {
            self.retain_parent_tiles(current_zoom)?;
        }

        // Retain child tiles for smoother zoom-in experience
        if current_zoom < self.options.max_zoom {
            self.retain_child_tiles(current_zoom)?;
        }

        Ok(())
    }

    /// Retain parent tiles (Leaflet's _retainParent logic)
    /// This replaces the old setup_parent_tile_data functionality
    fn retain_parent_tiles(&mut self, zoom: u8) -> Result<()> {
        let parent_zoom = zoom - 1;
        
        // Get current tiles at this zoom level
        let current_coords: Vec<_> = if let Some(level) = self.levels.get(&zoom) {
            level
                .tiles
                .iter()
                .filter(|(_, tile)| tile.current)
                .map(|(coord, _)| *coord)
                .collect()
        } else {
            return Ok(());
        };

        // Ensure parent level exists
        self.levels
            .entry(parent_zoom)
            .or_insert_with(|| TileLevel::new(parent_zoom));

        // Calculate parent coordinates and ensure they're loaded
        let mut parent_coords = std::collections::HashSet::new();
        for coord in current_coords {
            let parent_coord = TileCoord {
                x: coord.x / 2,
                y: coord.y / 2,
                z: parent_zoom,
            };
            parent_coords.insert(parent_coord);
        }

        if let Some(parent_level) = self.levels.get_mut(&parent_zoom) {
            for parent_coord in parent_coords {
                let tile_state = parent_level
                    .tiles
                    .entry(parent_coord)
                    .or_insert_with(|| TileState::new(parent_coord));

                tile_state.retain = true;

                // Load parent if not already loaded
                if !tile_state.is_loaded() && !tile_state.loading {
                    if let Some(cached_data) = self.tile_cache.get(&parent_coord) {
                        tile_state.mark_loaded(cached_data);
                    } else {
                        tile_state.loading = true;
                        if let Err(e) = self.tile_loader.queue_tile(
                            self.tile_source.as_ref(),
                            parent_coord,
                            TilePriority::Background,
                        ) {
                            tile_state.loading = false;
                            tile_state.mark_error(e.to_string());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Retain child tiles (Leaflet's _retainChildren logic)
    /// This replaces the old preload_next_zoom_level functionality
    fn retain_child_tiles(&mut self, zoom: u8) -> Result<()> {
        let child_zoom = zoom + 1;
        
        // Get a subset of current tiles to avoid loading too many children
        let current_coords: Vec<_> = if let Some(level) = self.levels.get(&zoom) {
            level
                .tiles
                .iter()
                .filter(|(_, tile)| tile.current)
                .map(|(coord, _)| *coord)
                .take(4) // Limit to 4 tiles to avoid excessive loading
                .collect()
        } else {
            return Ok(());
        };

        // Ensure child level exists
        self.levels
            .entry(child_zoom)
            .or_insert_with(|| TileLevel::new(child_zoom));

        // Calculate child coordinates
        let mut child_coords = Vec::new();
        for coord in current_coords {
            let children = coord.children();
            child_coords.extend(children.into_iter().take(4)); // 4 children per tile
        }

        // Mark loaded child tiles for retention
        if let Some(child_level) = self.levels.get_mut(&child_zoom) {
            for child_coord in child_coords {
                if let Some(tile_state) = child_level.tiles.get_mut(&child_coord) {
                    if tile_state.is_loaded() {
                        tile_state.retain = true;
                    }
                }
            }
        }

        Ok(())
    }

    /// Prune old tiles (Leaflet's _pruneTiles logic)
    /// Enhanced with better retention logic and boundary consideration
    fn prune_tiles(&mut self, current_zoom: u8) {
        // Extract boundary checking info to avoid borrow conflicts
        let render_bounds = self.render_bounds.clone();
        let boundary_buffer = self.boundary_buffer;
        
        // Prune tiles within each level
        for level in self.levels.values_mut() {
            level.tiles.retain(|coord, tile| {
                // Always keep current tiles
                if tile.current {
                    return true;
                }
                
                // Keep tiles marked for retention
                if tile.retain {
                    tile.retain = false; // Reset for next frame
                    return true;
                }
                
                // Keep recently loaded tiles for a short time
                if tile.is_loaded() {
                    if let Some(loaded_time) = tile.loaded_time {
                        if loaded_time.elapsed().as_secs() < 30 {
                            return true;
                        }
                    }
                }
                
                // Check boundary constraints (inline to avoid borrow conflict)
                if let Some(ref bounds) = render_bounds {
                    let tile_bounds = Self::calculate_tile_bounds_static(coord);
                    
                    let buffered_bounds = crate::core::geo::LatLngBounds::new(
                        crate::core::geo::LatLng::new(
                            bounds.south_west.lat - boundary_buffer,
                            bounds.south_west.lng - boundary_buffer,
                        ),
                        crate::core::geo::LatLng::new(
                            bounds.north_east.lat + boundary_buffer,
                            bounds.north_east.lng + boundary_buffer,
                        ),
                    );
                    
                    if !buffered_bounds.intersects(&tile_bounds) {
                        return false;
                    }
                }
                
                false
            });
        }

        // Remove levels that are too far from current zoom
        let levels_to_remove: Vec<_> = self
            .levels
            .keys()
            .filter(|&&zoom| (zoom as i16 - current_zoom as i16).abs() > 2)
            .cloned()
            .collect();
        
        for zoom in levels_to_remove {
            #[cfg(feature = "debug")]
            log::debug!("Pruning level {} (too far from current zoom {})", zoom, current_zoom);
            self.levels.remove(&zoom);
        }
    }

    /// Static version of calculate_tile_bounds to avoid borrow conflicts
    fn calculate_tile_bounds_static(coord: &TileCoord) -> crate::core::geo::LatLngBounds {
        let n = 2.0_f64.powi(coord.z as i32);
        
        // Northwest corner
        let nw_lng = coord.x as f64 / n * 360.0 - 180.0;
        let nw_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n);
        let nw_lat = nw_lat_rad.sinh().atan().to_degrees();
        
        // Southeast corner  
        let se_lng = (coord.x + 1) as f64 / n * 360.0 - 180.0;
        let se_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * (coord.y + 1) as f64 / n);
        let se_lat = se_lat_rad.sinh().atan().to_degrees();
        
        crate::core::geo::LatLngBounds::new(
            crate::core::geo::LatLng::new(se_lat, nw_lng), // south-west
            crate::core::geo::LatLng::new(nw_lat, se_lng), // north-east
        )
    }

    /// Count tiles currently loading across all levels
    fn count_loading_tiles(&self) -> usize {
        self.levels
            .values()
            .map(|level| {
                level
                    .tiles
                    .values()
                    .filter(|tile| tile.loading)
                    .count()
            })
            .sum()
    }

    /// Check if a tile coordinate is valid for the current configuration
    fn is_valid_tile(&self, coord: &TileCoord) -> bool {
        let max_coord = 1u32 << coord.z;
        coord.x < max_coord && coord.y < max_coord
            && coord.z >= self.options.min_zoom
            && coord.z <= self.options.max_zoom
    }

    /// Enhanced boundary checking with configurable buffer
    /// This provides the "border" functionality requested by the user
    pub(crate) fn is_tile_within_boundary(&self, coord: &TileCoord) -> bool {
        if let Some(bounds) = &self.render_bounds {
            let tile_bounds = Self::calculate_tile_bounds_static(coord);
            
            // Create buffered bounds for more flexible boundary checking
            let buffered_bounds = crate::core::geo::LatLngBounds::new(
                crate::core::geo::LatLng::new(
                    bounds.south_west.lat - self.boundary_buffer,
                    bounds.south_west.lng - self.boundary_buffer,
                ),
                crate::core::geo::LatLng::new(
                    bounds.north_east.lat + self.boundary_buffer,
                    bounds.north_east.lng + self.boundary_buffer,
                ),
            );
            
            // Check if tile intersects with buffered bounds
            buffered_bounds.intersects(&tile_bounds)
        } else {
            true // No boundary restrictions
        }
    }

    /// Set render bounds for boundary checking
    /// This enables the "border" functionality where only tiles within bounds are rendered
    pub fn set_render_bounds(&mut self, bounds: Option<crate::core::geo::LatLngBounds>) {
        // Trigger re-evaluation of tiles when bounds change
        self.pending_update = true;
        
        #[cfg(feature = "debug")]
        if let Some(ref bounds) = bounds {
            log::info!("Set render bounds: SW({:.6}, {:.6}) - NE({:.6}, {:.6})", 
                bounds.south_west.lat, bounds.south_west.lng,
                bounds.north_east.lat, bounds.north_east.lng);
        } else {
            log::info!("Cleared render bounds");
        }
        
        self.render_bounds = bounds;
    }

    /// Set boundary buffer for softer boundary enforcement
    pub fn set_boundary_buffer(&mut self, buffer: f64) {
        self.boundary_buffer = buffer;
        self.pending_update = true;
    }

    /// Start zoom animation with Leaflet-style parameters
    pub fn start_zoom_animation(
        &mut self,
        from_center: LatLng,
        to_center: LatLng,
        from_zoom: f64,
        to_zoom: f64,
        focus_point: Option<Point>,
    ) {
        if let Some(ref mut animation_manager) = self.animation_manager {
            animation_manager.start_zed_zoom(from_center, to_center, from_zoom, to_zoom, focus_point);
        }
    }

    /// Check if zoom animation is currently active
    pub fn is_zoom_animating(&self) -> bool {
        self.animation_manager
            .as_ref()
            .is_some_and(|manager| manager.is_animating())
    }

    /// Stop all zoom animations and reset transforms
    pub fn stop_zoom_animation(&mut self) {
        if let Some(ref mut animation_manager) = self.animation_manager {
            animation_manager.stop_zoom_animation();
        }
        
        // Reset all level transforms
        for (_, level) in self.levels.iter_mut() {
            level.animating = false;
            level.scale = 1.0;
            level.translation = Point::new(0.0, 0.0);
        }
    }
}
