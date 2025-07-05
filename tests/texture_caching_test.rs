#[cfg(test)]
mod texture_caching_tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[test]
    fn test_texture_name_generation() {
        // Test that identical data generates identical texture names
        let tile_data1 = vec![1, 2, 3, 4, 5];
        let tile_data2 = vec![1, 2, 3, 4, 5]; // Same data
        let tile_data3 = vec![1, 2, 3, 4, 6]; // Different data
        
        let bounds = ((10.0, 20.0), (30.0, 40.0)); // (x1,y1), (x2,y2)
        
        // Generate hashes for texture names
        let hash1 = generate_texture_hash(&tile_data1, &bounds);
        let hash2 = generate_texture_hash(&tile_data2, &bounds);
        let hash3 = generate_texture_hash(&tile_data3, &bounds);
        
        // Same data should generate same hash
        assert_eq!(hash1, hash2, "Identical tile data should generate identical texture names");
        
        // Different data should generate different hash
        assert_ne!(hash1, hash3, "Different tile data should generate different texture names");
        
        // Verify texture names are stable
        let name1 = format!("maplet_tile_cached_{:016x}", hash1);
        let name2 = format!("maplet_tile_cached_{:016x}", hash2);
        assert_eq!(name1, name2, "Texture names should be stable for identical data");
    }
    
    #[test]
    fn test_texture_hash_consistency() {
        // Test that the hash function is consistent across multiple calls
        let data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let bounds = ((0.0, 0.0), (256.0, 256.0));
        
        let hash1 = generate_texture_hash(&data, &bounds);
        let hash2 = generate_texture_hash(&data, &bounds);
        let hash3 = generate_texture_hash(&data, &bounds);
        
        assert_eq!(hash1, hash2, "Hash should be consistent");
        assert_eq!(hash2, hash3, "Hash should be consistent");
    }
    
    #[test]
    fn test_bounds_affect_hash() {
        // Test that different bounds generate different hashes
        let data = vec![1, 2, 3, 4];
        let bounds1 = ((0.0, 0.0), (256.0, 256.0));
        let bounds2 = ((100.0, 100.0), (356.0, 356.0));
        
        let hash1 = generate_texture_hash(&data, &bounds1);
        let hash2 = generate_texture_hash(&data, &bounds2);
        
        assert_ne!(hash1, hash2, "Different bounds should generate different hashes");
    }
    
    // Helper function that mimics the texture hash generation in the widget
    fn generate_texture_hash(data: &[u8], bounds: &((f64, f64), (f64, f64))) -> u64 {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        // Hash bounds coordinates manually since Point doesn't implement Hash
        (bounds.0.0 as i64).hash(&mut hasher);
        (bounds.0.1 as i64).hash(&mut hasher);
        (bounds.1.0 as i64).hash(&mut hasher);
        (bounds.1.1 as i64).hash(&mut hasher);
        hasher.finish()
    }
} 