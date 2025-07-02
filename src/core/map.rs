use crate::{
    background::{tasks::TaskManagerConfig, BackgroundTaskManager},
    core::{config::MapPerformanceOptions, geo::LatLng, viewport::Viewport},
    input::{Action, EventManager, InputEvent, InputHandler, MapEvent, MapOperations},
    layers::{base::LayerTrait, manager::LayerManager, animation::AnimationManager},
    plugins::base::PluginTrait,
    prelude::HashMap,
    Result,
};

#[derive(Debug, Clone)]
pub struct MapOptions {
    pub dragging: bool,
    pub scroll_wheel_zoom: bool,
    pub double_click_zoom: bool,
    pub touch_zoom: bool,
    pub keyboard: bool,
    pub max_bounds: Option<crate::core::geo::LatLngBounds>,
    pub min_zoom: Option<f64>,
    pub max_zoom: Option<f64>,
    pub attribution_control: bool,
    pub zoom_control: bool,
    pub zoom_snap: f64,
    pub zoom_delta: f64,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            dragging: true,
            scroll_wheel_zoom: true,
            double_click_zoom: true,
            touch_zoom: true,
            keyboard: true,
            max_bounds: None,
            min_zoom: None,
            max_zoom: None,
            attribution_control: true,
            zoom_control: true,
            zoom_snap: 1.0,
            zoom_delta: 1.0,
        }
    }
}

pub struct Map {
    pub viewport: Viewport,
    layer_manager: LayerManager,
    plugins: HashMap<String, Box<dyn PluginTrait>>,
    event_manager: EventManager,
    options: MapOptions,
    performance: MapPerformanceOptions,
    input_handler: InputHandler,
    background_tasks: BackgroundTaskManager,
    last_render_time: std::time::Instant,
    last_update_time: std::time::Instant,
    animation_manager: AnimationManager,
}

impl Map {
    pub fn new(center: LatLng, zoom: f64, size: crate::core::geo::Point) -> Self {
        let viewport = Viewport::new(center, zoom, size);
        Self::with_options(viewport, MapOptions::default())
    }

    pub fn for_testing(center: LatLng, zoom: f64, size: crate::core::geo::Point) -> Self {
        let viewport = Viewport::new(center, zoom, size);
        let options = MapOptions::default();
        let task_config = crate::background::tasks::TaskManagerConfig {
            test_mode: true,
            ..Default::default()
        };
        let performance = crate::core::config::MapPerformanceOptions::default();
        Self::with_options_and_performance(viewport, options, performance, task_config).unwrap()
    }

    pub fn with_options(viewport: Viewport, options: MapOptions) -> Self {
        let performance = MapPerformanceOptions::default();
        let now = std::time::Instant::now();

        let mut map = Self {
            viewport,
            layer_manager: LayerManager::new(),
            plugins: HashMap::default(),
            event_manager: EventManager::new(),
            options,
            performance,
            input_handler: InputHandler::new(),
            background_tasks: BackgroundTaskManager::with_default_config(),
            last_render_time: now,
            last_update_time: now,
            animation_manager: AnimationManager::new(),
        };

        if let (Some(min), Some(max)) = (map.options.min_zoom, map.options.max_zoom) {
            map.viewport.set_zoom_limits(min, max);
        }

        map
    }

    pub fn with_options_and_performance(
        viewport: Viewport,
        options: MapOptions,
        performance: MapPerformanceOptions,
        task_config: TaskManagerConfig,
    ) -> Result<Self> {
        let now = std::time::Instant::now();

        let mut map = Self {
            viewport,
            layer_manager: LayerManager::new(),
            plugins: HashMap::default(),
            event_manager: EventManager::new(),
            options,
            performance,
            input_handler: InputHandler::new(),
            background_tasks: BackgroundTaskManager::new(task_config),
            last_render_time: now,
            last_update_time: now,
            animation_manager: AnimationManager::new(),
        };

        if let (Some(min), Some(max)) = (map.options.min_zoom, map.options.max_zoom) {
            map.viewport.set_zoom_limits(min, max);
        }

        Ok(map)
    }

    pub fn set_view(&mut self, center: LatLng, zoom: f64) -> Result<()> {
        let old_center = self.viewport.center;
        let old_zoom = self.viewport.zoom;

        MapOperations::set_view(&mut self.viewport, center, zoom)?;

        if self.viewport.center != old_center || self.viewport.zoom != old_zoom {
            self.event_manager.emit(MapEvent::ViewChanged {
                center: self.viewport.center,
                zoom: self.viewport.zoom,
            });
        }

        Ok(())
    }

