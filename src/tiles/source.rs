use crate::core::geo::TileCoord;

/// Trait representing anything that can produce tile URLs for a given coordinate.
pub trait TileSource: Send + Sync {
    /// Build a URL for the requested `coord`.
    fn url(&self, coord: TileCoord) -> String;
}

/// Simple implementation that hits the default OpenStreetMap tile server.
pub struct OpenStreetMapSource {
    subdomains: Vec<&'static str>,
}

impl OpenStreetMapSource {
    pub fn new() -> Self {
        Self { subdomains: vec!["a", "b", "c"] }
    }
}

impl Default for OpenStreetMapSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TileSource for OpenStreetMapSource {
    fn url(&self, coord: TileCoord) -> String {
        // Guard against empty subdomain list (should not happen, but be safe)
        if self.subdomains.is_empty() {
            return format!("https://tile.openstreetmap.org/{}/{}/{}.png", coord.z, coord.x, coord.y);
        }

        let idx = ((coord.x + coord.y) % self.subdomains.len() as u32) as usize;
        let sub = self.subdomains[idx];
        format!(
            "https://{}.tile.openstreetmap.org/{}/{}/{}.png",
            sub, coord.z, coord.x, coord.y
        )
    }
} 