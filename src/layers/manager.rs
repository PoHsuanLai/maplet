use crate::{core::viewport::Viewport, layers::base::LayerTrait, Result};

use crate::rendering::context::RenderContext;

use crate::prelude::HashMap;

/// Manages layers for the map, handling ordering and rendering
pub struct LayerManager {
    /// All layers indexed by ID
    layers: HashMap<String, Box<dyn LayerTrait>>,
    /// Ordered list of layer IDs for rendering (sorted by z-index)
    render_order: Vec<String>,
}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            layers: HashMap::default(),
            render_order: Vec::new(),
        }
    }

    /// Adds a layer to the manager
    pub fn add_layer(&mut self, layer: Box<dyn LayerTrait>) -> Result<()> {
        let layer_id = layer.id().to_string();
        let z_index = layer.z_index();

        self.layers.insert(layer_id.clone(), layer);

        // Insert in sorted order by z-index
        let insert_pos = self
            .render_order
            .iter()
            .position(|id| {
                self.layers
                    .get(id)
                    .map(|l| l.z_index() > z_index)
                    .unwrap_or(false)
            })
            .unwrap_or(self.render_order.len());

        self.render_order.insert(insert_pos, layer_id);
        Ok(())
    }

    /// Removes a layer from the manager
    pub fn remove_layer(&mut self, layer_id: &str) -> Result<Option<Box<dyn LayerTrait>>> {
        self.render_order.retain(|id| id != layer_id);
        Ok(self.layers.remove(layer_id))
    }

    /// Gets a reference to a layer by ID
    pub fn get_layer(&self, layer_id: &str) -> Option<&dyn LayerTrait> {
        self.layers.get(layer_id).map(|l| l.as_ref())
    }

    /// Applies a function to a specific layer mutably
    pub fn with_layer_mut<F, R>(&mut self, layer_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn LayerTrait) -> R,
    {
        self.layers.get_mut(layer_id).map(|layer| f(layer.as_mut()))
    }

    /// Lists all layer IDs
    pub fn list_layers(&self) -> Vec<String> {
        self.layers.keys().cloned().collect()
    }

    /// Gets all layers in render order
    pub fn layers(&self) -> Vec<&dyn LayerTrait> {
        self.render_order
            .iter()
            .filter_map(|id| self.layers.get(id).map(|l| l.as_ref()))
            .collect()
    }

    /// Applies a function to each layer mutably in render order
    pub fn for_each_layer_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut dyn LayerTrait),
    {
        for id in self.render_order.clone() {
            if let Some(layer) = self.layers.get_mut(&id) {
                f(layer.as_mut());
            }
        }
    }

    /// Applies a function to each layer immutably in render order.
    pub fn for_each_layer<F>(&self, mut f: F)
    where
        F: FnMut(&dyn LayerTrait),
    {
        for id in &self.render_order {
            if let Some(layer) = self.layers.get(id) {
                f(layer.as_ref());
            }
        }
    }

    /// Renders all layers in order
    pub async fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        let viewport_bounds = viewport.bounds();

        for layer_id in self.render_order.clone() {
            if let Some(layer) = self.layers.get_mut(&layer_id) {
                // Only render visible layers that intersect with viewport
                if layer.is_visible() && layer.intersects_bounds(&viewport_bounds) {
                    layer.render(context, viewport)?;
                }
            }
        }
        Ok(())
    }

    /// Updates the render order based on current z-indices
    pub fn update_render_order(&mut self) {
        self.render_order.sort_by(|a, b| {
            let z_a = self.layers.get(a).map(|l| l.z_index()).unwrap_or(0);
            let z_b = self.layers.get(b).map(|l| l.z_index()).unwrap_or(0);
            z_a.cmp(&z_b)
        });
    }

    /// Gets the number of layers
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Checks if the manager is empty
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

impl Default for LayerManager {
    fn default() -> Self {
        Self::new()
    }
}
