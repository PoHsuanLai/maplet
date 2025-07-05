//! Control management - simplified to avoid duplication
//!
//! This module provides basic control configuration that works with the unified UI elements.

use crate::ui::elements::Position;

/// Simplified control configuration
#[derive(Debug, Clone)]
pub struct ControlConfig {
    pub visible: bool,
    pub position: Position,
    pub margin: f32,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            visible: true,
            position: Position::TopRight,
            margin: 10.0,
        }
    }
}

/// Simple control manager that delegates to the unified components
pub struct ControlManager {
    config: ControlConfig,
}

impl ControlManager {
    pub fn new(config: ControlConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(ControlConfig::default())
    }

    pub fn config(&self) -> &ControlConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: ControlConfig) {
        self.config = config;
    }
}

impl Default for ControlManager {
    fn default() -> Self {
        Self::with_default_config()
    }
}
