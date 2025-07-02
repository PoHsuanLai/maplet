use crate::core::geo::{LatLng, TileCoord};
use crate::core::viewport::Viewport;
use lru::LruCache;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Intelligent tile cache with multi-level prefetching strategy
#[derive(Debug)]
pub struct TileCache {
    cache: Arc<Mutex<LruCache<TileCoord, Arc<Vec<u8>>>>>,
    /// Current viewport for smart prefetching
    current_viewport: Arc<Mutex<Option<Viewport>>>,
    /// Movement direction for predictive prefetching
    movement_direction: Arc<Mutex<Option<(f64, f64)>>>, // (x, y) direction vector
    /// Last center position for direction calculation
    last_center: Arc<Mutex<Option<LatLng>>>,
}

impl TileCache {
    /// Create a new tile cache with the given capacity
    pub fn new(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(2048).unwrap()); // Increased default
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            current_viewport: Arc::new(Mutex::new(None)),
            movement_direction: Arc::new(Mutex::new(None)),
            last_center: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new tile cache with default capacity (2048 tiles for better prefetching)
    pub fn with_default_capacity() -> Self {
        Self::new(2048)
    }

    /// Update viewport and calculate movement direction for smart prefetching
    pub fn update_viewport(&self, viewport: &Viewport) {
        // Calculate movement direction
        if let Ok(mut last_center) = self.last_center.lock() {
            if let Some(last) = *last_center {
                let movement = (
                    viewport.center.lng - last.lng,
                    viewport.center.lat - last.lat,
                );
                
                // Normalize movement vector
                let magnitude = (movement.0 * movement.0 + movement.1 * movement.1).sqrt();
                if magnitude > 0.0001 { // Only update if significant movement
                    if let Ok(mut direction) = self.movement_direction.lock() {
                        *direction = Some((movement.0 / magnitude, movement.1 / magnitude));
                    }
                }
            }
            *last_center = Some(viewport.center);
        }

        // Update current viewport
        if let Ok(mut current) = self.current_viewport.lock() {
            *current = Some(viewport.clone());
        }
    }

    /// Get tiles that should be prefetched for the current view
    /// This implements your requirements:
    /// - Current view tiles
    /// - Tiles around current view
    /// - Tiles at ±1 zoom level
    /// - Directional prefetching based on movement
    pub fn get_prefetch_tiles(&self, viewport: &Viewport) -> Vec<TileCoord> {
        let mut prefetch_tiles = HashSet::new();
        let current_zoom = viewport.zoom.round() as u32;

        // 1. Get visible tiles for current zoom
        let visible_tiles = self.get_visible_tiles(viewport, current_zoom);
        prefetch_tiles.extend(visible_tiles.iter());

        // 2. Get surrounding tiles (padding around visible area)
        let padded_tiles = self.get_padded_tiles(viewport, current_zoom, 1); // 1 tile padding
        prefetch_tiles.extend(padded_tiles.iter());

        // 3. Get tiles at ±1 zoom levels
        if current_zoom > 0 {
            let lower_zoom_tiles = self.get_visible_tiles(viewport, current_zoom - 1);
            prefetch_tiles.extend(lower_zoom_tiles.iter());
        }
        if current_zoom < 18 {
            let higher_zoom_tiles = self.get_visible_tiles(viewport, current_zoom + 1);
            prefetch_tiles.extend(higher_zoom_tiles.iter());
        }

        // 4. Directional prefetching based on movement
        if let Ok(direction) = self.movement_direction.lock() {
            if let Some((dx, dy)) = *direction {
                let directional_tiles = self.get_directional_tiles(viewport, current_zoom, dx, dy);
                prefetch_tiles.extend(directional_tiles.iter());
            }
        }

        prefetch_tiles.into_iter().collect()
    }

    /// Get tiles visible in the current viewport
    fn get_visible_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();
        let scale = 2.0_f64.powi(zoom as i32);

        // Convert lat/lng bounds to tile coordinates
        let min_x = ((bounds.south_west.lng + 180.0) / 360.0 * scale).floor() as u32;
        let max_x = ((bounds.north_east.lng + 180.0) / 360.0 * scale).ceil() as u32;
        
        let lat_rad_north = bounds.north_east.lat.to_radians();
        let lat_rad_south = bounds.south_west.lat.to_radians();
        
