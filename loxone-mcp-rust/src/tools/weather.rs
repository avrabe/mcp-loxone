//! Weather monitoring MCP tools
//!
//! Tools for weather data, outdoor conditions, and forecasting.

use crate::tools::{ToolContext, ToolResponse};
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Weather data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    /// Current temperature
    pub temperature: Option<f64>,

    /// Humidity percentage
    pub humidity: Option<f64>,

    /// Wind speed
    pub wind_speed: Option<f64>,

    /// Wind direction
    pub wind_direction: Option<f64>,

    /// Precipitation amount
    pub precipitation: Option<f64>,

    /// Atmospheric pressure
    pub pressure: Option<f64>,

    /// UV index
    pub uv_index: Option<f64>,

    /// Solar radiation
    pub solar_radiation: Option<f64>,

    /// Weather description
    pub description: Option<String>,

    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Forecast data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    /// Forecast timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Temperature
    pub temperature: Option<f64>,

    /// Min temperature
    pub temp_min: Option<f64>,

    /// Max temperature
    pub temp_max: Option<f64>,

    /// Humidity
    pub humidity: Option<f64>,

    /// Precipitation probability
    pub precipitation_probability: Option<f64>,

    /// Weather description
    pub description: Option<String>,
}

/// Get current weather data
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_weather_data(context: ToolContext) -> ToolResponse {
    // Get weather-related devices
    let devices = match context.context.get_devices_by_category("weather").await {
        Ok(devices) => devices,
        Err(e) => return ToolResponse::error(e.to_string()),
    };

    if devices.is_empty() {
        return ToolResponse::error("No weather devices found in the system".to_string());
    }

    // Extract weather data from devices
    let mut weather_data = WeatherData {
        temperature: None,
        humidity: None,
        wind_speed: None,
        wind_direction: None,
        precipitation: None,
        pressure: None,
        uv_index: None,
        solar_radiation: None,
        description: None,
        last_updated: chrono::Utc::now(),
    };

    let mut weather_sensors = Vec::new();

    for device in devices {
        let mut sensor_data = HashMap::new();

        // Extract weather parameters from device states
        for (state_name, value) in &device.states {
            let param_value = value.as_f64();

            match state_name.to_lowercase().as_str() {
                "temperature" | "temp" | "tempout" => {
                    weather_data.temperature = param_value;
                    sensor_data.insert("temperature".to_string(), value.clone());
                }
                "humidity" | "humid" | "humidout" => {
                    weather_data.humidity = param_value;
                    sensor_data.insert("humidity".to_string(), value.clone());
                }
                "windspeed" | "wind_speed" | "wind" => {
                    weather_data.wind_speed = param_value;
                    sensor_data.insert("wind_speed".to_string(), value.clone());
                }
                "winddirection" | "wind_direction" | "winddir" => {
                    weather_data.wind_direction = param_value;
                    sensor_data.insert("wind_direction".to_string(), value.clone());
                }
                "precipitation" | "rain" | "rainfall" => {
                    weather_data.precipitation = param_value;
                    sensor_data.insert("precipitation".to_string(), value.clone());
                }
                "pressure" | "baro" | "barometric" => {
                    weather_data.pressure = param_value;
                    sensor_data.insert("pressure".to_string(), value.clone());
                }
                "uv" | "uvindex" | "uv_index" => {
                    weather_data.uv_index = param_value;
                    sensor_data.insert("uv_index".to_string(), value.clone());
                }
                "solar" | "solarradiation" | "solar_radiation" => {
                    weather_data.solar_radiation = param_value;
                    sensor_data.insert("solar_radiation".to_string(), value.clone());
                }
                _ => {
                    sensor_data.insert(state_name.clone(), value.clone());
                }
            }
        }

        weather_sensors.push(serde_json::json!({
            "device": device.name,
            "uuid": device.uuid,
            "type": device.device_type,
            "room": device.room,
            "data": sensor_data
        }));
    }

    let response_data = serde_json::json!({
        "weather": weather_data,
        "sensors": weather_sensors,
        "summary": generate_weather_summary(&weather_data),
        "timestamp": chrono::Utc::now()
    });

    ToolResponse::success_with_message(
        response_data,
        format!("Weather data from {} sensors", weather_sensors.len()),
    )
}

