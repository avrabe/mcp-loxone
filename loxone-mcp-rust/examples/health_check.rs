//! Example demonstrating comprehensive health checks
//!
//! This example shows how the enhanced health check system provides
//! detailed monitoring beyond basic connectivity.

use async_trait::async_trait;
use loxone_mcp_rust::client::LoxoneClient;
use loxone_mcp_rust::error::{LoxoneError, Result};
use loxone_mcp_rust::server::health_check::{HealthCheckConfig, HealthChecker, HealthStatus};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, Level};

/// Mock client for demonstration
struct MockLoxoneClient {
    should_fail_connectivity: bool,
    should_fail_system_info: bool,
    slow_responses: bool,
}

#[async_trait]
impl LoxoneClient for MockLoxoneClient {
    async fn connect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn is_connected(&self) -> Result<bool> {
        Ok(true)
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn send_command(
        &self,
        _uuid: &str,
        _action: &str,
    ) -> Result<loxone_mcp_rust::client::LoxoneResponse> {
        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }

        Ok(loxone_mcp_rust::client::LoxoneResponse {
            code: 200,
            value: serde_json::json!("OK"),
        })
    }

    async fn get_structure(&self) -> Result<loxone_mcp_rust::client::LoxoneStructure> {
        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }

        let mut rooms = HashMap::new();
        rooms.insert(
            "living-room-uuid".to_string(),
            serde_json::json!({
                "name": "Living Room",
                "uuid": "living-room-uuid"
            }),
        );

        let mut controls = HashMap::new();
        controls.insert(
            "light-1-uuid".to_string(),
            serde_json::json!({
                "name": "Living Room Light",
                "type": "LightController",
                "room": "living-room-uuid"
            }),
        );
        controls.insert(
            "blind-1-uuid".to_string(),
            serde_json::json!({
                "name": "Living Room Blind",
                "type": "Jalousie",
                "room": "living-room-uuid"
            }),
        );