    pub fn pan(&mut self) -> Result<()> {
        let old_center = self.viewport.center;

        if self.viewport.center != old_center {
            self.event_manager.emit(MapEvent::MoveEnd {
                center: self.viewport.center,
            });
        }

        Ok(())
    }

    pub fn zoom_to(
        &mut self,
        zoom: f64,
        focus_point: Option<crate::core::geo::Point>,
    ) -> Result<()> {
        let old_zoom = self.viewport.zoom;
        let old_center = self.viewport.center;

        let new_center = if let Some(focus) = focus_point {
            let viewport_size = self.viewport.size;
            let scale = 2_f64.powf(zoom - old_zoom);
            let view_half =
                crate::core::geo::Point::new(viewport_size.x / 2.0, viewport_size.y / 2.0);
            let center_offset = focus.subtract(&view_half).scale(1.0 - 1.0 / scale);
            let new_center_point = view_half.add(&center_offset);
            self.viewport.pixel_to_lat_lng(&new_center_point)
        } else {
            old_center
        };

        if self.animation_manager.try_animate_zoom(
            old_center,
            new_center,
            old_zoom,
            zoom,
            focus_point,
            None,
        ) {
            self.layer_manager.for_each_layer_mut(|layer| {
                if let Some(tile_layer) = layer.as_any_mut().downcast_mut::<crate::layers::tile::TileLayer>() {
                    tile_layer.start_zoom_animation(old_center, new_center, old_zoom, zoom, focus_point);
                }
            });
            
            self.event_manager
                .emit(MapEvent::ZoomStart { zoom: old_zoom });
            return Ok(());
        }

        MapOperations::zoom_to(&mut self.viewport, zoom, focus_point)?;

        if self.viewport.zoom != old_zoom {
            self.event_manager.emit(MapEvent::ZoomEnd {
                zoom: self.viewport.zoom,
            });
        }

        Ok(())
    }

    pub fn set_max_bounds(
        &mut self,
        bounds: Option<crate::core::geo::LatLngBounds>,
        viscosity: Option<f64>,
    ) {
        self.viewport.set_max_bounds(bounds, viscosity);
    }

    pub fn fit_bounds(
        &mut self,
        bounds: &crate::core::geo::LatLngBounds,
        padding: Option<f64>,
    ) -> Result<()> {
        MapOperations::fit_bounds(&mut self.viewport, bounds, padding)
    }

    pub fn add_layer(&mut self, layer: Box<dyn LayerTrait>) -> Result<()> {
        self.layer_manager.add_layer(layer)
    }

    pub fn remove_layer(&mut self, layer_id: &str) -> Result<()> {
        if self.layer_manager.remove_layer(layer_id)?.is_some() {
            self.event_manager.emit(MapEvent::LayerRemove {
                layer_id: layer_id.to_string(),
            });
        }
        Ok(())
    }

    pub fn get_layer(&self, layer_id: &str) -> Option<&dyn LayerTrait> {
        self.layer_manager.get_layer(layer_id)
    }

    pub fn with_layer_mut<F, R>(&mut self, layer_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn LayerTrait) -> R,
    {
        self.layer_manager.with_layer_mut(layer_id, f)
    }

