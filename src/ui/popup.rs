use crate::core::geo::LatLng;

pub struct Popup {
    pub position: LatLng,
    pub content: String,
    pub visible: bool,
}

impl Popup {
    pub fn new(position: LatLng, content: String) -> Self {
        Self {
            position,
            content,
            visible: false,
        }
    }
}
