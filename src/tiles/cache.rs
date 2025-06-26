use std::sync::{Arc, Mutex};
use lru::LruCache;
use std::num::NonZeroUsize;
use crate::core::geo::TileCoord;

/// In-memory tile cache using LRU eviction
#[derive(Debug)]
pub struct TileCache {
    cache: Arc<Mutex<LruCache<TileCoord, Arc<Vec<u8>>>>>,
}

impl TileCache {
    /// Create a new tile cache with the given capacity
    pub fn new(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1024).unwrap());
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
        }
    }

    /// Create a new tile cache with default capacity (1024 tiles)
    pub fn with_default_capacity() -> Self {
        Self::new(1024)
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

    /// Check if a tile is in the cache
    pub fn contains(&self, coord: &TileCoord) -> bool {
        self.cache.lock().ok()
            .map(|cache| cache.contains(coord))
            .unwrap_or(false)
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
        self.cache.lock().ok()
            .map(|cache| cache.len())
            .unwrap_or(0)
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache capacity
    pub fn capacity(&self) -> usize {
        self.cache.lock().ok()
            .map(|cache| cache.cap().get())
            .unwrap_or(0)
    }
}

impl Clone for TileCache {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
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