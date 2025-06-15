//! Sampling response parsing and command extraction
//!
//! Parses MCP sampling responses to extract actionable commands and structured data
//! for home automation execution.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Parsed sampling response with extracted commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingResponse {
    /// Original response text
    pub content: String,
    /// Extracted device commands
    pub commands: Vec<DeviceCommand>,
    /// Extracted recommendations
    pub recommendations: Vec<Recommendation>,
    /// Analysis summary
    pub analysis: Option<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// Device command extracted from sampling response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCommand {
    /// Device identifier (name or UUID)
    pub device: String,
    /// Action to perform
    pub action: String,
    /// Optional value for the action
    pub value: Option<String>,
    /// Room context if specified
    pub room: Option<String>,
    /// Confidence in this command (0.0 - 1.0)
    pub confidence: f32,
}

/// Recommendation extracted from sampling response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommendation text
    pub text: String,
    /// Category (energy, comfort, security, etc.)
    pub category: String,
    /// Priority (high, medium, low)
    pub priority: String,
    /// Whether this can be automated
    pub automatable: bool,
}

/// Command extractor for parsing sampling responses
pub struct CommandExtractor {
    /// Device name patterns for recognition
    device_patterns: HashMap<String, Vec<String>>,
    /// Action patterns for recognition
    action_patterns: HashMap<String, Vec<String>>,
    /// Room name patterns
    room_patterns: Vec<String>,
}

impl Default for CommandExtractor {
    fn default() -> Self {
        let mut device_patterns = HashMap::new();
        let mut action_patterns = HashMap::new();

        // Light device patterns
        device_patterns.insert(
            "light".to_string(),
            vec![
                "light".to_string(),
                "lamp".to_string(),
                "lighting".to_string(),
                "bulb".to_string(),
            ],
        );

        // Blind/Rolladen patterns
        device_patterns.insert(
            "blind".to_string(),
            vec![
                "blind".to_string(),
                "rolladen".to_string(),
                "shutter".to_string(),
                "curtain".to_string(),
                "shade".to_string(),
            ],
        );

        // Climate patterns
        device_patterns.insert(
            "climate".to_string(),
            vec![
                "temperature".to_string(),
                "heating".to_string(),
                "thermostat".to_string(),
                "climate".to_string(),
            ],
        );

        // Audio patterns
        device_patterns.insert(
            "audio".to_string(),
            vec![
                "audio".to_string(),
                "music".to_string(),
                "speaker".to_string(),
                "sound".to_string(),
            ],
        );

        // Light actions
        action_patterns.insert(
            "light".to_string(),
            vec![
                "turn on".to_string(),
                "turn off".to_string(),
                "switch on".to_string(),
                "switch off".to_string(),
                "dim".to_string(),
                "brighten".to_string(),
                "set to".to_string(),
            ],
        );

        // Blind actions
        action_patterns.insert(
            "blind".to_string(),
            vec![
                "open".to_string(),
                "close".to_string(),
                "raise".to_string(),
                "lower".to_string(),
                "up".to_string(),
                "down".to_string(),
                "stop".to_string(),
            ],
        );

        // Climate actions
        action_patterns.insert(
            "climate".to_string(),
            vec![
                "set temperature".to_string(),
                "adjust temperature".to_string(),
                "heat to".to_string(),
                "cool to".to_string(),
                "increase".to_string(),
                "decrease".to_string(),
            ],
        );

        // Audio actions
        action_patterns.insert(
            "audio".to_string(),
            vec![
                "play".to_string(),
                "stop".to_string(),
                "pause".to_string(),
                "volume".to_string(),
                "mute".to_string(),
                "unmute".to_string(),
            ],
        );

        let room_patterns = vec![
            "living room".to_string(),
            "kitchen".to_string(),
            "bedroom".to_string(),
            "bathroom".to_string(),
            "hallway".to_string(),
            "office".to_string(),
            "dining room".to_string(),
            "study".to_string(),
            "basement".to_string(),
            "attic".to_string(),
        ];

        Self {
            device_patterns,
            action_patterns,
            room_patterns,
        }
    }
}

