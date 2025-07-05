use crate::core::geo::{LatLng, TileCoord};
use crate::core::viewport::Viewport;
use lru::LruCache;
use crate::prelude::{HashSet, Arc, Mutex};
use std::num::NonZeroUsize;

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
    /// This implements a simple but effective approach:
    /// - Current view tiles (highest priority)
    /// - Tiles around current view with keepBuffer (medium priority)
    /// - Tiles at ±1 zoom level (background priority)
    pub fn get_prefetch_tiles(&self, viewport: &Viewport) -> Vec<TileCoord> {
        let mut prefetch_tiles = HashSet::default();
        let current_zoom = viewport.zoom.round() as u32;

        // 1. Get visible tiles for current zoom
        let visible_tiles = self.get_visible_tiles(viewport, current_zoom);
        prefetch_tiles.extend(visible_tiles.iter());

        // 2. Get surrounding tiles with keepBuffer
        let buffer_tiles = self.get_buffered_tiles(viewport, current_zoom, 2); // keepBuffer = 2 default
        prefetch_tiles.extend(buffer_tiles.iter());

        // 3. Get parent tiles at zoom-1 for smooth zoom out
        if current_zoom > 0 {
            let parent_tiles = self.get_parent_tiles(viewport, current_zoom - 1);
            prefetch_tiles.extend(parent_tiles.iter());
        }

        // 4. Get some child tiles at zoom+1 for smooth zoom in
        if current_zoom < 18 {
            let child_tiles = self.get_child_tiles(viewport, current_zoom + 1);
            // Limit child tiles to avoid loading too many
            prefetch_tiles.extend(child_tiles.iter().take(16));
        }

        prefetch_tiles.into_iter().collect()
    }

    /// Get tiles visible in the current viewport
    fn get_visible_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();
        
        // Debug: Log viewport bounds
        println!("🔍 [TILES] Viewport bounds: N={:.4}, S={:.4}, E={:.4}, W={:.4} (zoom={})", 
                 bounds.north_east.lat, bounds.south_west.lat, bounds.north_east.lng, bounds.south_west.lng, zoom);
        
        // Use unified projection for consistency
        let nw_proj = viewport.project(&LatLng::new(bounds.north_east.lat, bounds.south_west.lng), Some(zoom as f64));
        let se_proj = viewport.project(&LatLng::new(bounds.south_west.lat, bounds.north_east.lng), Some(zoom as f64));
        
        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as i32;
        let max_x = (se_proj.x / tile_size).ceil() as i32;
        let min_y = (nw_proj.y / tile_size).floor() as i32;
        let max_y = (se_proj.y / tile_size).ceil() as i32;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;
        
        // Debug: Log tile range
        println!("🔍 [TILES] Tile range: x={}-{}, y={}-{} (max_tile={})", 
                 min_x, max_x, min_y, max_y, max_tile);

        let mut tiles = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if x >= 0 && y >= 0 && x < max_tile && y < max_tile {
                    tiles.push(TileCoord { x: x as u32, y: y as u32, z: zoom as u8 });
                }
            }
        }
        
        // Debug: Log first few tiles
        if !tiles.is_empty() {
            println!("🔍 [TILES] First 3 tiles: {:?}", tiles.iter().take(3).collect::<Vec<_>>());
        }
        
        tiles
    }

    /// Get tiles with buffer around the visible area
    fn get_buffered_tiles(&self, viewport: &Viewport, zoom: u32, buffer: u32) -> Vec<TileCoord> {
        let bounds = viewport.bounds();
        
        // Use unified projection for consistency
        let nw_proj = viewport.project(&LatLng::new(bounds.north_east.lat, bounds.south_west.lng), Some(zoom as f64));
        let se_proj = viewport.project(&LatLng::new(bounds.south_west.lat, bounds.north_east.lng), Some(zoom as f64));
        
        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as i32 - buffer as i32;
        let max_x = (se_proj.x / tile_size).ceil() as i32 + buffer as i32;
        let min_y = (nw_proj.y / tile_size).floor() as i32 - buffer as i32;
        let max_y = (se_proj.y / tile_size).ceil() as i32 + buffer as i32;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;

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

    /// Get parent tiles at lower zoom for smooth zoom out
    fn get_parent_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        if zoom == 0 {
            return Vec::new();
        }

        let bounds = viewport.bounds();
        
        // Use unified projection for consistency
        let nw_proj = viewport.project(&LatLng::new(bounds.north_east.lat, bounds.south_west.lng), Some(zoom as f64));
        let se_proj = viewport.project(&LatLng::new(bounds.south_west.lat, bounds.north_east.lng), Some(zoom as f64));
        
        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as i32;
        let max_x = (se_proj.x / tile_size).ceil() as i32;
        let min_y = (nw_proj.y / tile_size).floor() as i32;
        let max_y = (se_proj.y / tile_size).ceil() as i32;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;

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

    /// Get child tiles at higher zoom for smooth zoom in
    fn get_child_tiles(&self, viewport: &Viewport, zoom: u32) -> Vec<TileCoord> {
        if zoom > 18 {
            return Vec::new();
        }

        let bounds = viewport.bounds();
        
        // Use unified projection for consistency
        let nw_proj = viewport.project(&LatLng::new(bounds.north_east.lat, bounds.south_west.lng), Some(zoom as f64));
        let se_proj = viewport.project(&LatLng::new(bounds.south_west.lat, bounds.north_east.lng), Some(zoom as f64));
        
        let tile_size = 256.0;
        let min_x = (nw_proj.x / tile_size).floor() as i32;
        let max_x = (se_proj.x / tile_size).ceil() as i32;
        let min_y = (nw_proj.y / tile_size).floor() as i32;
        let max_y = (se_proj.y / tile_size).ceil() as i32;

        let max_tile = (256.0 * 2_f64.powf(zoom as f64) / tile_size) as i32;

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

/// Implement unified Cacheable trait for TileCache
impl crate::traits::Cacheable for TileCache {
    type Key = crate::core::geo::TileCoord;
    type Value = Arc<Vec<u8>>;
    
    fn get_cached(&self, key: &Self::Key) -> Option<Self::Value> {
        self.get(key)
    }
    
    fn cache(&mut self, key: Self::Key, value: Self::Value) {
        self.put(key, value);
    }
    
    fn invalidate(&mut self, key: &Self::Key) {
        self.remove(key);
    }
    
    fn clear_cache(&mut self) {
        self.clear();
    }
    
    fn cache_stats(&self) -> crate::traits::CacheStats {
        crate::traits::CacheStats {
            hits: 0, // TileCache doesn't track hits/misses currently
            misses: 0,
            size: self.len(),
        }
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
