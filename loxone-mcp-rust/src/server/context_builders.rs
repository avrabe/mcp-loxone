//! Context builders for aggregating MCP resources into LLM-optimized prompts
//!
//! This module provides utilities for building comprehensive context from multiple
//! MCP resources, optimized for LLM processing and home automation intelligence.

use crate::{
    error::{LoxoneError, Result},
    server::{
        resources::{ResourceHandler, ResourceManager},
        LoxoneMcpServer,
    },
};
use chrono::{Datelike, Timelike};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::debug;

/// Context builder for aggregating multiple resources into LLM-optimized format
pub struct ContextBuilder<'a> {
    server: &'a LoxoneMcpServer,
    resource_manager: ResourceManager,
    included_resources: Vec<String>,
    context_type: ContextType,
    metadata: HashMap<String, Value>,
}

/// Types of context for different LLM use cases
#[derive(Debug, Clone, Copy)]
pub enum ContextType {
    /// General home automation context
    General,
    /// Energy optimization focused context
    Energy,
    /// Security and safety focused context
    Security,
    /// Comfort and convenience focused context
    Comfort,
    /// Entertainment and ambiance focused context
    Entertainment,
    /// Comprehensive all-systems context
    Comprehensive,
}

impl ContextType {
    /// Get the default resources for this context type
    pub fn default_resources(&self) -> Vec<&'static str> {
        match self {
            ContextType::General => vec![
                "loxone://rooms",
                "loxone://system/status",
                "loxone://system/capabilities",
            ],
            ContextType::Energy => vec![
                "loxone://devices/category/lighting",
                "loxone://devices/category/climate",
                "loxone://system/status",
                "loxone://system/capabilities",
            ],
            ContextType::Security => vec![
                "loxone://sensors/door-window",
                "loxone://system/status",
                "loxone://devices/category/lighting",
            ],
            ContextType::Comfort => vec![
                "loxone://rooms",
                "loxone://devices/category/climate",
                "loxone://sensors/temperature",
                "loxone://system/capabilities",
            ],
            ContextType::Entertainment => vec![
                "loxone://audio/zones",
                "loxone://audio/sources",
                "loxone://devices/category/lighting",
                "loxone://rooms",
            ],
            ContextType::Comprehensive => vec![
                "loxone://rooms",
                "loxone://devices/all",
                "loxone://system/status",
                "loxone://system/capabilities",
                "loxone://system/categories",
                "loxone://audio/zones",
                "loxone://sensors/door-window",
                "loxone://sensors/temperature",
            ],
        }
    }

    /// Get suggested automation focus areas for this context
    pub fn automation_focus(&self) -> Vec<&'static str> {
        match self {
            ContextType::General => {
                vec!["Scene creation", "Basic automation", "Device coordination"]
            }
            ContextType::Energy => vec![
                "Energy optimization",
                "Usage pattern analysis",
                "Efficiency improvements",
                "Cost reduction strategies",
            ],
            ContextType::Security => vec![
                "Security automation",
                "Intrusion detection",
                "Presence simulation",
                "Emergency responses",
            ],
            ContextType::Comfort => vec![
                "Climate optimization",
                "Circadian lighting",
                "Personalized environments",
                "Wellness automation",
            ],
            ContextType::Entertainment => vec![
                "Audio-visual scenes",
                "Entertainment automation",
                "Ambiance creation",
                "Multi-zone coordination",
            ],
            ContextType::Comprehensive => vec![
                "Whole-home automation",
                "System integration",
                "Advanced orchestration",
                "Intelligent coordination",
            ],
        }
    }
}

impl<'a> ContextBuilder<'a> {
    /// Create a new context builder
    pub fn new(server: &'a LoxoneMcpServer, context_type: ContextType) -> Self {
        Self {
            server,
            resource_manager: ResourceManager::new(),
            included_resources: context_type
                .default_resources()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            context_type,
            metadata: HashMap::new(),
        }
    }