impl CommandExtractor {
    /// Parse a sampling response and extract commands and recommendations
    pub fn parse_response(&self, content: String) -> Result<SamplingResponse> {
        debug!("Parsing sampling response, {} characters", content.len());

        let commands = self.extract_commands(&content)?;
        let recommendations = self.extract_recommendations(&content);
        let analysis = self.extract_analysis(&content);
        let confidence = self.calculate_confidence(&content, &commands);

        Ok(SamplingResponse {
            content,
            commands,
            recommendations,
            analysis,
            confidence,
        })
    }

    /// Extract device commands from response text
    fn extract_commands(&self, text: &str) -> Result<Vec<DeviceCommand>> {
        let mut commands = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Skip headers, bullet points, and explanatory text
            if line.starts_with('#') || line.starts_with("##") || line.starts_with("###") {
                continue;
            }

            if let Some(command) = self.parse_command_line(line) {
                commands.push(command);
            }
        }

        debug!(
            "Extracted {} commands from sampling response",
            commands.len()
        );
        Ok(commands)
    }

    /// Parse a single line for device commands
    fn parse_command_line(&self, line: &str) -> Option<DeviceCommand> {
        let line_lower = line.to_lowercase();

        // Find device type
        let mut device_type = None;
        let mut device_match = None;

        for (dev_type, patterns) in &self.device_patterns {
            for pattern in patterns {
                if line_lower.contains(pattern) {
                    device_type = Some(dev_type.clone());
                    device_match = Some(pattern.clone());
                    break;
                }
            }
            if device_type.is_some() {
                break;
            }
        }

        let device_type = device_type?;

        // Find action
        let mut action = None;
        let action_patterns = self.action_patterns.get(&device_type)?;

        for pattern in action_patterns {
            if line_lower.contains(pattern) {
                action = Some(self.normalize_action(pattern, &device_type));
                break;
            }
        }

        let action = action?;

        // Find room
        let room = self.extract_room(&line_lower);

        // Extract device name
        let device_name = self.extract_device_name(line, &device_type, &room);

        // Extract value if applicable
        let value = self.extract_value(line, &action);

        // Calculate confidence based on pattern matches
        let confidence = self.calculate_command_confidence(line, &device_match.unwrap_or_default());

        Some(DeviceCommand {
            device: device_name,
            action,
            value,
            room,
            confidence,
        })
    }

    /// Extract room name from text
    fn extract_room(&self, text: &str) -> Option<String> {
        for room in &self.room_patterns {
            if text.contains(room) {
                return Some(self.capitalize_words(room));
            }
        }
        None
    }

    /// Extract device name from text
    fn extract_device_name(&self, text: &str, device_type: &str, room: &Option<String>) -> String {
        // Try to build a specific device name
        if let Some(room) = room {
            format!("{} {}", room, self.capitalize_words(device_type))
        } else {
            // Look for specific device names in the text
            let text_lower = text.to_lowercase();

            // Check for specific device patterns
            if text_lower.contains("all") {
                format!("All {}", self.capitalize_words(device_type))
            } else {
                // Default to generic device name
                self.capitalize_words(device_type)
            }
        }
    }

    /// Extract value from action text (e.g., temperature, percentage)
    fn extract_value(&self, text: &str, action: &str) -> Option<String> {
        match action {
            "set_temperature" | "adjust_temperature" => self.extract_temperature_value(text),
            "dim" | "brighten" => self.extract_percentage_value(text),
            "volume" => self.extract_percentage_value(text),
            _ => None,
        }
    }

    /// Extract temperature value from text
    fn extract_temperature_value(&self, text: &str) -> Option<String> {
        use regex::Regex;

        // Pattern for temperature values
        let re = Regex::new(r"(\d+(?:\.\d+)?)\s*°?[CcFf]?").ok()?;

        if let Some(caps) = re.captures(text) {
            if let Some(temp_match) = caps.get(1) {
                let temp_str = temp_match.as_str();

                // Check if it's Fahrenheit and convert
                if text.to_lowercase().contains('f') || text.to_lowercase().contains("fahrenheit") {
                    if let Ok(f) = temp_str.parse::<f32>() {
                        let c = (f - 32.0) * 5.0 / 9.0;
                        return Some(format!("{:.1}", c));
                    }
                }

                return Some(temp_str.to_string());
            }
        }

        None
    }

    /// Extract percentage value from text
    fn extract_percentage_value(&self, text: &str) -> Option<String> {
        use regex::Regex;

        let re = Regex::new(r"(\d+)\s*%").ok()?;

        if let Some(caps) = re.captures(text) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }

        // Look for numeric values that could be percentages
        let re_num = Regex::new(r"(\d+)").ok()?;
        if let Some(caps) = re_num.captures(text) {
            if let Some(num_str) = caps.get(1).map(|m| m.as_str()) {
                if let Ok(num) = num_str.parse::<u32>() {
                    if num <= 100 {
                        return Some(num_str.to_string());
                    }
                }
            }
        }

        None
    }

    /// Normalize action to standard format
    fn normalize_action(&self, pattern: &str, device_type: &str) -> String {
        match (pattern, device_type) {
            ("turn on" | "switch on", "light") => "on".to_string(),
            ("turn off" | "switch off", "light") => "off".to_string(),
            ("open" | "raise" | "up", "blind") => "up".to_string(),
            ("close" | "lower" | "down", "blind") => "down".to_string(),
            ("set temperature" | "adjust temperature" | "heat to" | "cool to", "climate") => {
                "set_temperature".to_string()
            }
            ("play", "audio") => "play".to_string(),
            ("stop", "audio") => "stop".to_string(),
            ("volume", "audio") => "volume".to_string(),
            _ => pattern.replace(' ', "_").to_string(),
        }
    }

    /// Extract recommendations from response text
    fn extract_recommendations(&self, text: &str) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for line in lines {
            let line = line.trim();

            // Look for recommendation patterns
            if self.is_recommendation_line(line) {
                if let Some(rec) = self.parse_recommendation(line) {
                    recommendations.push(rec);
                }
            }
        }

        recommendations
    }

    /// Check if a line contains a recommendation
    fn is_recommendation_line(&self, line: &str) -> bool {
        let line_lower = line.to_lowercase();

        line_lower.contains("recommend")
            || line_lower.contains("suggest")
            || line_lower.contains("consider")
            || line_lower.contains("advice")
            || line.starts_with('-')
            || line.starts_with('•')
            || line.starts_with("1.")
            || line.starts_with("2.")
            || line.starts_with("3.")
    }

    /// Parse a recommendation from text
    fn parse_recommendation(&self, line: &str) -> Option<Recommendation> {
        let text = line.trim_start_matches([
            '-', '•', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '.', ' ',
        ]);

        if text.is_empty() {
            return None;
        }

        let category = self.categorize_recommendation(text);
        let priority = self.assess_priority(text);
        let automatable = self.is_automatable(text);

        Some(Recommendation {
            text: text.to_string(),
            category,
            priority,
            automatable,
        })
    }

    /// Categorize a recommendation
    fn categorize_recommendation(&self, text: &str) -> String {
        let text_lower = text.to_lowercase();

        if text_lower.contains("energy")
            || text_lower.contains("power")
            || text_lower.contains("efficiency")
        {
            "energy".to_string()
        } else if text_lower.contains("comfort")
            || text_lower.contains("temperature")
            || text_lower.contains("cozy")
        {
            "comfort".to_string()
        } else if text_lower.contains("security")
            || text_lower.contains("safety")
            || text_lower.contains("lock")
        {
            "security".to_string()
        } else if text_lower.contains("light") || text_lower.contains("lighting") {
            "lighting".to_string()
        } else if text_lower.contains("climate")
            || text_lower.contains("heating")
            || text_lower.contains("cooling")
        {
            "climate".to_string()
        } else {
            "general".to_string()
        }
    }

    /// Assess recommendation priority
    fn assess_priority(&self, text: &str) -> String {
        let text_lower = text.to_lowercase();

        if text_lower.contains("urgent")
            || text_lower.contains("critical")
            || text_lower.contains("immediately")
        {
            "high".to_string()
        } else if text_lower.contains("important") || text_lower.contains("should") {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }

    /// Check if recommendation can be automated
    fn is_automatable(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();

        // Check for automation-friendly actions
        text_lower.contains("turn")
            || text_lower.contains("set")
            || text_lower.contains("adjust")
            || text_lower.contains("open")
            || text_lower.contains("close")
            || text_lower.contains("dim")
            || text_lower.contains("schedule")
    }

    /// Extract analysis summary from response
    fn extract_analysis(&self, text: &str) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();

        // Look for analysis section
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            if line_lower.contains("analysis")
                || line_lower.contains("summary")
                || line_lower.contains("overview")
            {
                // Take the next few lines as analysis
                let analysis_lines: Vec<&str> = lines.iter().skip(i + 1).take(3).cloned().collect();

                let analysis = analysis_lines.join(" ").trim().to_string();
                if !analysis.is_empty() {
                    return Some(analysis);
                }
            }
        }

        // Fallback: take first paragraph as analysis
        let first_paragraph = text.split("\n\n").next()?.trim().to_string();

        if first_paragraph.len() > 50 {
            Some(first_paragraph)
        } else {
            None
        }
    }

    /// Calculate overall confidence score
    fn calculate_confidence(&self, text: &str, commands: &[DeviceCommand]) -> f32 {
        if commands.is_empty() {
            return 0.3; // Low confidence if no commands extracted
        }

        let avg_command_confidence: f32 =
            commands.iter().map(|cmd| cmd.confidence).sum::<f32>() / commands.len() as f32;

        // Boost confidence if response seems well-structured
        let structure_bonus = if self.has_good_structure(text) {
            0.1
        } else {
            0.0
        };

        (avg_command_confidence + structure_bonus).min(1.0)
    }

    /// Calculate confidence for a single command
    fn calculate_command_confidence(&self, line: &str, _pattern: &str) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence

        // Boost for clear device names
        if self
            .room_patterns
            .iter()
            .any(|room| line.to_lowercase().contains(room))
        {
            confidence += 0.2;
        }

        // Boost for specific values
        if line.contains('%') || line.contains('°') || line.contains("degrees") {
            confidence += 0.2;
        }

        // Boost for strong action verbs
        if line.to_lowercase().contains("turn on") || line.to_lowercase().contains("set to") {
            confidence += 0.1;
        }

        confidence.min(1.0)
    }

    /// Check if response has good structure
    fn has_good_structure(&self, text: &str) -> bool {
        let line_count = text.lines().count();
        let has_bullets = text.contains('-') || text.contains('•');
        let has_numbers = text.contains("1.") || text.contains("2.");

        line_count > 3 && (has_bullets || has_numbers)
    }

    /// Capitalize words for device names
    fn capitalize_words(&self, text: &str) -> String {
        text.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_temperature_value() {
        let extractor = CommandExtractor::default();

        assert_eq!(
            extractor.extract_temperature_value("Set temperature to 22°C"),
            Some("22".to_string())
        );
        assert_eq!(
            extractor.extract_temperature_value("Heat to 72°F"),
            Some("22.2".to_string())
        );
        assert_eq!(
            extractor.extract_temperature_value("Adjust to 20.5 degrees"),
            Some("20.5".to_string())
        );
    }

    #[test]
    fn test_parse_command_line() {
        let extractor = CommandExtractor::default();

        let command = extractor
            .parse_command_line("Turn on living room light")
            .unwrap();
        assert_eq!(command.device, "Living Room Light");
        assert_eq!(command.action, "on");
        assert_eq!(command.room, Some("Living Room".to_string()));
    }

    #[test]
    fn test_extract_recommendations() {
        let extractor = CommandExtractor::default();

        let text =
            "I recommend:\n- Turn on the lights\n- Close the blinds\n- Set temperature to 22°C";
        let recommendations = extractor.extract_recommendations(text);

        assert_eq!(recommendations.len(), 4); // "I recommend:" is also treated as a recommendation
                                              // The actual recommendations start from index 1
        assert!(recommendations[1].automatable);
    }
}
