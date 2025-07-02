//! Background GeoJSON parsing tasks
//!
//! This module provides background tasks for parsing GeoJSON data
//! without blocking the main thread.

use crate::background::tasks::{BackgroundTask, TaskId, TaskPriority};
use crate::core::geo::LatLngBounds;
use crate::data::geojson::{GeoJson, GeoJsonFeature, GeoJsonLayer};
use crate::Result;
use std::future::Future;
use std::pin::Pin;

#[cfg(feature = "debug")]
use log::{debug, error, info};

/// Task for parsing GeoJSON from a string in the background
pub struct GeoJsonParseTask {
    pub task_id: TaskId,
    pub data: String,
    pub source_url: Option<String>,
}

impl GeoJsonParseTask {
    pub fn new(task_id: String, data: String, source_url: Option<String>) -> Self {
        Self {
            task_id,
            data,
            source_url,
        }
    }
}

impl BackgroundTask for GeoJsonParseTask {
    fn execute(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(feature = "debug")]
            info!("Starting GeoJSON parse for task: {}", self.task_id);

            let data = self.data.clone();
            let source = self
                .source_url
                .clone()
                .unwrap_or_else(|| "inline".to_string());

            // Parse GeoJSON on a blocking thread pool to avoid blocking the async runtime
            #[cfg(feature = "tokio-runtime")]
            let result = tokio::task::spawn_blocking(move || {
                #[cfg(feature = "debug")]
                debug!("Parsing GeoJSON data from source: {}", source);

                // Try to parse as GeoJSON
                match serde_json::from_str::<GeoJson>(&data) {
                    Ok(geojson) => {
                        #[cfg(feature = "debug")]
                        debug!("Successfully parsed GeoJSON");

                        let layer = GeoJsonLayer::from_geojson(geojson);
                        Ok(Box::new(layer) as Box<dyn std::any::Any + Send>)
                    }
                    Err(e) => {
                        #[cfg(feature = "debug")]
                        error!("Failed to parse GeoJSON: {}", e);
                        Err(Box::new(crate::Error::ParseError(format!(
                            "GeoJSON parse error: {}",
                            e
                        )))
                            as Box<dyn std::error::Error + Send + Sync>)
                    }
                }
            })
            .await;

            #[cfg(not(feature = "tokio-runtime"))]
            let result = {
                #[cfg(feature = "debug")]
                debug!("Parsing GeoJSON data from source: {}", source);

                // Parse synchronously when tokio is not available
                match serde_json::from_str::<GeoJson>(&data) {
                    Ok(geojson) => {
                        #[cfg(feature = "debug")]
                        debug!("Successfully parsed GeoJSON");

                        let layer = GeoJsonLayer::from_geojson(geojson);
                        Ok(Box::new(layer) as Box<dyn std::any::Any + Send>)
                    }
                    Err(e) => {
                        #[cfg(feature = "debug")]
                        error!("Failed to parse GeoJSON: {}", e);
                        Err(Box::new(crate::Error::ParseError(format!(
                            "GeoJSON parse error: {}",
                            e
                        )))
                            as Box<dyn std::error::Error + Send + Sync>)
                    }
                }
            };

