use crate::{
    background::{tasks::TaskManagerConfig, BackgroundTaskManager},
    core::{config::MapPerformanceOptions, geo::LatLng, viewport::Viewport},
    input::{Action, EventManager, InputEvent, InputHandler, MapEvent, MapOperations},
    layers::{base::LayerTrait, manager::LayerManager, animation::AnimationManager},
    plugins::base::PluginTrait,
    prelude::HashMap,
    traits::PointMath,
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

/// Centralized update orchestrator that eliminates flickering
/// by being the single source of truth for all timing decisions
#[derive(Debug, Clone)]
pub struct UpdateOrchestrator {
    last_frame_time: std::time::Instant,
    frame_count: u64,
    target_fps: u32,
    min_frame_interval_ms: u64,
    force_update_reasons: Vec<String>,
    animation_active: bool,
    background_work_pending: bool,
    viewport_changed: bool,
    layers_need_update: bool,
    initial_render_done: bool,
    idle_render_count: u64,
}

impl UpdateOrchestrator {
    pub fn new(target_fps: u32) -> Self {
        Self {
            last_frame_time: std::time::Instant::now(),
            frame_count: 0,
            target_fps,
            min_frame_interval_ms: 1000 / target_fps as u64,
            force_update_reasons: Vec::new(),
            animation_active: false,
            background_work_pending: false,
            viewport_changed: false,
            layers_need_update: false,
            initial_render_done: false,
            idle_render_count: 0,
        }
    }

    /// The ONLY method that decides whether to update/render
    /// This is purely functional - no side effects
    pub fn should_update_and_render(&mut self) -> (bool, Vec<String>) {
        let now = std::time::Instant::now();
        let elapsed_ms = now.duration_since(self.last_frame_time).as_millis() as u64;
        
        let mut reasons = Vec::new();
        let mut should_update = false;

        // Always render initially to show the map
        if !self.initial_render_done {
            should_update = true;
            reasons.push("initial_render".to_string());
            self.initial_render_done = true;
        }

        // Always update if we have force reasons (animations, input, etc.)
        if !self.force_update_reasons.is_empty() {
            should_update = true;
            reasons.append(&mut self.force_update_reasons);
        }

        // Update if animations are active
        if self.animation_active {
            should_update = true;
            reasons.push("animation_active".to_string());
        }

        // Update if viewport changed
        if self.viewport_changed {
            should_update = true;
            reasons.push("viewport_changed".to_string());
            self.viewport_changed = false;
        }

        // Update if layers need work
        if self.layers_need_update {
            should_update = true;
            reasons.push("layers_need_update".to_string());
            self.layers_need_update = false;
        }

        // Update if background work completed
        if self.background_work_pending {
            should_update = true;
            reasons.push("background_work_completed".to_string());
            self.background_work_pending = false;
        }

        // Render periodically even when idle to ensure continuous visibility
        // This prevents the map from disappearing when nothing is happening
        if elapsed_ms >= self.min_frame_interval_ms && self.idle_render_count < 10 {
            should_update = true;
            reasons.push("periodic_render".to_string());
            self.idle_render_count += 1;
        }

        // Always render if enough time has passed (ensures map stays visible)
        if elapsed_ms >= self.min_frame_interval_ms * 2 {
            should_update = true;
            reasons.push("maintain_visibility".to_string());
            self.idle_render_count = 0; // Reset idle counter
        }

        // Throttle updates to target FPS unless forced, but be more permissive
        if should_update && elapsed_ms < (self.min_frame_interval_ms / 2) && reasons.len() == 1 && reasons[0] == "background_work_completed" {
            // Only throttle rapid background work updates
            should_update = false;
            reasons.clear();
        }

        // Update frame timing if we're actually updating
        if should_update {
            self.last_frame_time = now;
            self.frame_count += 1;
        }

        (should_update, reasons)
    }

    // Functional setters - no side effects except state updates
    pub fn mark_animation_active(&mut self, active: bool) {
        if active != self.animation_active {
            self.animation_active = active;
            if active {
                self.force_update_reasons.push("animation_started".to_string());
            }
        }
    }

    pub fn mark_viewport_changed(&mut self) {
        self.viewport_changed = true;
    }

    pub fn mark_layers_need_update(&mut self) {
        self.layers_need_update = true;
    }

    pub fn mark_background_work_pending(&mut self) {
        self.background_work_pending = true;
    }

    pub fn force_update(&mut self, reason: String) {
        self.force_update_reasons.push(reason);
    }

    pub fn mark_content_ready(&mut self) {
        self.layers_need_update = true;
        self.force_update_reasons.push("content_ready".to_string());
    }

    pub fn reset_idle_state(&mut self) {
        self.idle_render_count = 0;
    }

    pub fn current_fps(&self) -> f64 {
        if self.frame_count < 2 {
            return 60.0;
        }
        1000.0 / self.min_frame_interval_ms as f64
    }
}

#[derive(Debug, Clone)]
pub struct UpdatePerformanceMetrics {
    pub current_fps: f64,
    pub target_fps: u32,
    pub frame_count: u64,
    pub is_animating: bool,
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
    animation_manager: AnimationManager,
    update_orchestrator: UpdateOrchestrator,
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
        let target_fps = performance.framerate.target_fps.unwrap_or(60);

        let mut map = Self {
            viewport,
            layer_manager: LayerManager::new(),
            plugins: HashMap::default(),
            event_manager: EventManager::new(),
            options,
            performance,
            input_handler: InputHandler::new(),
            background_tasks: BackgroundTaskManager::with_default_config(),
            animation_manager: AnimationManager::new(),
            update_orchestrator: UpdateOrchestrator::new(target_fps),
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
        let target_fps = performance.framerate.target_fps.unwrap_or(60);

        let mut map = Self {
            viewport,
            layer_manager: LayerManager::new(),
            plugins: HashMap::default(),
            event_manager: EventManager::new(),
            options,
            performance,
            input_handler: InputHandler::new(),
            background_tasks: BackgroundTaskManager::new(task_config),
            animation_manager: AnimationManager::new(),
            update_orchestrator: UpdateOrchestrator::new(target_fps),
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
            // Mark viewport change in orchestrator
            self.update_orchestrator.mark_viewport_changed();
            
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
            // Mark animation as active in the orchestrator
            self.update_orchestrator.mark_animation_active(true);
            
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

        if !actions.is_empty() {
            // Mark input activity in orchestrator
            self.update_orchestrator.force_update("user_input".to_string());
        }

        for action in actions {
            match &action {
                Action::Zoom { level, focus_point, animate: true, .. } => {
                    // Use the map's zoom_to method to trigger animations
                    self.zoom_to(*level, *focus_point)?;
                }
                Action::Zoom { level, focus_point, animate: false, .. } => {
                    // Direct zoom without animation
                    MapOperations::zoom_to(&mut self.viewport, *level, *focus_point)?;
                    self.update_orchestrator.mark_viewport_changed();
                }
                Action::PanInertia { .. } => {
                    self.input_handler.start_animation(
                        action,
                        self.viewport.center,
                        self.viewport.zoom,
                    );
                }
                _ => {
                    MapOperations::execute_action(&mut self.viewport, action)?;
                    // Mark viewport change for immediate actions
                    self.update_orchestrator.mark_viewport_changed();
                }
            }
        }

        Ok(())
    }

    /// The ONLY method that should be called for updating/rendering
    /// This replaces all previous render() and update() methods
    pub fn update_and_render(
        &mut self,
        render_context: &mut crate::rendering::context::RenderContext,
    ) -> Result<bool> {
        // Check for background task results and mark if work is pending
        if self.background_tasks.has_pending_results() {
            self.update_orchestrator.mark_background_work_pending();
        }

        // Update animations and mark if active
        if let Some(animation_state) = self.animation_manager.update() {
            self.update_orchestrator.mark_animation_active(animation_state.progress < 1.0);
            
            // Apply the animation's transform to the viewport for visual effect
            self.viewport.set_transform(animation_state.transform);
            
            // Update center and zoom during animation (smooth interpolation)
            if animation_state.progress < 1.0 {
                // During animation, use interpolated values but don't update actual viewport zoom/center until complete
                // This creates the smooth visual effect while keeping tile loading stable
            } else {
                // Animation complete - apply final values and clear transform
                self.viewport.center = animation_state.center;
                self.viewport.zoom = animation_state.zoom;
                self.viewport.clear_transform();
                
                // Emit zoom end event
                self.event_manager.emit(MapEvent::ZoomEnd { 
                    zoom: animation_state.zoom 
                });
            }
        } else {
            self.update_orchestrator.mark_animation_active(false);
            // Ensure transform is cleared when no animation is active
            if self.viewport.has_active_transform() {
                self.viewport.clear_transform();
            }
        }

        // Process input animations
        if let Some((center, zoom)) = self.input_handler.update_animation() {
            self.viewport.center = center;
            self.viewport.zoom = zoom;
            self.update_orchestrator.mark_viewport_changed();
        }

        // Check if orchestrator decides to update/render
        let (should_update, reasons) = self.update_orchestrator.should_update_and_render();
        
        // Be more permissive - always render if we have layers that need rendering
        let force_render = !self.layer_manager.is_empty();
        
        if !should_update && !force_render {
            return Ok(false);
        }

        // Process background task results
        let task_results = self.background_tasks.try_recv_results();
        if !task_results.is_empty() {
            // Handle results and potentially trigger layer updates
            for _result in task_results {
                self.update_orchestrator.mark_layers_need_update();
            }
        }

        // Update layers and check for content changes
        let mut content_changed = false;
        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.update(0.016); // Fixed delta time since timing is controlled centrally
            
            // Check if tile layer has new content
            if let Some(tile_layer) = layer.as_any().downcast_ref::<crate::layers::tile::TileLayer>() {
                if tile_layer.needs_repaint() {
                    content_changed = true;
                }
            }
        });

        // Notify orchestrator if content changed
        if content_changed {
            self.update_orchestrator.mark_content_ready();
        }

        // Update plugins
        for (_, plugin) in self.plugins.iter_mut() {
            let _ = plugin.update(0.016);
        }

        // Render everything
        render_context.begin_frame()?;
        
        // Set up viewport clipping bounds
        let viewport_bounds = (
            crate::core::geo::Point::new(0.0, 0.0),
            crate::core::geo::Point::new(self.viewport.size.x, self.viewport.size.y),
        );
        render_context.set_clip_bounds(viewport_bounds.0, viewport_bounds.1);
        
        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.render(render_context, &self.viewport);
        });
        
        for (_, plugin) in self.plugins.iter_mut() {
            let _ = plugin.render(render_context, &self.viewport);
        }
        
        // Clear clipping after rendering
        render_context.clear_clip_bounds();

        // Log update reasons for debugging
        #[cfg(feature = "debug")]
        if !reasons.is_empty() {
            println!("🔄 [UPDATE] Reasons: {:?}", reasons);
        }

        Ok(true)
    }

    /// Legacy render method - deprecated, use update_and_render instead
    #[deprecated(note = "Use update_and_render instead")]
    pub fn render(
        &mut self,
        render_context: &mut crate::rendering::context::RenderContext,
    ) -> Result<()> {
        let _ = self.update_and_render(render_context)?;
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
        // Update orchestrator target FPS if changed
        if let Some(target_fps) = performance.framerate.target_fps {
            self.update_orchestrator = UpdateOrchestrator::new(target_fps);
        }
        
        self.performance = performance;
    }

    /// Get current performance metrics from the orchestrator
    pub fn get_performance_metrics(&self) -> UpdatePerformanceMetrics {
        UpdatePerformanceMetrics {
            current_fps: self.update_orchestrator.current_fps(),
            target_fps: self.update_orchestrator.target_fps,
            frame_count: self.update_orchestrator.frame_count,
            is_animating: self.update_orchestrator.animation_active,
        }
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

    /// Get the update orchestrator for advanced configuration
    pub fn update_orchestrator(&self) -> &UpdateOrchestrator {
        &self.update_orchestrator
    }

    /// Get mutable access to the update orchestrator for advanced configuration
    pub fn update_orchestrator_mut(&mut self) -> &mut UpdateOrchestrator {
        &mut self.update_orchestrator
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