    /// Add additional resources to include in the context
    pub fn with_resources(mut self, resources: Vec<&str>) -> Self {
        for resource in resources {
            if !self.included_resources.contains(&resource.to_string()) {
                self.included_resources.push(resource.to_string());
            }
        }
        self
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: &str, value: Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    /// Add user query context
    pub fn with_user_query(self, query: &str) -> Self {
        self.with_metadata("user_query", json!(query))
    }

    /// Add temporal context (time of day, season, etc.)
    pub fn with_temporal_context(self, time_context: TemporalContext) -> Self {
        self.with_metadata("temporal_context", json!(time_context))
    }

    /// Build the aggregated context
    pub async fn build(self) -> Result<LlmContext> {
        debug!(
            "Building LLM context with {} resources",
            self.included_resources.len()
        );

        let mut resource_data = HashMap::new();
        let mut failed_resources = Vec::new();

        // Fetch all requested resources
        for resource_uri in &self.included_resources {
            match self.fetch_resource(resource_uri).await {
                Ok(data) => {
                    let key = self.resource_key_from_uri(resource_uri);
                    resource_data.insert(key, data);
                }
                Err(e) => {
                    debug!("Failed to fetch resource {}: {}", resource_uri, e);
                    failed_resources.push(resource_uri.clone());
                }
            }
        }

        // Build automation insights based on context type and available data
        let automation_insights = self.build_automation_insights(&resource_data).await?;

        // Build optimization opportunities
        let optimization_opportunities = self
            .build_optimization_opportunities(&resource_data)
            .await?;

        Ok(LlmContext {
            context_type: self.context_type,
            timestamp: chrono::Utc::now(),
            resources: resource_data,
            automation_insights,
            optimization_opportunities,
            metadata: self.metadata,
            failed_resources,
            automation_focus: self
                .context_type
                .automation_focus()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
    }

    /// Fetch a single resource
    async fn fetch_resource(&self, uri: &str) -> Result<Value> {
        let context = self.resource_manager.parse_uri(uri)?;
        let content = ResourceHandler::read_resource(self.server, context).await?;
        Ok(content.data)
    }

    /// Convert resource URI to a clean key name
    fn resource_key_from_uri(&self, uri: &str) -> String {
        uri.replace("loxone://", "")
            .replace("/", "_")
            .replace("-", "_")
    }

    /// Build automation insights from aggregated resources
    async fn build_automation_insights(
        &self,
        resources: &HashMap<String, Value>,
    ) -> Result<AutomationInsights> {
        let mut insights = AutomationInsights::default();

        // Analyze device capabilities
        if let Some(capabilities) = resources.get("system_capabilities") {
            insights.device_capabilities = self.analyze_device_capabilities(capabilities);
        }

        // Analyze room potential
        if let Some(rooms) = resources.get("rooms") {
            insights.room_automation_potential = self.analyze_room_potential(rooms, resources);
        }

        // Analyze integration opportunities
        insights.integration_opportunities = self.analyze_integration_opportunities(resources);

        // Context-specific insights
        match self.context_type {
            ContextType::Energy => {
                insights.energy_insights = self.analyze_energy_opportunities(resources);
            }
            ContextType::Security => {
                insights.security_insights = self.analyze_security_opportunities(resources);
            }
            ContextType::Comfort => {
                insights.comfort_insights = self.analyze_comfort_opportunities(resources);
            }
            _ => {}
        }

        Ok(insights)
    }

    /// Build optimization opportunities
    async fn build_optimization_opportunities(
        &self,
        resources: &HashMap<String, Value>,
    ) -> Result<Vec<OptimizationOpportunity>> {
        let mut opportunities = Vec::new();

        // Device coordination opportunities
        if let Some(devices) = resources.get("devices_all") {
            opportunities.extend(self.find_device_coordination_opportunities(devices));
        }

        // Energy optimization opportunities
        if resources.contains_key("devices_category_lighting")
            || resources.contains_key("devices_category_climate")
        {
            opportunities.extend(self.find_energy_opportunities(resources));
        }

        // Scene creation opportunities
        if let Some(rooms) = resources.get("rooms") {
            opportunities.extend(self.find_scene_opportunities(rooms, resources));
        }

        Ok(opportunities)
    }

    // Analysis helper methods
    fn analyze_device_capabilities(&self, capabilities: &Value) -> DeviceCapabilities {
        DeviceCapabilities {
            lighting_count: capabilities["lighting_count"].as_u64().unwrap_or(0),
            blind_count: capabilities["blind_count"].as_u64().unwrap_or(0),
            sensor_count: capabilities["sensor_count"].as_u64().unwrap_or(0),
            climate_count: capabilities["climate_count"].as_u64().unwrap_or(0),
            has_audio: capabilities["has_audio"].as_bool().unwrap_or(false),
            automation_score: self.calculate_automation_score(capabilities),
        }
    }

    fn analyze_room_potential(
        &self,
        rooms: &Value,
        _resources: &HashMap<String, Value>,
    ) -> Vec<RoomPotential> {
        let mut room_potentials = Vec::new();

        if let Some(rooms_array) = rooms["rooms"].as_array() {
            for room in rooms_array {
                if let Some(room_name) = room["name"].as_str() {
                    let device_count = room["device_count"].as_u64().unwrap_or(0);
                    let potential_score = (device_count as f64 / 10.0).min(1.0);

                    room_potentials.push(RoomPotential {
                        name: room_name.to_string(),
                        device_count,
                        automation_potential: potential_score,
                        suggested_improvements: self.suggest_room_improvements(room),
                    });
                }
            }
        }

        room_potentials
    }

    fn analyze_integration_opportunities(&self, resources: &HashMap<String, Value>) -> Vec<String> {
        let mut opportunities = Vec::new();

        let has_lighting = resources.contains_key("devices_category_lighting");
        let has_climate = resources.contains_key("devices_category_climate");
        let has_audio = resources.contains_key("audio_zones");
        let has_sensors = resources.contains_key("sensors_door_window");

        if has_lighting && has_sensors {
            opportunities.push("Security lighting automation".to_string());
        }

        if has_lighting && has_climate {
            opportunities.push("Coordinated comfort scenes".to_string());
        }

        if has_audio && has_lighting {
            opportunities.push("Audio-visual entertainment scenes".to_string());
        }

        if has_lighting && has_sensors && has_climate {
            opportunities.push("Comprehensive home automation".to_string());
        }

        opportunities
    }

    fn analyze_energy_opportunities(&self, resources: &HashMap<String, Value>) -> Vec<String> {
        let mut insights = Vec::new();

        if resources.contains_key("devices_category_lighting") {
            insights.push("Implement occupancy-based lighting control".to_string());
            insights.push("Use daylight sensors for automatic dimming".to_string());
        }

        if resources.contains_key("devices_category_climate") {
            insights.push("Coordinate HVAC with solar gain management".to_string());
            insights.push("Implement zone-based temperature control".to_string());
        }

        insights
    }

    fn analyze_security_opportunities(&self, resources: &HashMap<String, Value>) -> Vec<String> {
        let mut insights = Vec::new();

        if resources.contains_key("sensors_door_window") {
            insights.push("Implement intrusion detection automation".to_string());
            insights.push("Create presence simulation routines".to_string());
        }

        if resources.contains_key("devices_category_lighting") {
            insights.push("Coordinate security lighting responses".to_string());
        }

        insights
    }

    fn analyze_comfort_opportunities(&self, resources: &HashMap<String, Value>) -> Vec<String> {
        let mut insights = Vec::new();

        if resources.contains_key("devices_category_climate") {
            insights.push("Implement personalized climate profiles".to_string());
        }

        if resources.contains_key("devices_category_lighting") {
            insights.push("Create circadian rhythm lighting".to_string());
        }

        insights
    }

    fn find_device_coordination_opportunities(
        &self,
        _devices: &Value,
    ) -> Vec<OptimizationOpportunity> {
        vec![OptimizationOpportunity {
            title: "Multi-device Scene Creation".to_string(),
            description: "Coordinate lighting, blinds, and climate for optimal scenes".to_string(),
            impact: Impact::High,
            effort: Effort::Medium,
            category: "coordination".to_string(),
        }]
    }

    fn find_energy_opportunities(
        &self,
        _resources: &HashMap<String, Value>,
    ) -> Vec<OptimizationOpportunity> {
        vec![OptimizationOpportunity {
            title: "Smart Energy Scheduling".to_string(),
            description: "Implement time-based and occupancy-based device control".to_string(),
            impact: Impact::High,
            effort: Effort::Low,
            category: "energy".to_string(),
        }]
    }

    fn find_scene_opportunities(
        &self,
        _rooms: &Value,
        _resources: &HashMap<String, Value>,
    ) -> Vec<OptimizationOpportunity> {
        vec![OptimizationOpportunity {
            title: "Room-specific Automation Scenes".to_string(),
            description: "Create tailored automation scenes for each room's function".to_string(),
            impact: Impact::Medium,
            effort: Effort::Low,
            category: "scenes".to_string(),
        }]
    }

    fn calculate_automation_score(&self, capabilities: &Value) -> f64 {
        let lighting = capabilities["lighting_count"].as_u64().unwrap_or(0) as f64;
        let climate = capabilities["climate_count"].as_u64().unwrap_or(0) as f64;
        let sensors = capabilities["sensor_count"].as_u64().unwrap_or(0) as f64;
        let blinds = capabilities["blind_count"].as_u64().unwrap_or(0) as f64;

        ((lighting * 0.3 + climate * 0.4 + sensors * 0.2 + blinds * 0.1) / 20.0).min(1.0)
    }

    fn suggest_room_improvements(&self, room: &Value) -> Vec<String> {
        let mut suggestions = Vec::new();
        let device_count = room["device_count"].as_u64().unwrap_or(0);

        if device_count < 3 {
            suggestions.push("Consider adding more automation devices".to_string());
        }

        if device_count >= 5 {
            suggestions
                .push("Excellent automation potential - consider advanced scenes".to_string());
        }

        suggestions
    }
}

/// Temporal context for time-aware automation
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TemporalContext {
    pub time_of_day: String,
    pub day_of_week: String,
    pub season: String,
    pub is_weekend: bool,
    pub is_holiday: Option<bool>,
}

impl Default for TemporalContext {
    fn default() -> Self {
        let now = chrono::Utc::now();
        let hour = now.hour();

        let time_of_day = match hour {
            6..=11 => "morning",
            12..=17 => "afternoon",
            18..=21 => "evening",
            _ => "night",
        };

        Self {
            time_of_day: time_of_day.to_string(),
            day_of_week: now.format("%A").to_string(),
            season: "unknown".to_string(), // Would need additional logic
            is_weekend: matches!(now.weekday().number_from_monday(), 6..=7),
            is_holiday: None,
        }
    }
}

/// Complete LLM context with all aggregated data
#[derive(Debug, serde::Serialize)]
pub struct LlmContext {
    pub context_type: ContextType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub resources: HashMap<String, Value>,
    pub automation_insights: AutomationInsights,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub metadata: HashMap<String, Value>,
    pub failed_resources: Vec<String>,
    pub automation_focus: Vec<String>,
}

/// Insights about automation potential and opportunities
#[derive(Debug, Default, serde::Serialize)]
pub struct AutomationInsights {
    pub device_capabilities: DeviceCapabilities,
    pub room_automation_potential: Vec<RoomPotential>,
    pub integration_opportunities: Vec<String>,
    pub energy_insights: Vec<String>,
    pub security_insights: Vec<String>,
    pub comfort_insights: Vec<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct DeviceCapabilities {
    pub lighting_count: u64,
    pub blind_count: u64,
    pub sensor_count: u64,
    pub climate_count: u64,
    pub has_audio: bool,
    pub automation_score: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct RoomPotential {
    pub name: String,
    pub device_count: u64,
    pub automation_potential: f64,
    pub suggested_improvements: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct OptimizationOpportunity {
    pub title: String,
    pub description: String,
    pub impact: Impact,
    pub effort: Effort,
    pub category: String,
}

#[derive(Debug, serde::Serialize)]
pub enum Impact {
    Low,
    Medium,
    High,
}

#[derive(Debug, serde::Serialize)]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl serde::Serialize for ContextType {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            ContextType::General => "general",
            ContextType::Energy => "energy",
            ContextType::Security => "security",
            ContextType::Comfort => "comfort",
            ContextType::Entertainment => "entertainment",
            ContextType::Comprehensive => "comprehensive",
        };
        serializer.serialize_str(s)
    }
}

/// Convenience functions for common context building patterns
impl LlmContext {
    /// Convert to JSON for LLM consumption
    pub fn to_json(&self) -> Result<Value> {
        serde_json::to_value(self)
            .map_err(|e| LoxoneError::invalid_input(format!("Serialization error: {}", e)))
    }

    /// Get a summary suitable for LLM prompts
    pub fn summary(&self) -> String {
        format!(
            "Home automation system with {} devices across {} rooms. \
            Automation score: {:.1}/10. Key opportunities: {}",
            self.automation_insights.device_capabilities.lighting_count
                + self.automation_insights.device_capabilities.climate_count
                + self.automation_insights.device_capabilities.sensor_count,
            self.automation_insights.room_automation_potential.len(),
            self.automation_insights
                .device_capabilities
                .automation_score
                * 10.0,
            self.optimization_opportunities
                .iter()
                .take(3)
                .map(|o| o.title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