/// Get outdoor environmental conditions
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_outdoor_conditions(context: ToolContext) -> ToolResponse {
    // Get weather data first
    let weather_response = get_weather_data(context).await;

    match weather_response.status.as_str() {
        "success" => {
            let weather_data = weather_response.data.clone();

            // Add comfort assessment and recommendations
            let mut conditions = weather_data.as_object().unwrap().clone();

            if let Some(weather_obj) = conditions.get("weather").and_then(|w| w.as_object()) {
                let weather_obj_clone = weather_obj.clone();
                let comfort = assess_outdoor_comfort(&weather_obj_clone);
                conditions.insert("comfort_assessment".to_string(), comfort);

                let recommendations = generate_outdoor_recommendations(&weather_obj_clone);
                conditions.insert("recommendations".to_string(), recommendations);
            }

            ToolResponse::success_with_message(
                serde_json::Value::Object(conditions),
                "Outdoor conditions with comfort assessment".to_string(),
            )
        }
        _ => weather_response,
    }
}

/// Get daily weather forecast
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_weather_forecast_daily(
    _context: ToolContext,
    // #[description("Number of days to forecast")] // TODO: Re-enable when rmcp API is clarified
    days: Option<u32>,
) -> ToolResponse {
    let forecast_days = days.unwrap_or(7).min(14); // Limit to 14 days

    // In a real implementation, this would query forecast data from weather services
    // For now, generate sample forecast data
    let mut forecast = Vec::new();

    for day in 0..forecast_days {
        let forecast_date = chrono::Utc::now() + chrono::Duration::days(day as i64);

        // Generate sample forecast data (in reality, this would come from weather API)
        let base_temp = 20.0 + (day as f64 * 0.5) - 2.0;
        let forecast_point = ForecastPoint {
            timestamp: forecast_date,
            temperature: Some(base_temp),
            temp_min: Some(base_temp - 5.0),
            temp_max: Some(base_temp + 5.0),
            humidity: Some(60.0 + (day as f64 * 2.0)),
            precipitation_probability: Some((day as f64 * 10.0) % 100.0),
            description: Some(generate_weather_description(day)),
        };

        forecast.push(forecast_point);
    }

    let response_data = serde_json::json!({
        "forecast": forecast,
        "forecast_days": forecast_days,
        "generated_at": chrono::Utc::now(),
        "note": "Sample forecast data - integrate with weather service for real data"
    });

    ToolResponse::success_with_message(
        response_data,
        format!("{forecast_days}-day weather forecast"),
    )
}

/// Get hourly weather forecast
// #[tool] // TODO: Re-enable when rmcp API is clarified
pub async fn get_weather_forecast_hourly(
    _context: ToolContext,
    // #[description("Number of hours to forecast")] // TODO: Re-enable when rmcp API is clarified
    hours: Option<u32>,
) -> ToolResponse {
    let forecast_hours = hours.unwrap_or(24).min(72); // Limit to 72 hours

    // Generate sample hourly forecast data
    let mut forecast = Vec::new();

    for hour in 0..forecast_hours {
        let forecast_time = chrono::Utc::now() + chrono::Duration::hours(hour as i64);

        let base_temp = 18.0 + (hour as f64 * 0.2) + (hour as f64 / 24.0).sin() * 5.0;
        let forecast_point = ForecastPoint {
            timestamp: forecast_time,
            temperature: Some(base_temp),
            temp_min: None,
            temp_max: None,
            humidity: Some(55.0 + (hour as f64 * 0.5) % 40.0),
            precipitation_probability: Some((hour as f64 * 3.0) % 100.0),
            description: Some(generate_hourly_weather_description(hour)),
        };

        forecast.push(forecast_point);
    }

    let response_data = serde_json::json!({
        "forecast": forecast,
        "forecast_hours": forecast_hours,
        "generated_at": chrono::Utc::now(),
        "note": "Sample forecast data - integrate with weather service for real data"
    });

    ToolResponse::success_with_message(
        response_data,
        format!("{forecast_hours}-hour weather forecast"),
    )
}

/// Generate weather summary text
fn generate_weather_summary(weather: &WeatherData) -> String {
    let mut summary_parts = Vec::new();

    if let Some(temp) = weather.temperature {
        summary_parts.push(format!("{temp:.1}°C"));
    }

    if let Some(humidity) = weather.humidity {
        summary_parts.push(format!("{humidity:.0}% humidity"));
    }

    if let Some(wind) = weather.wind_speed {
        if wind > 0.0 {
            summary_parts.push(format!("{wind:.1} km/h wind"));
        }
    }

    if let Some(rain) = weather.precipitation {
        if rain > 0.0 {
            summary_parts.push(format!("{rain:.1}mm rain"));
        }
    }

    if summary_parts.is_empty() {
        "No weather data available".to_string()
    } else {
        summary_parts.join(", ")
    }
}

