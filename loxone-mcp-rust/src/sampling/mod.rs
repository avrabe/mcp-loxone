//! MCP Sampling protocol implementation
//!
//! This module implements the MCP sampling protocol which allows servers to request
//! LLM completions from clients. This is the proper way to integrate LLMs with MCP
//! instead of making direct API calls.

pub mod client;
pub mod config;
pub mod executor;
pub mod protocol;
// pub mod provider; // Removed - compilation issues
pub mod response_parser;
pub mod service;

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP Sampling request message content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl SamplingMessageContent {
    /// Create text content
    pub fn text<S: Into<String>>(text: S) -> Self {
        Self {
            content_type: "text".to_string(),
            text: Some(text.into()),
            data: None,
            mime_type: None,
        }
    }

    /// Create image content
    pub fn image<S: Into<String>>(data: S, mime_type: S) -> Self {
        Self {
            content_type: "image".to_string(),
            text: None,
            data: Some(data.into()),
            mime_type: Some(mime_type.into()),
        }
    }
}

/// MCP Sampling request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: String,
    pub content: SamplingMessageContent,
}

impl SamplingMessage {
    /// Create user message
    pub fn user<S: Into<String>>(text: S) -> Self {
        Self {
            role: "user".to_string(),
            content: SamplingMessageContent::text(text),
        }
    }

    /// Create assistant message
    pub fn assistant<S: Into<String>>(text: S) -> Self {
        Self {
            role: "assistant".to_string(),
            content: SamplingMessageContent::text(text),
        }
    }
}

/// Model preference hints for sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHint {
    pub name: String,
}

/// Model preferences for sampling requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f32>,
}

impl Default for ModelPreferences {
    fn default() -> Self {
        Self {
            hints: Some(vec![
                ModelHint {
                    name: "claude-3-sonnet".to_string(),
                },
                ModelHint {
                    name: "gpt-4".to_string(),
                },
            ]),
            cost_priority: Some(0.3),
            speed_priority: Some(0.5),
            intelligence_priority: Some(0.8),
        }
    }
}

/// MCP Sampling request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    pub messages: Vec<SamplingMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl SamplingRequest {
    /// Create a new sampling request
    pub fn new(messages: Vec<SamplingMessage>) -> Self {
        Self {
            messages,
            system_prompt: None,
            model_preferences: Some(ModelPreferences::default()),
            max_tokens: Some(1000),
            temperature: Some(0.7),
            stop_sequences: None,
            include_context: None,
            metadata: None,
        }
    }

    /// Set system prompt
    pub fn with_system_prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set model preferences
    pub fn with_model_preferences(mut self, preferences: ModelPreferences) -> Self {
        self.model_preferences = Some(preferences);
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        if self.metadata.is_none() {
            self.metadata = Some(HashMap::new());
        }
        self.metadata.as_mut().unwrap().insert(key, value);
        self
    }
}

/// MCP Sampling response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingResponse {
    pub model: String,
    pub stop_reason: String,
    pub role: String,
    pub content: SamplingMessageContent,
}

/// Sampling request builder for home automation scenarios
pub struct AutomationSamplingBuilder {
    pub system_prompt: String,
    context_data: HashMap<String, serde_json::Value>,
}

impl AutomationSamplingBuilder {
    /// Create new builder with home automation system prompt
    pub fn new() -> Self {
        Self {
            system_prompt: "You are an intelligent home automation assistant for a Loxone system. \
                           Analyze the current home state and provide specific, actionable automation recommendations. \
                           Consider user preferences, time of day, weather, and energy efficiency. \
                           Respond with clear device control suggestions using available Loxone commands.".to_string(),
            context_data: HashMap::new(),
        }
    }

    /// Add room data to context
    pub fn with_rooms(mut self, rooms_data: serde_json::Value) -> Self {
        self.context_data.insert("rooms".to_string(), rooms_data);
        self
    }

    /// Add device data to context
    pub fn with_devices(mut self, devices_data: serde_json::Value) -> Self {
        self.context_data
            .insert("devices".to_string(), devices_data);
        self
    }

    /// Add sensor data to context
    pub fn with_sensors(mut self, sensors_data: serde_json::Value) -> Self {
        self.context_data
            .insert("sensors".to_string(), sensors_data);
        self
    }

    /// Add weather data to context
    pub fn with_weather(mut self, weather_data: serde_json::Value) -> Self {
        self.context_data
            .insert("weather".to_string(), weather_data);
        self
    }

