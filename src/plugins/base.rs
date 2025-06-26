use crate::{
    core::{map::Map, viewport::Viewport},
    input::events::InputEvent,
    Result,
};
use async_trait::async_trait;

#[cfg(feature = "render")]
use crate::rendering::context::RenderContext;

#[async_trait]
pub trait PluginTrait: Send + Sync {
    fn name(&self) -> &str;
    fn on_add(&self, map: &mut Map) -> Result<()> {
        Ok(())
    }
    fn on_remove(&self, map: &mut Map) -> Result<()> {
        Ok(())
    }
    fn handle_input(&mut self, input: &InputEvent) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, delta_time: f64) -> Result<()> {
        Ok(())
    }
    #[cfg(feature = "render")]
    fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
        Ok(())
    }

    #[cfg(not(feature = "render"))]
    fn render(&mut self, _context: &mut (), _viewport: &Viewport) -> Result<()> {
        Ok(())
    }
}

// Create all other module stubs
use std::collections::HashMap;

pub struct BasePlugin {
    pub name: String,
    pub enabled: bool,
    pub options: HashMap<String, serde_json::Value>,
}

impl BasePlugin {
    pub fn new(name: String) -> Self {
        Self {
            name,
            enabled: true,
            options: HashMap::new(),
        }
    }
}

#[async_trait]
impl PluginTrait for BasePlugin {
    fn name(&self) -> &str {
        &self.name
    }
}