        let min_y = ((1.0 - (lat_rad_north.tan() + (1.0 / lat_rad_north.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).floor() as u32;
        let max_y = ((1.0 - (lat_rad_south.tan() + (1.0 / lat_rad_south.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).ceil() as u32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                tiles.push(TileCoord { x, y, z: zoom as u8 });
            }
        }
        tiles
    }

    /// Get tiles with padding around the visible area
    fn get_padded_tiles(&self, viewport: &Viewport, zoom: u32, padding: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();
        let scale = 2.0_f64.powi(zoom as i32);

        // Calculate tile bounds with padding
        let min_x = ((bounds.south_west.lng + 180.0) / 360.0 * scale).floor() as i32 - padding as i32;
        let max_x = ((bounds.north_east.lng + 180.0) / 360.0 * scale).ceil() as i32 + padding as i32;
        
        let lat_rad_north = bounds.north_east.lat.to_radians();
        let lat_rad_south = bounds.south_west.lat.to_radians();
        
        let min_y = ((1.0 - (lat_rad_north.tan() + (1.0 / lat_rad_north.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).floor() as i32 - padding as i32;
        let max_y = ((1.0 - (lat_rad_south.tan() + (1.0 / lat_rad_south.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).ceil() as i32 + padding as i32;

        let max_tile = (2.0_f64.powi(zoom as i32)) as i32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if x >= 0 && y >= 0 && x < max_tile && y < max_tile {
                    tiles.push(TileCoord { 
                        x: x as u32, 
                        y: y as u32, 
                        z: zoom as u8 
                    });
                }
            }
        }
        tiles
    }

    /// Get tiles in the predicted movement direction
    fn get_directional_tiles(&self, viewport: &Viewport, zoom: u32, dx: f64, dy: f64) -> Vec<TileCoord> {
        let bounds = viewport.bounds();
        let scale = 2.0_f64.powi(zoom as i32);

        // Calculate how far to look ahead based on movement direction
        let look_ahead_distance = 2.0; // Look 2 tile widths ahead

        // Calculate new bounds shifted in movement direction
        let lng_shift = dx * look_ahead_distance * 360.0 / scale;
        let lat_shift = dy * look_ahead_distance * 180.0 / scale;

        let shifted_west = bounds.south_west.lng + lng_shift;
        let shifted_east = bounds.north_east.lng + lng_shift;
        let shifted_north = (bounds.north_east.lat + lat_shift).clamp(-85.0, 85.0);
        let shifted_south = (bounds.south_west.lat + lat_shift).clamp(-85.0, 85.0);

        // Get tiles for the shifted viewport
        let min_x = ((shifted_west + 180.0) / 360.0 * scale).floor() as i32;
        let max_x = ((shifted_east + 180.0) / 360.0 * scale).ceil() as i32;
        
        let lat_rad_north = shifted_north.to_radians();
        let lat_rad_south = shifted_south.to_radians();
        
        let min_y = ((1.0 - (lat_rad_north.tan() + (1.0 / lat_rad_north.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).floor() as i32;
        let max_y = ((1.0 - (lat_rad_south.tan() + (1.0 / lat_rad_south.cos())).ln() / std::f64::consts::PI) / 2.0 * scale).ceil() as i32;

        let max_tile = (2.0_f64.powi(zoom as i32)) as i32;

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if x >= 0 && y >= 0 && x < max_tile && y < max_tile {
                    tiles.push(TileCoord { 
                        x: x as u32, 
                        y: y as u32, 
                        z: zoom as u8 
                    });
                }
            }
        }
        tiles
    }

    /// Get a tile from the cache
    pub fn get(&self, coord: &TileCoord) -> Option<Arc<Vec<u8>>> {
        self.cache.lock().ok()?.get(coord).cloned()
    }

    /// Insert a tile into the cache
    pub fn insert(&self, coord: TileCoord, data: Vec<u8>) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(coord, Arc::new(data));
        }
    }

    /// Insert a tile into the cache (using Arc directly)
    pub fn put(&self, coord: TileCoord, data: Arc<Vec<u8>>) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(coord, data);
        }
    }

    /// Check if a tile is in the cache without retrieving it
    pub fn contains(&self, coord: &TileCoord) -> bool {
        if let Ok(cache) = self.cache.lock() {
            cache.contains(coord)
        } else {
            false
        }
    }

    /// Remove a tile from the cache
    pub fn remove(&self, coord: &TileCoord) -> Option<Arc<Vec<u8>>> {
        self.cache.lock().ok()?.pop(coord)
    }

    /// Clear all tiles from the cache
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Get the current number of cached tiles
    pub fn len(&self) -> usize {
        self.cache.lock().ok().map(|cache| cache.len()).unwrap_or(0)
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache capacity
    pub fn capacity(&self) -> usize {
        self.cache
            .lock()
            .ok()
            .map(|cache| cache.cap().get())
            .unwrap_or(0)
    }
}

impl Clone for TileCache {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            current_viewport: Arc::clone(&self.current_viewport),
            movement_direction: Arc::clone(&self.movement_direction),
            last_center: Arc::clone(&self.last_center),
        }
    }
}

impl Default for TileCache {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_cache_basic_operations() {
        let cache = TileCache::new(2);
        let coord1 = TileCoord { x: 1, y: 2, z: 3 };
        let coord2 = TileCoord { x: 4, y: 5, z: 6 };
        let data1 = vec![1, 2, 3];
        let data2 = vec![4, 5, 6];

        // Initially empty
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Insert and retrieve
        cache.insert(coord1, data1.clone());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&coord1));

        let retrieved = cache.get(&coord1).unwrap();
        assert_eq!(*retrieved, data1);

        // Insert second item
        cache.insert(coord2, data2.clone());
        assert_eq!(cache.len(), 2);

        // Clear cache
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_tile_cache_lru_eviction() {
        let cache = TileCache::new(2);
        let coord1 = TileCoord { x: 1, y: 1, z: 1 };
        let coord2 = TileCoord { x: 2, y: 2, z: 2 };
        let coord3 = TileCoord { x: 3, y: 3, z: 3 };

        // Fill cache to capacity
        cache.insert(coord1, vec![1]);
        cache.insert(coord2, vec![2]);
        assert_eq!(cache.len(), 2);

        // Insert third item should evict first
        cache.insert(coord3, vec![3]);
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&coord1)); // Evicted
        assert!(cache.contains(&coord2));
        assert!(cache.contains(&coord3));
    }
}
