
use crate::{
    core::{map::Map, viewport::Viewport},
    input::events::InputEvent,
    Result,
};
use async_trait::async_trait;

use crate::rendering::context::RenderContext;

#[async_trait]
pub trait PluginTrait: Send + Sync {
    fn name(&self) -> &str;
    fn on_add(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }
    fn on_remove(&self, _map: &mut Map) -> Result<()> {
        Ok(())
    }
    fn handle_input(&mut self, _input: &InputEvent) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, _delta_time: f64) -> Result<()> {
        Ok(())
    }
    fn render(&mut self, _context: &mut RenderContext, _viewport: &Viewport) -> Result<()> {
        Ok(())
    }
}