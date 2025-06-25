use crate::{
    core::{map::Map, viewport::Viewport},
    input::events::InputEvent,
    rendering::context::RenderContext,
    Result,
};
use async_trait::async_trait;

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
    async fn render(&mut self, context: &mut RenderContext, viewport: &Viewport) -> Result<()> {
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
