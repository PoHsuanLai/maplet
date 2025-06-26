use crate::{
    background::{tasks::TaskManagerConfig, BackgroundTaskManager},
    core::{config::MapPerformanceOptions, geo::LatLng, viewport::Viewport},
    input::events::InputEvent,
    layers::{base::LayerTrait, manager::LayerManager},
    plugins::base::PluginTrait,
    Result,
};

use fxhash::FxHashMap;
use std::collections::VecDeque;

/// Event types that can be emitted by the map
#[derive(Debug, Clone, PartialEq)]
pub enum MapEvent {
    /// Map view has changed (center, zoom, or size)
    ViewChanged { center: LatLng, zoom: f64 },
    /// Mouse/touch click on the map
    Click {
        lat_lng: LatLng,
        pixel: crate::core::geo::Point,
    },
    /// Mouse/touch move over the map
    MouseMove {
        lat_lng: LatLng,
        pixel: crate::core::geo::Point,
    },
    /// Zoom started
    ZoomStart { zoom: f64 },
    /// Zoom ended
    ZoomEnd { zoom: f64 },
    /// Pan started
    MoveStart { center: LatLng },
    /// Pan ended
    MoveEnd { center: LatLng },
    /// Layer was added to the map
    LayerAdd { layer_id: String },
    /// Layer was removed from the map  
    LayerRemove { layer_id: String },
    /// Base layer was changed
    BaseLayerChange { layer_id: String },
    /// Overlay layer was added
    OverlayAdd { layer_id: String },
    /// Overlay layer was removed
    OverlayRemove { layer_id: String },
}

/// Event listener callback type
pub type EventCallback = Box<dyn Fn(&MapEvent) + Send + Sync>;

/// Main map structure that manages the entire mapping interface
pub struct Map {
    /// Current viewport (center, zoom, size)
    pub viewport: Viewport,
    /// Layer management
    layer_manager: LayerManager,
    /// Registered plugins
    plugins: FxHashMap<String, Box<dyn PluginTrait>>,
    /// Event listeners
    event_listeners: FxHashMap<String, Vec<EventCallback>>,
    /// Event queue for processing
    event_queue: VecDeque<MapEvent>,
    /// Map options and settings
    options: MapOptions,
    /// Performance configuration
    performance: MapPerformanceOptions,
    /// Whether the map is currently being dragged
    is_dragging: bool,
    /// Whether the map is currently being zoomed
    is_zooming: bool,
    /// Background task manager for CPU-intensive operations
    background_tasks: BackgroundTaskManager,
    /// Timing tracking for frame rate control
    last_render_time: std::time::Instant,
    /// Timing tracking for update rate control
    last_update_time: std::time::Instant,
}

/// Configuration options for the map
#[derive(Debug, Clone)]
pub struct MapOptions {
    /// Whether the map can be dragged
    pub dragging: bool,
    /// Whether the map can be zoomed with mouse wheel
    pub scroll_wheel_zoom: bool,
    /// Whether the map can be zoomed by double-clicking
    pub double_click_zoom: bool,
    /// Whether the map can be zoomed with touch gestures
    pub touch_zoom: bool,
    /// Whether keyboard navigation is enabled
    pub keyboard: bool,
    /// Maximum bounds for the map view
    pub max_bounds: Option<crate::core::geo::LatLngBounds>,
    /// Minimum zoom level
    pub min_zoom: Option<f64>,
    /// Maximum zoom level
    pub max_zoom: Option<f64>,
    /// Whether to show attribution control
    pub attribution_control: bool,
    /// Whether to show zoom control
    pub zoom_control: bool,
    /// Snap zoom levels to multiples of this value
    pub zoom_snap: f64,
    /// Amount of zoom change for one discrete step
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

impl Map {
    /// Creates a new map with default options
    pub fn new(center: LatLng, zoom: f64, size: crate::core::geo::Point) -> Self {
        let viewport = Viewport::new(center, zoom, size);
        Self::with_options(viewport, MapOptions::default())
    }

