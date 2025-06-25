use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported animation data formats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub enum DataFormat {
    /// JSON format for human-readable animation data
    #[default]
    Json,
    /// Binary format for efficient storage and transmission
    Binary,
    /// Custom format with specific encoding
    Custom(String),
}

/// Animation keyframe data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeData {
    /// Time position of the keyframe (in seconds)
    pub time: f64,
    /// Values at this keyframe (property name -> value)
    pub values: HashMap<String, serde_json::Value>,
    /// Easing function to use for this keyframe
    pub easing: Option<String>,
    /// Interpolation method
    pub interpolation: Option<String>,
}

/// Animation track data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    /// Track name/identifier
    pub name: String,
    /// Property this track animates
    pub property: String,
    /// Keyframes in this track
    pub keyframes: Vec<KeyframeData>,
    /// Whether this track is enabled
    pub enabled: bool,
}

/// Complete animation data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationData {
    /// Animation name/identifier
    pub name: String,
    /// Duration of the animation in seconds
    pub duration: f64,
    /// Animation tracks
    pub tracks: Vec<TrackData>,
    /// Metadata for the animation
    pub metadata: HashMap<String, serde_json::Value>,
    /// Whether the animation loops
    pub loop_animation: bool,
    /// Whether the animation plays in reverse when looping
    pub ping_pong: bool,
}

impl DataFormat {
    /// Create a new DataFormat instance
    pub fn new() -> Self {
        DataFormat::Json
    }

    /// Create a custom format with specific encoding
    pub fn custom(encoding: String) -> Self {
        DataFormat::Custom(encoding)
    }

    /// Serialize animation data to the specified format
    pub fn serialize(&self, data: &AnimationData) -> Result<Vec<u8>> {
        match self {
            DataFormat::Json => {
                let json = serde_json::to_string_pretty(data)?;
                Ok(json.into_bytes())
            }
            DataFormat::Binary => {
                let bytes = bincode::serialize(data)?;
                Ok(bytes)
            }
            DataFormat::Custom(encoding) => {
                // For custom formats, we'll use JSON as base and add encoding metadata
                let mut custom_data = HashMap::new();
                custom_data.insert(
                    "encoding".to_string(),
                    serde_json::Value::String(encoding.clone()),
                );
                custom_data.insert("data".to_string(), serde_json::to_value(data)?);

                let json = serde_json::to_string(&custom_data)?;
                Ok(json.into_bytes())
            }
        }
    }

    /// Deserialize animation data from the specified format
    pub fn deserialize(&self, bytes: &[u8]) -> Result<AnimationData> {
        match self {
            DataFormat::Json => {
                let json_str = std::str::from_utf8(bytes)?;
                let data: AnimationData = serde_json::from_str(json_str)?;
                Ok(data)
            }
            DataFormat::Binary => {
                let data: AnimationData = bincode::deserialize(bytes)?;
                Ok(data)
            }
            DataFormat::Custom(_encoding) => {
                // For custom formats, try to parse as JSON with encoding metadata
                let json_str = std::str::from_utf8(bytes)?;
                let custom_data: HashMap<String, serde_json::Value> =
                    serde_json::from_str(json_str)?;

                if let Some(data_value) = custom_data.get("data") {
                    let data: AnimationData = serde_json::from_value(data_value.clone())?;
                    Ok(data)
                } else {
                    // Fallback to direct JSON parsing
                    let data: AnimationData = serde_json::from_str(json_str)?;
                    Ok(data)
                }
            }
        }
    }

    /// Get the file extension for this format
    pub fn file_extension(&self) -> &str {
        match self {
            DataFormat::Json => "json",
            DataFormat::Binary => "bin",
            DataFormat::Custom(_) => "custom",
        }
    }

    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &str {
        match self {
            DataFormat::Json => "application/json",
            DataFormat::Binary => "application/octet-stream",
            DataFormat::Custom(_) => "application/x-custom-animation",
        }
    }