        Ok(loxone_mcp_rust::client::LoxoneStructure {
            last_modified: chrono::Utc::now().to_string(),
            rooms,
            controls,
            cats: HashMap::new(),
            global_states: HashMap::new(),
        })
    }

    async fn get_device_states(
        &self,
        uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }

        let mut states = HashMap::new();
        for uuid in uuids {
            states.insert(
                uuid.clone(),
                serde_json::json!({
                    "value": 1.0,
                    "lastUpdate": chrono::Utc::now()
                }),
            );
        }
        Ok(states)
    }

    async fn get_state_values(
        &self,
        state_uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(600)).await;
        }

        let mut states = HashMap::new();
        for uuid in state_uuids {
            states.insert(uuid.clone(), serde_json::json!(42.0));
        }
        Ok(states)
    }

    async fn get_system_info(&self) -> Result<serde_json::Value> {
        if self.should_fail_system_info {
            return Err(LoxoneError::connection("System info unavailable"));
        }

        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(1200)).await;
        }

        Ok(serde_json::json!({
            "swVersion": "12.3.4.5",
            "serialNr": "502F12345678",
            "macAddress": "50:2F:12:34:56:78",
            "deviceName": "LoxBerry",
            "projectName": "Smart Home",
            "localUrl": "http://192.168.1.77",
            "remoteUrl": "https://smartbuilding.loxone.com",
            "tempUnit": 1,
            "currency": "€",
            "squareUnit": "m²",
            "location": "47.123456,15.123456",
            "categoryTitle": "My Smart Home",
            "roomTitle": "Rooms",
            "miniserverType": 1,
            "currentUser": {
                "name": "admin",
                "uuid": "12345678-1234-1234-1234-123456789012",
                "isAdmin": true
            }
        }))
    }

    async fn health_check(&self) -> Result<bool> {
        if self.should_fail_connectivity {
            return Err(LoxoneError::connection("Connection failed"));
        }

        if self.slow_responses {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        Ok(true)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::main]
async fn main() -> loxone_mcp_rust::error::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🏥 Enhanced health check demonstration");

    // Example 1: Healthy system
    info!("\n🟢 Example 1: Healthy system");
    let healthy_client = Arc::new(MockLoxoneClient {
        should_fail_connectivity: false,
        should_fail_system_info: false,
        slow_responses: false,
    });

    let health_checker = HealthChecker::with_defaults(healthy_client);
    let report = health_checker.check_health().await?;

    info!("Overall Status: {:?}", report.overall_status);
    info!("Response Time: {}ms", report.overall_response_time_ms);
    info!(
        "Summary: {} total checks, {} healthy, {} degraded, {} unhealthy, {} critical",
        report.summary.total_checks,
        report.summary.healthy_checks,
        report.summary.degraded_checks,
        report.summary.unhealthy_checks,
        report.summary.critical_checks
    );

    for check in &report.checks {
        info!(
            "✓ {}: {:?} ({}ms) - {}",
            check.name, check.status, check.response_time_ms, check.message
        );
    }

    // Example 2: Degraded system (slow responses)
    info!("\n🟡 Example 2: Degraded system (slow responses)");
    let slow_client = Arc::new(MockLoxoneClient {
        should_fail_connectivity: false,
        should_fail_system_info: false,
        slow_responses: true,
    });

    let slow_config = HealthCheckConfig {
        slow_response_threshold_ms: 500, // Lower threshold for demo
        critical_response_threshold_ms: 2000,
        ..Default::default()
    };

    let health_checker = HealthChecker::new(slow_client, slow_config);
    let report = health_checker.check_health().await?;

    info!("Overall Status: {:?}", report.overall_status);
    info!("Response Time: {}ms", report.overall_response_time_ms);

    for check in &report.checks {
        let status_emoji = match check.status {
            HealthStatus::Healthy => "✅",
            HealthStatus::Degraded => "⚠️ ",
            HealthStatus::Unhealthy => "❌",
            HealthStatus::Critical => "🚨",
        };
        info!(
            "{} {}: {:?} ({}ms) - {}",
            status_emoji, check.name, check.status, check.response_time_ms, check.message
        );
    }

    // Example 3: Unhealthy system
    info!("\n🔴 Example 3: Unhealthy system");
    let failing_client = Arc::new(MockLoxoneClient {
        should_fail_connectivity: false,
        should_fail_system_info: true,
        slow_responses: false,
    });

    let health_checker = HealthChecker::with_defaults(failing_client);
    let report = health_checker.check_health().await?;

    info!("Overall Status: {:?}", report.overall_status);
    info!("Response Time: {}ms", report.overall_response_time_ms);

    for check in &report.checks {
        let status_emoji = match check.status {
            HealthStatus::Healthy => "✅",
            HealthStatus::Degraded => "⚠️ ",
            HealthStatus::Unhealthy => "❌",
            HealthStatus::Critical => "🚨",
        };
        info!(
            "{} {}: {:?} ({}ms) - {}",
            status_emoji, check.name, check.status, check.response_time_ms, check.message
        );
    }

    // Example 4: Critical system
    info!("\n🚨 Example 4: Critical system (no connectivity)");
    let critical_client = Arc::new(MockLoxoneClient {
        should_fail_connectivity: true,
        should_fail_system_info: true,
        slow_responses: false,
    });

    let health_checker = HealthChecker::with_defaults(critical_client);
    let report = health_checker.check_health().await?;

    info!("Overall Status: {:?}", report.overall_status);
    info!("Response Time: {}ms", report.overall_response_time_ms);

    for check in &report.checks {
        let status_emoji = match check.status {
            HealthStatus::Healthy => "✅",
            HealthStatus::Degraded => "⚠️ ",
            HealthStatus::Unhealthy => "❌",
            HealthStatus::Critical => "🚨",
        };
        info!(
            "{} {}: {:?} ({}ms) - {}",
            status_emoji, check.name, check.status, check.response_time_ms, check.message
        );
    }

    // Example 5: Health status scores
    info!("\n📊 Health status scoring system:");
    info!("Healthy: {} points", HealthStatus::Healthy.score());
    info!("Degraded: {} points", HealthStatus::Degraded.score());
    info!("Unhealthy: {} points", HealthStatus::Unhealthy.score());
    info!("Critical: {} points", HealthStatus::Critical.score());

    info!("\n✅ Health check demonstration complete");
    Ok(())
}