    /// Creates a new map with custom options
    pub fn with_options(viewport: Viewport, options: MapOptions) -> Self {
        let performance = MapPerformanceOptions::default();
        let now = std::time::Instant::now();

        let mut map = Self {
            viewport,
            layer_manager: LayerManager::new(),
            plugins: FxHashMap::default(),
            event_listeners: FxHashMap::default(),
            event_queue: VecDeque::new(),
            options,
            performance,
            is_dragging: false,
            is_zooming: false,
            background_tasks: BackgroundTaskManager::with_default_config(),
            last_render_time: now,
            last_update_time: now,
        };

        // Apply zoom limits from options
        if let (Some(min), Some(max)) = (map.options.min_zoom, map.options.max_zoom) {
            map.viewport.set_zoom_limits(min, max);
        }

        map
    }

    /// Creates a new map with custom options and performance configuration
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
            plugins: FxHashMap::default(),
            event_listeners: FxHashMap::default(),
            event_queue: VecDeque::new(),
            options,
            performance,
            is_dragging: false,
            is_zooming: false,
            background_tasks: BackgroundTaskManager::new(task_config),
            last_render_time: now,
            last_update_time: now,
        };

        // Apply zoom limits from options
        if let (Some(min), Some(max)) = (map.options.min_zoom, map.options.max_zoom) {
            map.viewport.set_zoom_limits(min, max);
        }

