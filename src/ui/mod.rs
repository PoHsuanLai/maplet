pub mod elements;
pub mod traits;

pub mod widget;

pub mod style;

pub mod components;
pub mod controls;
pub mod popup;

pub use traits::{
    AsAny, BaseConfig, EventHandler, Positionable, Renderable, Styleable, UiEvent, UiEventResult,
    ViewportAware,
};

pub use elements::{Attribution, Button, Position, UiManager, ZoomControl};

pub use widget::{Map, MapTheme, MapWidgetExt};

pub use style::{
    AttributionStyle, MapStyle, MapThemes, MarkerStyle, StyleExt, VectorStyle, ZoomControlStyle,
};

pub use controls::{ControlConfig, ControlManager};

pub use popup::{Popup, PopupManager, PopupPosition, PopupStyle};

pub use components::*;

pub trait UiMapExt {
    fn map(&mut self) -> egui::Response;

    fn map_at(&mut self, lat: f64, lng: f64) -> egui::Response;

    fn map_at_zoom(&mut self, lat: f64, lng: f64, zoom: f64) -> egui::Response;
}

impl UiMapExt for egui::Ui {
    fn map(&mut self) -> egui::Response {
        self.add(Map::new())
    }

    fn map_at(&mut self, lat: f64, lng: f64) -> egui::Response {
        self.add(Map::new().center(lat, lng))
    }

    fn map_at_zoom(&mut self, lat: f64, lng: f64, zoom: f64) -> egui::Response {
        self.add(Map::new().center(lat, lng).zoom(zoom))
    }
}