            match result {
                Ok(layer_result) => layer_result,
                Err(e) => {
                    #[cfg(feature = "debug")]
                    error!("Background GeoJSON parsing failed: {}", e);

                    Err(Box::new(crate::Error::Plugin(format!(
                        "Background task error: {}",
                        e
                    )))
                        as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        TaskPriority::Normal
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimate based on data length
        let base_ms = 50; // Base parsing time
        let data_factor = (self.data.len() / 1024).min(1000); // 1ms per KB, capped at 1s
        std::time::Duration::from_millis(base_ms + data_factor as u64)
    }
}

/// Task for loading GeoJSON data from a URL in the background
pub struct GeoJsonLoadTask {
    pub task_id: TaskId,
    pub url: String,
}

impl GeoJsonLoadTask {
    pub fn new(task_id: String, url: String) -> Self {
        Self { task_id, url }
    }
}

impl BackgroundTask for GeoJsonLoadTask {
    fn execute(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(feature = "debug")]
            info!(
                "Loading GeoJSON from URL: {} (task: {})",
                self.url, self.task_id
            );

            // Download the data
            match reqwest::get(&self.url).await {
                Ok(response) => {
                    match response.text().await {
                        Ok(data) => {
                            #[cfg(feature = "debug")]
                            debug!("Downloaded {} bytes from {}", data.len(), self.url);

                            // Parse the downloaded data
                            let parse_task = GeoJsonParseTask::new(
                                format!("{}_parse", self.task_id),
                                data,
                                Some(self.url.clone()),
                            );

                            parse_task.execute().await
                        }
                        Err(e) => {
                            #[cfg(feature = "debug")]
                            error!("Failed to read response text from {}: {}", self.url, e);
                            Err(Box::new(crate::Error::Network(e))
                                as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                }
                Err(e) => {
                    #[cfg(feature = "debug")]
                    error!("Failed to load from {}: {}", self.url, e);
                    Err(Box::new(crate::Error::Network(e))
                        as Box<dyn std::error::Error + Send + Sync>)
                }
            }
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        TaskPriority::Normal
    }

    fn estimated_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(1000) // 1 second estimate for network + parsing
    }
}

/// Task for calculating bounds of GeoJSON features
pub struct CalculateBoundsTask {
    task_id: String,
    features: Vec<GeoJsonFeature>,
    priority: TaskPriority,
}

impl CalculateBoundsTask {
    pub fn new(task_id: String, features: Vec<GeoJsonFeature>) -> Self {
        Self {
            task_id,
            features,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl BackgroundTask for CalculateBoundsTask {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let features = self.features.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut bounds: Option<LatLngBounds> = None;

                for feature in &features {
                    if let Some(geometry) = &feature.geometry {
                        if let Some(geom_bounds) =
                            crate::data::geojson::GeoJsonLayer::geometry_bounds(geometry)
                        {
                            if let Some(ref mut b) = bounds {
                                b.extend(&geom_bounds.south_west);
                                b.extend(&geom_bounds.north_east);
                            } else {
                                bounds = Some(geom_bounds);
                            }
                        }
                    }
                }

                bounds
            })
            .await
            .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))?;

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimate based on number of features
        let base_time = std::time::Duration::from_millis(10);
        let feature_factor = (self.features.len() / 100).max(1) as u32;
        base_time * feature_factor.min(20) // Cap at 200ms
    }
}

/// Task for filtering GeoJSON features based on a predicate
pub struct FilterFeaturesTask {
    task_id: String,
    features: Vec<GeoJsonFeature>,
    filter_expression: String, // Simple string-based filter for now
    priority: TaskPriority,
}

impl FilterFeaturesTask {
    pub fn new(task_id: String, features: Vec<GeoJsonFeature>, filter_expression: String) -> Self {
        Self {
            task_id,
            features,
            filter_expression,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl BackgroundTask for FilterFeaturesTask {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            let features = self.features.clone();
            let filter_expr = self.filter_expression.clone();

            let result = tokio::task::spawn_blocking(move || {
                // Simple property-based filtering
                // Format: "property_name=value" or "property_name>value" etc.
                let filtered: Vec<GeoJsonFeature> = features
                    .into_iter()
                    .filter(|feature| {
                        if let Some(properties) = &feature.properties {
                            // Simple string contains check for now
                            // In a real implementation, you'd want a proper expression parser
                            if filter_expr.contains('=') {
                                let parts: Vec<&str> = filter_expr.split('=').collect();
                                if parts.len() == 2 {
                                    if let Some(value) = properties.get(parts[0]) {
                                        return value.as_str().is_some_and(|v| v == parts[1]);
                                    }
                                }
                            }
                        }
                        false
                    })
                    .collect();

                filtered
            })
            .await
            .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))?;

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Estimate based on number of features and filter complexity
        let base_time = std::time::Duration::from_millis(5);
        let feature_factor = (self.features.len() / 1000).max(1) as u32;
        base_time * feature_factor.min(50) // Cap at 250ms
    }
}

/// Convenience functions for creating common GeoJSON background tasks
pub mod tasks {
    use super::*;

    /// Create a task to parse GeoJSON from a URL
    pub fn parse_geojson_from_url(task_id: String, url: String) -> ParseGeoJsonFromUrlTask {
        ParseGeoJsonFromUrlTask::new(task_id, url)
    }

    /// Create a task to parse GeoJSON from a string
    pub fn parse_geojson_from_str(task_id: String, geojson_str: String) -> GeoJsonParseTask {
        GeoJsonParseTask::new(task_id, geojson_str, None)
    }

    /// Create a task to calculate bounds
    pub fn calculate_bounds(task_id: String, features: Vec<GeoJsonFeature>) -> CalculateBoundsTask {
        CalculateBoundsTask::new(task_id, features)
    }

    /// Create a task to filter features
    pub fn filter_features(
        task_id: String,
        features: Vec<GeoJsonFeature>,
        filter_expression: String,
    ) -> FilterFeaturesTask {
        FilterFeaturesTask::new(task_id, features, filter_expression)
    }
}

/// Task for downloading and parsing GeoJSON from a URL
pub struct ParseGeoJsonFromUrlTask {
    task_id: String,
    url: String,
    priority: TaskPriority,
}

impl ParseGeoJsonFromUrlTask {
    pub fn new(task_id: String, url: String) -> Self {
        Self {
            task_id,
            url,
            priority: TaskPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl BackgroundTask for ParseGeoJsonFromUrlTask {
    fn execute(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Box<dyn std::any::Any + Send>>> + Send + '_>,
    > {
        Box::pin(async move {
            // Download the GeoJSON
            let response = reqwest::get(&self.url)
                .await
                .map_err(crate::Error::Network)?;

            let geojson_str = response.text().await.map_err(crate::Error::Network)?;

            // Parse in background thread
            let result = tokio::task::spawn_blocking(move || {
                serde_json::from_str::<GeoJson>(&geojson_str)
                    .map_err(|e| crate::Error::ParseError(format!("Invalid GeoJSON: {}", e)))
            })
            .await
            .map_err(|e| crate::Error::Plugin(format!("Task execution failed: {}", e)))??;

            Ok(Box::new(result) as Box<dyn std::any::Any + Send>)
        })
    }

    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn priority(&self) -> TaskPriority {
        self.priority
    }

    fn estimated_duration(&self) -> std::time::Duration {
        // Network + parsing time estimate
        std::time::Duration::from_millis(1000) // 1 second estimate
    }
}