    /// Validate animation data
    pub fn validate(&self, data: &AnimationData) -> Result<()> {
        // Check basic validation
        if data.duration <= 0.0 {
            return Err("Animation duration must be positive".into());
        }

        if data.tracks.is_empty() {
            return Err("Animation must have at least one track".into());
        }

        // Validate each track
        for track in &data.tracks {
            if track.keyframes.is_empty() {
                return Err(
                    format!("Track '{}' must have at least one keyframe", track.name).into(),
                );
            }

            // Check that keyframes are sorted by time
            for i in 1..track.keyframes.len() {
                if track.keyframes[i].time < track.keyframes[i - 1].time {
                    return Err(format!(
                        "Keyframes in track '{}' must be sorted by time",
                        track.name
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    /// Convert animation data to a different format
    pub fn convert(&self, data: &AnimationData, target_format: &DataFormat) -> Result<Vec<u8>> {
        target_format.serialize(data)
    }
}


impl AnimationData {
    /// Create a new animation data structure
    pub fn new(name: String, duration: f64) -> Self {
        Self {
            name,
            duration,
            tracks: Vec::new(),
            metadata: HashMap::new(),
            loop_animation: false,
            ping_pong: false,
        }
    }

    /// Add a track to the animation
    pub fn add_track(&mut self, track: TrackData) {
        self.tracks.push(track);
    }

    /// Remove a track by name
    pub fn remove_track(&mut self, track_name: &str) -> Option<TrackData> {
        if let Some(index) = self.tracks.iter().position(|t| t.name == track_name) {
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }

    /// Get a track by name
    pub fn get_track(&self, track_name: &str) -> Option<&TrackData> {
        self.tracks.iter().find(|t| t.name == track_name)
    }

    /// Get a mutable reference to a track by name
    pub fn get_track_mut(&mut self, track_name: &str) -> Option<&mut TrackData> {
        self.tracks.iter_mut().find(|t| t.name == track_name)
    }

    /// Set metadata
    pub fn set_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Calculate the total number of keyframes across all tracks
    pub fn total_keyframes(&self) -> usize {
        self.tracks.iter().map(|t| t.keyframes.len()).sum()
    }

    /// Get the minimum and maximum time values across all tracks
    pub fn time_range(&self) -> (f64, f64) {
        let mut min_time = f64::INFINITY;
        let mut max_time = f64::NEG_INFINITY;

        for track in &self.tracks {
            for keyframe in &track.keyframes {
                min_time = min_time.min(keyframe.time);
                max_time = max_time.max(keyframe.time);
            }
        }

        if min_time == f64::INFINITY {
            (0.0, 0.0)
        } else {
            (min_time, max_time)
        }
    }
}

impl TrackData {
    /// Create a new track
    pub fn new(name: String, property: String) -> Self {
        Self {
            name,
            property,
            keyframes: Vec::new(),
            enabled: true,
        }
    }

    /// Add a keyframe to the track
    pub fn add_keyframe(&mut self, keyframe: KeyframeData) {
        self.keyframes.push(keyframe);
        // Sort by time
        self.keyframes
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    /// Remove a keyframe at the specified time
    pub fn remove_keyframe(&mut self, time: f64) -> Option<KeyframeData> {
        if let Some(index) = self
            .keyframes
            .iter()
            .position(|k| (k.time - time).abs() < f64::EPSILON)
        {
            Some(self.keyframes.remove(index))
        } else {
            None
        }
    }

    /// Get the keyframe at or before the specified time
    pub fn get_keyframe_at(&self, time: f64) -> Option<&KeyframeData> {
        self.keyframes
            .iter()
            .filter(|k| k.time <= time)
            .max_by(|a, b| a.time.partial_cmp(&b.time).unwrap())
    }

    /// Get the next keyframe after the specified time
    pub fn get_next_keyframe(&self, time: f64) -> Option<&KeyframeData> {
        self.keyframes
            .iter()
            .filter(|k| k.time > time)
            .min_by(|a, b| a.time.partial_cmp(&b.time).unwrap())
    }
}

impl KeyframeData {
    /// Create a new keyframe
    pub fn new(time: f64) -> Self {
        Self {
            time,
            values: HashMap::new(),
            easing: None,
            interpolation: None,
        }
    }

    /// Set a value for a property
    pub fn set_value(&mut self, property: String, value: serde_json::Value) {
        self.values.insert(property, value);
    }

    /// Get a value for a property
    pub fn get_value(&self, property: &str) -> Option<&serde_json::Value> {
        self.values.get(property)
    }

    /// Set the easing function
    pub fn set_easing(&mut self, easing: String) {
        self.easing = Some(easing);
    }

    /// Set the interpolation method
    pub fn set_interpolation(&mut self, interpolation: String) {
        self.interpolation = Some(interpolation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_format_serialization() {
        let format = DataFormat::Json;
        let mut animation = AnimationData::new("test_animation".to_string(), 5.0);

        let mut track = TrackData::new("position".to_string(), "transform".to_string());
        let mut keyframe = KeyframeData::new(0.0);
        keyframe.set_value(
            "x".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        keyframe.set_value(
            "y".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        track.add_keyframe(keyframe);

        let mut keyframe2 = KeyframeData::new(5.0);
        keyframe2.set_value(
            "x".to_string(),
            serde_json::Value::Number(serde_json::Number::from(100)),
        );
        keyframe2.set_value(
            "y".to_string(),
            serde_json::Value::Number(serde_json::Number::from(100)),
        );
        track.add_keyframe(keyframe2);

        animation.add_track(track);

        let bytes = format.serialize(&animation).unwrap();
        let deserialized = format.deserialize(&bytes).unwrap();

        assert_eq!(animation.name, deserialized.name);
        assert_eq!(animation.duration, deserialized.duration);
        assert_eq!(animation.tracks.len(), deserialized.tracks.len());
    }

    #[test]
    fn test_animation_data_operations() {
        let mut animation = AnimationData::new("test".to_string(), 10.0);

        let track = TrackData::new("test_track".to_string(), "position".to_string());
        animation.add_track(track);

        assert_eq!(animation.tracks.len(), 1);
        assert!(animation.get_track("test_track").is_some());

        let removed = animation.remove_track("test_track");
        assert!(removed.is_some());
        assert_eq!(animation.tracks.len(), 0);
    }
}
