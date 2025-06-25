pub struct MapControls {
    pub zoom_enabled: bool,
    pub pan_enabled: bool,
}

impl MapControls {
    pub fn new() -> Self {
        Self {
            zoom_enabled: true,
            pan_enabled: true,
        }
    }
}

impl Default for MapControls {
    fn default() -> Self {
        Self::new()
    }
}