    pub fn for_each_layer_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut dyn crate::layers::base::LayerTrait),
    {
        self.layer_manager.for_each_layer_mut(&mut f);
    }

    pub fn list_layers(&self) -> Vec<String> {
        self.layer_manager.list_layers()
    }

    pub fn add_plugin(&mut self, name: String, plugin: Box<dyn PluginTrait>) -> Result<()> {
        plugin.on_add(self)?;
        self.plugins.insert(name, plugin);
        Ok(())
    }

    pub fn remove_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.remove(name) {
            plugin.on_remove(self)?;
        }
        Ok(())
    }

    pub fn on<F>(&mut self, event_type: &str, callback: F)
    where
        F: Fn(&MapEvent) + Send + Sync + 'static,
    {
        self.event_manager.on(event_type, callback);
    }

    pub fn process_events(&mut self) -> Vec<MapEvent> {
        self.event_manager.process_events()
    }

    pub fn handle_input(&mut self, input: InputEvent) -> Result<()> {
        let actions =
            self.input_handler
                .handle_event(input, self.viewport.center, self.viewport.zoom);

        for action in actions {
            match &action {
                Action::PanInertia { .. } => {
                    self.input_handler.start_animation(
                        action,
                        self.viewport.center,
                        self.viewport.zoom,
                    );
                }
                _ => {
                    MapOperations::execute_action(&mut self.viewport, action)?;
                }
            }
        }

        Ok(())
    }

    pub fn update(&mut self, delta_time: f64) -> Result<()> {
        let now = std::time::Instant::now();

        if now.duration_since(self.last_update_time).as_millis() < 16 {
            return Ok(());
        }

        if let Some(animation_state) = self.animation_manager.update() {
            // Update integrated tile layer animations (they handle their own animation state now)
            // No need for manual tile layer animation updates
            
            let viewport_transform = crate::core::viewport::Transform::new(
                animation_state.transform.translate,
                animation_state.transform.scale,
                animation_state.transform.origin,
            );
            self.viewport.set_transform(viewport_transform);

            if animation_state.progress >= 1.0 {
                // Animation complete - clean up
                self.viewport.set_center(animation_state.center);
                self.viewport.set_zoom(animation_state.zoom);
                self.viewport.clear_transform();
                self.event_manager.emit(MapEvent::ZoomEnd {
                    zoom: animation_state.zoom,
                });
            }
        }

        if let Some((center, zoom)) = self.input_handler.update_animation() {
            self.viewport.set_center(center);
            self.viewport.set_zoom(zoom);
        }

        let actions = self
            .input_handler
            .process_queued_events(self.viewport.center, self.viewport.zoom);
        for action in actions {
            MapOperations::execute_action(&mut self.viewport, action)?;
        }

        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.update(delta_time);
        });

        for plugin in self.plugins.values_mut() {
            plugin.update(delta_time)?;
        }

        self.last_update_time = now;
        Ok(())
    }

    pub fn render(
        &mut self,
        render_context: &mut crate::rendering::context::RenderContext,
    ) -> Result<()> {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_render_time).as_millis() < 16 {
            return Ok(());
        }

        render_context.begin_frame()?;
        
        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.render(render_context, &self.viewport);
        });
        
        for (_, plugin) in self.plugins.iter_mut() {
            let _ = plugin.render(render_context, &self.viewport);
        }

        self.last_render_time = now;
        Ok(())
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }
    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }
    pub fn options(&self) -> &MapOptions {
        &self.options
    }
    pub fn background_tasks(&self) -> &BackgroundTaskManager {
        &self.background_tasks
    }
    pub fn performance(&self) -> &MapPerformanceOptions {
        &self.performance
    }

    pub fn set_performance(&mut self, performance: MapPerformanceOptions) {
        self.performance = performance;
    }

    pub fn should_render(&self) -> bool {
        std::time::Instant::now()
            .duration_since(self.last_render_time)
            .as_millis()
            > 16
    }

    pub fn should_update(&self) -> bool {
        std::time::Instant::now()
            .duration_since(self.last_update_time)
            .as_millis()
            > 16
    }

    pub fn mark_render(&mut self) {
        self.last_render_time = std::time::Instant::now();
    }

    pub fn mark_update(&mut self) {
        self.last_update_time = std::time::Instant::now();
    }

    pub fn stop_animations(&mut self) {
        self.animation_manager.stop_zoom_animation();
        self.input_handler.stop_animation();
    }

    pub fn set_zoom_animation_enabled(&mut self, enabled: bool) {
        self.animation_manager.set_zoom_animation_enabled(enabled);
    }

    pub fn set_zoom_animation_threshold(&mut self, threshold: f64) {
        self.animation_manager
            .set_zoom_animation_threshold(threshold);
    }

    pub fn set_zoom_animation_style(
        &mut self,
        easing: crate::layers::animation::EasingType,
        duration: std::time::Duration,
    ) {
        self.animation_manager.set_zoom_style(easing, duration);
    }

    pub async fn render_layers(
        &mut self,
        render_context: &mut crate::rendering::context::RenderContext,
    ) -> Result<()> {
        self.layer_manager
            .render(render_context, &self.viewport)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geo::LatLng;
    use crate::layers::tile::TileLayer;

    #[tokio::test]
    async fn test_map_creation() {
        let center = LatLng::new(0.0, 0.0);
        let zoom = 1.0;
        let size = crate::core::geo::Point::new(800.0, 600.0);
        let map = Map::new(center, zoom, size);

        assert_eq!(map.viewport.center, center);
        assert_eq!(map.viewport.zoom, zoom);
        assert_eq!(map.viewport.size, size);
    }

    #[tokio::test]
    async fn test_set_view() {
        let mut map = Map::new(
            LatLng::new(0.0, 0.0),
            1.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );
        let new_center = LatLng::new(10.0, 20.0);
        let new_zoom = 5.0;

        map.set_view(new_center, new_zoom).unwrap();
        assert_eq!(map.viewport.center, new_center);
        assert_eq!(map.viewport.zoom, new_zoom);
    }

    #[tokio::test]
    async fn test_layer_management() {
        let mut map = Map::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        let tile_layer = TileLayer::openstreetmap("osm".to_string(), "OpenStreetMap".to_string());
        let result = map.add_layer(Box::new(tile_layer));
        assert!(result.is_ok());

        assert!(map.get_layer("osm").is_some());

        let layers = map.list_layers();
        assert!(layers.contains(&"osm".to_string()));

        let remove_result = map.remove_layer("osm");
        assert!(remove_result.is_ok());
        assert!(map.get_layer("osm").is_none());
    }

    #[tokio::test]
    async fn test_zoom_to() {
        let mut map = Map::new(
            LatLng::new(37.7749, -122.4194),
            10.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        let new_zoom = 15.0;
        let result = map.zoom_to(new_zoom, None);
        assert!(result.is_ok());
        assert_eq!(map.viewport.zoom, new_zoom);
    }

    #[tokio::test]
    async fn test_performance_settings() {
        let mut map = Map::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        let mut perf = MapPerformanceOptions::default();
        perf.framerate.target_fps = Some(30);
        map.set_performance(perf);

        assert_eq!(map.performance().framerate.target_fps, Some(30));
    }

    #[tokio::test]
    async fn test_animation_controls() {
        let mut map = Map::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        map.set_zoom_animation_enabled(false);
        map.set_zoom_animation_enabled(true);

        map.set_zoom_animation_threshold(0.5);

        map.stop_animations();
    }

    #[tokio::test]
    async fn test_timing_controls() {
        let map = Map::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        let _should_render = map.should_render();
        let _should_update = map.should_update();
    }

    #[tokio::test]
    async fn test_viewport_access() {
        let mut map = Map::new(
            LatLng::new(37.7749, -122.4194),
            12.0,
            crate::core::geo::Point::new(800.0, 600.0),
        );

        let viewport = map.viewport();
        assert_eq!(viewport.center.lat, 37.7749);

        let new_size = crate::core::geo::Point::new(1200.0, 900.0);
        map.viewport_mut().set_size(new_size);
        assert_eq!(map.viewport().size, new_size);
    }

    #[test]
    fn test_map_options() {
        let options = MapOptions::default();
        assert!(options.dragging);
        assert!(options.scroll_wheel_zoom);
        assert!(options.double_click_zoom);
        assert!(options.touch_zoom);
        assert!(options.keyboard);
        assert!(options.attribution_control);
        assert!(options.zoom_control);
        assert_eq!(options.zoom_snap, 1.0);
        assert_eq!(options.zoom_delta, 1.0);
    }

    #[tokio::test]
    async fn test_coordinate_bounds() {
        let extreme_coords = [
            (85.0511, 180.0),
            (-85.0511, -180.0),
            (0.0, 0.0),
            (37.7749, -122.4194), // San Francisco
        ];

        for (lat, lng) in extreme_coords.iter() {
            let center = LatLng::new(*lat, *lng);
            let map = Map::new(center, 10.0, crate::core::geo::Point::new(800.0, 600.0));
            assert_eq!(map.viewport.center.lat, *lat);
            assert_eq!(map.viewport.center.lng, *lng);
        }
    }

    #[tokio::test]
    async fn test_zoom_bounds() {
        let zoom_levels = [0.0, 1.0, 5.0, 10.0, 15.0, 18.0];

        for zoom in zoom_levels.iter() {
            let map = Map::new(
                LatLng::new(37.7749, -122.4194),
                *zoom,
                crate::core::geo::Point::new(800.0, 600.0),
            );
            assert_eq!(map.viewport.zoom, *zoom);
        }
    }
}