        Ok(map)
    }

    /// Sets the map view to a specific center and zoom level
    pub fn set_view(&mut self, center: LatLng, zoom: f64) -> Result<()> {
        let old_center = self.viewport.center;
        let old_zoom = self.viewport.zoom;

        self.viewport.set_center(center);
        self.viewport.set_zoom(zoom);

        // Check if we're within max bounds
        if let Some(max_bounds) = &self.options.max_bounds {
            if !max_bounds.contains(&center) {
                // Clamp to bounds
                let clamped_center = LatLng::new(
                    center
                        .lat
                        .clamp(max_bounds.south_west.lat, max_bounds.north_east.lat),
                    center
                        .lng
                        .clamp(max_bounds.south_west.lng, max_bounds.north_east.lng),
                );
                self.viewport.set_center(clamped_center);
            }
        }

        // Emit view change event if anything actually changed
        if self.viewport.center != old_center || self.viewport.zoom != old_zoom {
            self.emit_event(MapEvent::ViewChanged {
                center: self.viewport.center,
                zoom: self.viewport.zoom,
            });
        }

        Ok(())
    }

    /// Pans the map by the given pixel offset
    pub fn pan(&mut self, delta: crate::core::geo::Point) -> Result<()> {
        if !self.options.dragging {
            return Ok(());
        }

        let old_center = self.viewport.center;
        self.viewport.pan(delta);

        if self.viewport.center != old_center {
            self.emit_event(MapEvent::ViewChanged {
                center: self.viewport.center,
                zoom: self.viewport.zoom,
            });
        }

        Ok(())
    }

    /// Zooms the map to a specific level
    pub fn zoom_to(
        &mut self,
        zoom: f64,
        focus_point: Option<crate::core::geo::Point>,
    ) -> Result<()> {
        // Apply snapping before committing
        let mut target_zoom = zoom.clamp(
            self.options.min_zoom.unwrap_or(self.viewport.min_zoom),
            self.options.max_zoom.unwrap_or(self.viewport.max_zoom),
        );

        if self.options.zoom_snap > 0.0 {
            target_zoom = ((target_zoom / self.options.zoom_snap).round()) * self.options.zoom_snap;
        }

        self.viewport.zoom_to(target_zoom, focus_point);
        self.emit_event(MapEvent::ViewChanged {
            center: self.viewport.center,
            zoom: self.viewport.zoom,
        });
        Ok(())
    }

    /// Adds a layer to the map
    pub fn add_layer(&mut self, layer: Box<dyn LayerTrait>) -> Result<()> {
        let layer_id = layer.id().to_string();
        layer.on_add(self)?;
        self.layer_manager.add_layer(layer)?;

        self.emit_event(MapEvent::LayerAdd { layer_id });
        Ok(())
    }

    /// Removes a layer from the map
    pub fn remove_layer(&mut self, layer_id: &str) -> Result<()> {
        if let Some(layer) = self.layer_manager.remove_layer(layer_id)? {
            layer.on_remove(self)?;
            self.emit_event(MapEvent::LayerRemove {
                layer_id: layer_id.to_string(),
            });
        }
        Ok(())
    }

    /// Gets a reference to a layer by ID
    pub fn get_layer(&self, layer_id: &str) -> Option<&dyn LayerTrait> {
        self.layer_manager.get_layer(layer_id)
    }

    /// Gets a mutable reference to a layer by ID
    /// Applies a function to a layer mutably
    pub fn with_layer_mut<F, R>(&mut self, layer_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn LayerTrait) -> R,
    {
        self.layer_manager.with_layer_mut(layer_id, f)
    }

    /// Iterate over all layers mutably.
    pub fn for_each_layer_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut dyn crate::layers::base::LayerTrait),
    {
        self.layer_manager.for_each_layer_mut(|layer| f(layer));
    }

    /// Lists all layer IDs
    pub fn list_layers(&self) -> Vec<String> {
        self.layer_manager.list_layers()
    }

    /// Adds a plugin to the map
    pub fn add_plugin(&mut self, name: String, plugin: Box<dyn PluginTrait>) -> Result<()> {
        plugin.on_add(self)?;
        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Removes a plugin from the map
    pub fn remove_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.remove(name) {
            plugin.on_remove(self)?;
        }
        Ok(())
    }

    /// Registers an event listener
    pub fn on<F>(&mut self, event_type: &str, callback: F)
    where
        F: Fn(&MapEvent) + Send + Sync + 'static,
    {
        let callbacks = self
            .event_listeners
            .entry(event_type.to_string())
            .or_default();
        callbacks.push(Box::new(callback));
    }

    /// Emits an event to all registered listeners
    fn emit_event(&mut self, event: MapEvent) {
        // Add to queue for processing
        self.event_queue.push_back(event.clone());

        // Get event type string
        let event_type = match &event {
            MapEvent::ViewChanged { .. } => "viewchange",
            MapEvent::Click { .. } => "click",
            MapEvent::MouseMove { .. } => "mousemove",
            MapEvent::ZoomStart { .. } => "zoomstart",
            MapEvent::ZoomEnd { .. } => "zoomend",
            MapEvent::MoveStart { .. } => "movestart",
            MapEvent::MoveEnd { .. } => "moveend",
            MapEvent::LayerAdd { .. } => "layeradd",
            MapEvent::LayerRemove { .. } => "layerremove",
            MapEvent::BaseLayerChange { .. } => "baselayerchange",
            MapEvent::OverlayAdd { .. } => "overlayadd",
            MapEvent::OverlayRemove { .. } => "overlayremove",
        };

        // Call all registered listeners for this event type
        if let Some(callbacks) = self.event_listeners.get(event_type) {
            for callback in callbacks {
                callback(&event);
            }
        }
    }

    /// Handles input events
    pub fn handle_input(&mut self, input: InputEvent) -> Result<()> {
        match input {
            InputEvent::Click { position } => {
                let lat_lng = self.viewport.pixel_to_lat_lng(&position);
                self.emit_event(MapEvent::Click {
                    lat_lng,
                    pixel: position,
                });
            }
            InputEvent::MouseMove { position } => {
                let lat_lng = self.viewport.pixel_to_lat_lng(&position);
                self.emit_event(MapEvent::MouseMove {
                    lat_lng,
                    pixel: position,
                });
            }
            InputEvent::DragStart { position: _ } => {
                // Start tracking drag
                self.is_dragging = true;
                self.emit_event(MapEvent::MoveStart {
                    center: self.viewport.center,
                });
            }
            InputEvent::Drag { delta } => {
                if !self.is_dragging {
                    self.is_dragging = true;
                    self.emit_event(MapEvent::MoveStart {
                        center: self.viewport.center,
                    });
                }
                self.pan(delta)?;
            }
            InputEvent::DragEnd => {
                if self.is_dragging {
                    self.is_dragging = false;
                    self.emit_event(MapEvent::MoveEnd {
                        center: self.viewport.center,
                    });
                }
            }
            InputEvent::Scroll { delta, position } => {
                if self.options.scroll_wheel_zoom {
                    if !self.is_zooming {
                        self.is_zooming = true;
                        self.emit_event(MapEvent::ZoomStart {
                            zoom: self.viewport.zoom,
                        });
                    }

                    let zoom_delta = if delta > 0.0 { 0.5 } else { -0.5 };
                    self.zoom_to(self.viewport.zoom + zoom_delta, Some(position))?;

                    self.is_zooming = false;
                    self.emit_event(MapEvent::ZoomEnd {
                        zoom: self.viewport.zoom,
                    });
                }
            }
            InputEvent::DoubleClick { position } => {
                if self.options.double_click_zoom {
                    self.zoom_to(self.viewport.zoom + 1.0, Some(position))?;
                }
            }
            InputEvent::KeyPress { key, modifiers } => {
                if self.options.keyboard {
                    self.handle_keyboard_navigation(&key, &modifiers)?;
                }
            }
            InputEvent::Resize { size } => {
                self.viewport.set_size(size);
                self.emit_event(MapEvent::ViewChanged {
                    center: self.viewport.center,
                    zoom: self.viewport.zoom,
                });
            }
            InputEvent::Touch {
                ref event_type,
                ref touches,
            } => {
                if self.options.touch_zoom {
                    self.handle_touch_events(event_type, touches)?;
                }
            }
        }

        // Forward input to layers and plugins
        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.handle_input(&input);
        });

        for plugin in self.plugins.values_mut() {
            plugin.handle_input(&input)?;
        }

        Ok(())
    }

    /// Updates the map state (called each frame)
    pub fn update(&mut self, delta_time: f64) -> Result<()> {
        // Update all layers
        self.layer_manager.for_each_layer_mut(|layer| {
            let _ = layer.update(delta_time);
        });

        // Update all plugins
        for plugin in self.plugins.values_mut() {
            plugin.update(delta_time)?;
        }

        Ok(())
    }

    /// Renders the map
    /// Render the map using the provided render context
    #[cfg(feature = "render")]
    pub fn render(
        &mut self,
        render_context: &mut crate::rendering::context::RenderContext,
    ) -> Result<()> {
        use crate::core::bounds::Bounds;
        use crate::spatial::culling::Culling;

        let screen_bounds =
            Bounds::from_coords(0.0, 0.0, self.viewport.size.x, self.viewport.size.y);

        // Render all layers in z-index order, culling those off-screen.
        self.layer_manager.for_each_layer_mut(|layer| {
            if !layer.visible() {
                return;
            }

            // If layer has geographical bounds, do a quick cull.
            if let Some(layer_geo_bounds) = layer.bounds() {
                // Convert to pixel AABB.
                let sw_px = self.viewport.lat_lng_to_pixel(&layer_geo_bounds.south_west);
                let ne_px = self.viewport.lat_lng_to_pixel(&layer_geo_bounds.north_east);
                let layer_px_bounds = Bounds::from_coords(sw_px.x, ne_px.y, ne_px.x, sw_px.y);

                if !Culling::aabb_intersects(&screen_bounds, &layer_px_bounds) {
                    return; // culled
                }
            }

            // If this is a tile layer, update its tiles first (sync processing of completed downloads)
            if let Some(tile_layer) = layer
                .as_any_mut()
                .downcast_mut::<crate::layers::tile::TileLayer>()
            {
                let _ = tile_layer.update_tiles(&self.viewport);
            }

            let _ = layer.render(render_context, &self.viewport);
        });

        // Render all plugins
        for plugin in self.plugins.values_mut() {
            let _ = plugin.render(render_context, &self.viewport);
        }

        Ok(())
    }

    /// No-op render method when render feature is disabled
    #[cfg(not(feature = "render"))]
    pub fn render(&mut self, _render_context: &mut ()) -> Result<()> {
        Err(Box::new(crate::MapError::FeatureNotEnabled(
            "render feature not enabled - enable 'render' feature to use GPU rendering".to_string(),
        )))
    }

    /// Gets the current viewport
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Gets a mutable reference to the viewport
    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    /// Gets the map options
    pub fn options(&self) -> &MapOptions {
        &self.options
    }

    /// Gets access to the background task manager
    pub fn background_tasks(&self) -> &BackgroundTaskManager {
        &self.background_tasks
    }

    /// Get the current performance configuration
    pub fn performance(&self) -> &MapPerformanceOptions {
        &self.performance
    }

    /// Update the performance configuration
    pub fn set_performance(&mut self, performance: MapPerformanceOptions) {
        self.performance = performance;
    }

    /// Check if rendering should occur based on performance settings
    pub fn should_render(&self) -> bool {
        self.performance
            .framerate
            .should_render(self.last_render_time)
    }

    /// Check if updating should occur based on performance settings
    pub fn should_update(&self) -> bool {
        self.performance
            .framerate
            .should_update(self.last_update_time)
    }

    /// Mark that a render has occurred (for timing tracking)
    pub fn mark_render(&mut self) {
        self.last_render_time = std::time::Instant::now();
    }

    /// Mark that an update has occurred (for timing tracking)
    pub fn mark_update(&mut self) {
        self.last_update_time = std::time::Instant::now();
    }

    /// Processes queued events (call this regularly)
    pub fn process_events(&mut self) -> Vec<MapEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.event_queue.pop_front() {
            events.push(event);
        }
        events
    }

    /// Fits the map view to contain the given bounds
    pub fn fit_bounds(
        &mut self,
        bounds: &crate::core::geo::LatLngBounds,
        padding: Option<f64>,
    ) -> Result<()> {
        self.viewport.fit_bounds(bounds, padding);
        self.emit_event(MapEvent::ViewChanged {
            center: self.viewport.center,
            zoom: self.viewport.zoom,
        });
        Ok(())
    }

    /// Handle keyboard navigation
    fn handle_keyboard_navigation(
        &mut self,
        key: &crate::input::events::KeyCode,
        _modifiers: &crate::input::events::KeyModifiers,
    ) -> Result<()> {
        use crate::core::geo::Point;
        use crate::input::events::KeyCode;

        let pan_distance = 50.0; // pixels
        let zoom_delta = 0.5;

        match key {
            KeyCode::ArrowUp => {
                self.pan(Point::new(0.0, -pan_distance))?;
            }
            KeyCode::ArrowDown => {
                self.pan(Point::new(0.0, pan_distance))?;
            }
            KeyCode::ArrowLeft => {
                self.pan(Point::new(-pan_distance, 0.0))?;
            }
            KeyCode::ArrowRight => {
                self.pan(Point::new(pan_distance, 0.0))?;
            }
            KeyCode::Plus => {
                self.zoom_to(self.viewport.zoom + zoom_delta, None)?;
            }
            KeyCode::Minus => {
                self.zoom_to(self.viewport.zoom - zoom_delta, None)?;
            }
            KeyCode::Home => {
                // Reset to initial view
                self.set_view(LatLng::new(0.0, 0.0), 1.0)?;
            }
            _ => {} // Ignore other keys
        }

        Ok(())
    }

    /// Handle touch events for gestures
    fn handle_touch_events(
        &mut self,
        event_type: &crate::input::events::TouchEventType,
        touches: &[crate::input::events::TouchPoint],
    ) -> Result<()> {
        use crate::core::geo::Point;
        use crate::input::events::TouchEventType;

        match event_type {
            TouchEventType::Start => {
                if touches.len() == 1 {
                    // Single touch - start potential pan
                    self.is_dragging = true;
                } else if touches.len() == 2 {
                    // Two finger touch - potential pinch zoom
                    self.is_zooming = true;
                    self.emit_event(MapEvent::ZoomStart {
                        zoom: self.viewport.zoom,
                    });
                }
            }
            TouchEventType::Move => {
                if touches.len() == 1 && self.is_dragging {
                    // Single finger pan
                    if let Some(last_touch) = touches.first() {
                        let delta = Point::new(
                            last_touch.position.x
                                - last_touch
                                    .previous_position
                                    .unwrap_or(last_touch.position)
                                    .x,
                            last_touch.position.y
                                - last_touch
                                    .previous_position
                                    .unwrap_or(last_touch.position)
                                    .y,
                        );
                        self.pan(delta)?;
                    }
                } else if touches.len() == 2 && self.is_zooming {
                    // Pinch zoom
                    let touch1 = &touches[0];
                    let touch2 = &touches[1];

                    let current_distance = ((touch1.position.x - touch2.position.x).powi(2)
                        + (touch1.position.y - touch2.position.y).powi(2))
                    .sqrt();

                    if let (Some(prev1), Some(prev2)) =
                        (touch1.previous_position, touch2.previous_position)
                    {
                        let previous_distance =
                            ((prev1.x - prev2.x).powi(2) + (prev1.y - prev2.y).powi(2)).sqrt();

                        if previous_distance > 0.0 {
                            let scale_factor = current_distance / previous_distance;
                            let zoom_delta = scale_factor.log2();

                            // Calculate center point of pinch
                            let center = Point::new(
                                (touch1.position.x + touch2.position.x) / 2.0,
                                (touch1.position.y + touch2.position.y) / 2.0,
                            );

                            self.zoom_to(self.viewport.zoom + zoom_delta, Some(center))?;
                        }
                    }
                }
            }
            TouchEventType::End => {
                if self.is_dragging {
                    self.is_dragging = false;
                    self.emit_event(MapEvent::MoveEnd {
                        center: self.viewport.center,
                    });
                }
                if self.is_zooming {
                    self.is_zooming = false;
                    self.emit_event(MapEvent::ZoomEnd {
                        zoom: self.viewport.zoom,
                    });
                }
            }
            TouchEventType::Cancel => {
                self.is_dragging = false;
                self.is_zooming = false;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geo::Point;

    #[test]
    fn test_map_creation() {
        let map = Map::new(
            LatLng::new(40.7128, -74.0060),
            10.0,
            Point::new(800.0, 600.0),
        );

        assert_eq!(map.viewport.zoom, 10.0);
        assert_eq!(map.viewport.center.lat, 40.7128);
    }

    #[test]
    fn test_set_view() {
        let mut map = Map::new(LatLng::new(0.0, 0.0), 1.0, Point::new(800.0, 600.0));

        let new_center = LatLng::new(40.7128, -74.0060);
        map.set_view(new_center, 12.0).unwrap();

        assert_eq!(map.viewport.center, new_center);
        assert_eq!(map.viewport.zoom, 12.0);
    }

    #[test]
    fn test_event_handling() {
        let mut map = Map::new(LatLng::new(0.0, 0.0), 1.0, Point::new(800.0, 600.0));

        let event_received = false;
        map.on("click", move |_event| {
            // In a real test, we'd use Arc<Mutex<bool>> or similar
            // event_received = true;
        });

        let click_input = InputEvent::Click {
            position: Point::new(400.0, 300.0),
        };

        map.handle_input(click_input).unwrap();

        // Verify event was queued
        assert!(!map.event_queue.is_empty());
    }
}