/// Assess outdoor comfort based on weather conditions
fn assess_outdoor_comfort(
    weather: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut comfort_score: f64 = 50.0; // Base score out of 100
    let mut factors = Vec::new();

    // Temperature comfort
    if let Some(temp) = weather.get("temperature").and_then(|v| v.as_f64()) {
        let temp_comfort = match temp {
            t if t < 0.0 => {
                factors.push("Very cold temperature".to_string());
                10.0
            }
            t if t < 10.0 => {
                factors.push("Cold temperature".to_string());
                30.0
            }
            t if (10.0..=25.0).contains(&t) => {
                factors.push("Comfortable temperature".to_string());
                90.0
            }
            t if t <= 30.0 => {
                factors.push("Warm temperature".to_string());
                70.0
            }
            _ => {
                factors.push("Hot temperature".to_string());
                30.0
            }
        };
        comfort_score = (comfort_score + temp_comfort) / 2.0;
    }

    // Wind comfort
    if let Some(wind) = weather.get("wind_speed").and_then(|v| v.as_f64()) {
        let wind_comfort = match wind {
            w if w < 10.0 => 90.0,
            w if w < 20.0 => {
                factors.push("Breezy conditions".to_string());
                70.0
            }
            w if w < 40.0 => {
                factors.push("Windy conditions".to_string());
                40.0
            }
            _ => {
                factors.push("Very windy conditions".to_string());
                20.0
            }
        };
        comfort_score = (comfort_score + wind_comfort) / 2.0;
    }

    // Precipitation comfort
    if let Some(rain) = weather.get("precipitation").and_then(|v| v.as_f64()) {
        if rain > 0.0 {
            factors.push("Precipitation present".to_string());
            comfort_score = (comfort_score + 30.0) / 2.0;
        }
    }

    let comfort_level = match comfort_score {
        s if s >= 80.0 => "Excellent",
        s if s >= 60.0 => "Good",
        s if s >= 40.0 => "Fair",
        s if s >= 20.0 => "Poor",
        _ => "Very Poor",
    };

    serde_json::json!({
        "score": comfort_score.round() as u32,
        "level": comfort_level,
        "factors": factors
    })
}

/// Generate outdoor activity recommendations
fn generate_outdoor_recommendations(
    weather: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut recommendations = Vec::new();

    if let Some(temp) = weather.get("temperature").and_then(|v| v.as_f64()) {
        match temp {
            t if t < 0.0 => {
                recommendations.push("Dress warmly with multiple layers");
                recommendations.push("Limit outdoor exposure time");
            }
            t if t < 10.0 => {
                recommendations.push("Wear warm clothing and jacket");
                recommendations.push("Good for brisk walking");
            }
            t if (10.0..=25.0).contains(&t) => {
                recommendations.push("Perfect weather for outdoor activities");
                recommendations.push("Great for walking, cycling, or sports");
            }
            t if t <= 30.0 => {
                recommendations.push("Wear light, breathable clothing");
                recommendations.push("Stay hydrated");
            }
            _ => {
                recommendations.push("Seek shade and stay hydrated");
                recommendations.push("Avoid prolonged sun exposure");
            }
        }
    }

    if let Some(wind) = weather.get("wind_speed").and_then(|v| v.as_f64()) {
        if wind > 20.0 {
            recommendations.push("Secure loose items outdoors");
            recommendations.push("Be cautious of wind chill");
        }
    }

    if let Some(rain) = weather.get("precipitation").and_then(|v| v.as_f64()) {
        if rain > 0.0 {
            recommendations.push("Bring umbrella or rain gear");
            recommendations.push("Watch for slippery surfaces");
        }
    }

    serde_json::Value::Array(
        recommendations
            .into_iter()
            .map(|r| serde_json::Value::String(r.to_string()))
            .collect(),
    )
}

/// Generate sample weather description for daily forecast
fn generate_weather_description(day: u32) -> String {
    let descriptions = [
        "Partly cloudy",
        "Sunny",
        "Mostly cloudy",
        "Light rain",
        "Overcast",
        "Clear skies",
        "Scattered showers",
    ];

    descriptions[day as usize % descriptions.len()].to_string()
}

/// Generate sample weather description for hourly forecast
fn generate_hourly_weather_description(hour: u32) -> String {
    let descriptions = [
        "Clear",
        "Partly cloudy",
        "Cloudy",
        "Light rain",
        "Overcast",
        "Fog",
    ];

    descriptions[hour as usize % descriptions.len()].to_string()
}
