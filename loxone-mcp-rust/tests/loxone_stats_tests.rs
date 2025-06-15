//! Tests for Loxone statistics collection system

use loxone_mcp_rust::client::{ClientContext, LoxoneDevice};
use loxone_mcp_rust::mock::MockLoxoneClient;
use loxone_mcp_rust::monitoring::loxone_stats::LoxoneStatsCollector;
use loxone_mcp_rust::monitoring::metrics::MetricsCollector;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_stats_collector_creation() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let _collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector,
        #[cfg(feature = "influxdb")]
        None,
    );

    // Collector should be created successfully (just test creation)
}

#[tokio::test]
async fn test_comfort_index_calculation() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector,
        #[cfg(feature = "influxdb")]
        None,
    );

    // Perfect conditions should give 100
    let perfect_score = collector.calculate_comfort_index(22.0, Some(50.0));
    assert_eq!(perfect_score, 100.0);

    // Good conditions should be above 80
    let good_score = collector.calculate_comfort_index(23.0, Some(55.0));
    assert!(good_score > 80.0);

    // Poor conditions should be lower
    let poor_score = collector.calculate_comfort_index(30.0, Some(80.0));
    assert!(poor_score < 70.0);
}

#[tokio::test]
async fn test_device_categorization() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector,
        #[cfg(feature = "influxdb")]
        None,
    );

    // Test light device detection
    let light_device = LoxoneDevice {
        uuid: "light-1".to_string(),
        name: "Living Room Light".to_string(),
        device_type: "LightController".to_string(),
        room: Some("Living Room".to_string()),
        states: HashMap::new(),
        category: "lighting".to_string(),
        sub_controls: HashMap::new(),
    };

    assert!(collector.is_controllable_device(&light_device));
    assert!(!collector.is_climate_sensor(&light_device));

    // Test climate sensor detection
    let temp_sensor = LoxoneDevice {
        uuid: "temp-1".to_string(),
        name: "Living Room Temperature".to_string(),
        device_type: "TemperatureSensor".to_string(),
        room: Some("Living Room".to_string()),
        states: HashMap::new(),
        category: "sensors".to_string(),
        sub_controls: HashMap::new(),
    };

    assert!(!collector.is_controllable_device(&temp_sensor));
    assert!(collector.is_climate_sensor(&temp_sensor));
}

#[tokio::test]
async fn test_temperature_extraction() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector,
        #[cfg(feature = "influxdb")]
        None,
    );

    // Test device with temperature value
    let mut device = LoxoneDevice {
        uuid: "temp-1".to_string(),
        name: "Temperature Sensor".to_string(),
        device_type: "TemperatureSensor".to_string(),
        room: Some("Living Room".to_string()),
        states: HashMap::new(),
        category: "sensors".to_string(),
        sub_controls: HashMap::new(),
    };

    // Add temperature state
    device.states.insert("value".to_string(), json!(22.5));

    let temp = collector.get_temperature_value(&device).await;
    assert_eq!(temp, Some(22.5));

    // Test device with temperature under different key
    device.states.clear();
    device.states.insert("temperature".to_string(), json!(23.0));

    let temp2 = collector.get_temperature_value(&device).await;
    assert_eq!(temp2, Some(23.0));
}

#[tokio::test]
async fn test_state_value_parsing() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector,
        #[cfg(feature = "influxdb")]
        None,
    );

    // Test on state detection
    assert!(collector.is_on_state("on"));
    assert!(collector.is_on_state("1"));
    assert!(collector.is_on_state("true"));

    // Test off state detection
    assert!(!collector.is_on_state("off"));
    assert!(!collector.is_on_state("0"));
    assert!(!collector.is_on_state("false"));
}

#[tokio::test]
async fn test_metrics_initialization() {
    let client = Arc::new(MockLoxoneClient::new());
    let context = Arc::new(ClientContext::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    let collector = LoxoneStatsCollector::new(
        client,
        context,
        metrics_collector.clone(),
        #[cfg(feature = "influxdb")]
        None,
    );

    // Initialize metrics
    collector.init_metrics().await.unwrap();

    // Check that Loxone-specific metrics were registered
    let prometheus_output = metrics_collector.export_prometheus().await;

    assert!(prometheus_output.contains("loxone_active_devices"));
    assert!(prometheus_output.contains("loxone_device_power_cycles_total"));
    assert!(prometheus_output.contains("loxone_system_health_score"));
}

// Note: More comprehensive integration tests would require a real Loxone structure
// and would be better suited for integration test suite with test doubles