    /// Build sampling request for a specific automation scenario
    pub fn build_cozy_request(
        &self,
        time_of_day: &str,
        weather: &str,
        mood: &str,
    ) -> Result<SamplingRequest> {
        let context_text = self.build_context_text()?;

        let user_message = SamplingMessage::user(format!(
            "I want to make my home cozy. It's {} and the weather is {}. I'm looking for a {} atmosphere. \
             Please analyze the current state and suggest optimal settings for lighting, temperature, and blinds.\n\n\
             Current Home State:\n{}",
            time_of_day, weather, mood, context_text
        ));

        let request = SamplingRequest::new(vec![user_message])
            .with_system_prompt(&self.system_prompt)
            .with_max_tokens(800)
            .with_temperature(0.7)
            .with_metadata(
                "scenario".to_string(),
                serde_json::Value::String("cozy_home".to_string()),
            )
            .with_metadata(
                "time_of_day".to_string(),
                serde_json::Value::String(time_of_day.to_string()),
            )
            .with_metadata(
                "weather".to_string(),
                serde_json::Value::String(weather.to_string()),
            )
            .with_metadata(
                "mood".to_string(),
                serde_json::Value::String(mood.to_string()),
            );

        Ok(request)
    }

    /// Build sampling request for event preparation
    pub fn build_event_request(
        &self,
        event_type: &str,
        room: Option<&str>,
        duration: Option<&str>,
        guest_count: Option<&str>,
    ) -> Result<SamplingRequest> {
        let context_text = self.build_context_text()?;

        let mut event_description = format!("I'm preparing for a {}", event_type);
        if let Some(room) = room {
            event_description.push_str(&format!(" in the {}", room));
        }
        if let Some(duration) = duration {
            event_description.push_str(&format!(" lasting {}", duration));
        }
        if let Some(guest_count) = guest_count {
            event_description.push_str(&format!(" with {} guests", guest_count));
        }

        let user_message = SamplingMessage::user(format!(
            "{}. Please suggest the optimal home automation settings.\n\n\
             Current Home State:\n{}",
            event_description, context_text
        ));

        let mut request = SamplingRequest::new(vec![user_message])
            .with_system_prompt(&self.system_prompt)
            .with_max_tokens(800)
            .with_temperature(0.7)
            .with_metadata(
                "scenario".to_string(),
                serde_json::Value::String("event_preparation".to_string()),
            )
            .with_metadata(
                "event_type".to_string(),
                serde_json::Value::String(event_type.to_string()),
            );

        if let Some(room) = room {
            request = request.with_metadata(
                "room".to_string(),
                serde_json::Value::String(room.to_string()),
            );
        }

        Ok(request)
    }

    /// Build context text from available data
    pub fn build_context_text(&self) -> Result<String> {
        let mut context_parts = Vec::new();

        if let Some(rooms) = self.context_data.get("rooms") {
            context_parts.push(format!(
                "Available Rooms:\n{}",
                serde_json::to_string_pretty(rooms).unwrap_or_default()
            ));
        }

        if let Some(devices) = self.context_data.get("devices") {
            context_parts.push(format!(
                "Lighting Devices:\n{}",
                serde_json::to_string_pretty(devices).unwrap_or_default()
            ));
        }

        if let Some(sensors) = self.context_data.get("sensors") {
            context_parts.push(format!(
                "Temperature Readings:\n{}",
                serde_json::to_string_pretty(sensors).unwrap_or_default()
            ));
        }

        if let Some(weather) = self.context_data.get("weather") {
            context_parts.push(format!(
                "Weather Data:\n{}",
                serde_json::to_string_pretty(weather).unwrap_or_default()
            ));
        }

        Ok(context_parts.join("\n\n"))
    }
}

impl Default for AutomationSamplingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a simple text sampling request
pub fn create_text_request<S: Into<String>>(
    user_message: S,
    system_prompt: Option<S>,
) -> SamplingRequest {
    let messages = vec![SamplingMessage::user(user_message)];

    let mut request = SamplingRequest::new(messages);
    if let Some(prompt) = system_prompt {
        request = request.with_system_prompt(prompt);
    }

    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampling_message_creation() {
        let user_msg = SamplingMessage::user("Hello world");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content.content_type, "text");
        assert_eq!(user_msg.content.text.unwrap(), "Hello world");
    }

    #[test]
    fn test_sampling_request_builder() {
        let request = SamplingRequest::new(vec![SamplingMessage::user("Test")])
            .with_system_prompt("System prompt")
            .with_max_tokens(500)
            .with_temperature(0.5);

        assert_eq!(request.system_prompt.unwrap(), "System prompt");
        assert_eq!(request.max_tokens.unwrap(), 500);
        assert_eq!(request.temperature.unwrap(), 0.5);
    }

    #[test]
    fn test_automation_sampling_builder() {
        let builder =
            AutomationSamplingBuilder::new().with_rooms(serde_json::json!({"living_room": "test"}));

        let request = builder
            .build_cozy_request("evening", "rainy", "relaxing")
            .unwrap();
        assert!(request.system_prompt.is_some());
        assert_eq!(request.messages.len(), 1);
        assert!(request.messages[0]
            .content
            .text
            .as_ref()
            .unwrap()
            .contains("cozy"));
    }
}
