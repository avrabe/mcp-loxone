//! Dashboard data provider for historical data visualization

use super::core::UnifiedHistoryStore;
use super::events::*;
use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::HashMap;
use tracing::debug;

/// Dashboard data provider
pub struct DashboardHistoryProvider {
    store: UnifiedHistoryStore,
}

impl DashboardHistoryProvider {
    /// Create new dashboard provider
    pub fn new(store: UnifiedHistoryStore) -> Self {
        Self { store }
    }

    /// Get comprehensive dashboard data
    pub async fn get_dashboard_data(&self) -> Result<DashboardHistoryData> {
        debug!("Generating dashboard history data");

        let now = Utc::now();
        let last_24h = now - Duration::hours(24);
        let last_7d = now - Duration::days(7);

        // Collect all data concurrently
        let (
            device_activity,
            sensor_trends,
            system_health,
            energy_usage,
            recent_events,
            audit_summary,
        ) = tokio::try_join!(
            self.get_device_activity_chart(last_24h, now),
            self.get_sensor_trend_charts(last_24h, now),
            self.get_system_health_timeline(last_24h, now),
            self.get_energy_usage_graph(last_7d, now),
            self.get_recent_events_feed(50),
            self.get_audit_summary(last_24h, now),
        )?;

        Ok(DashboardHistoryData {
            device_activity,
            sensor_trends,
            system_health,
            energy_usage,
            recent_events,
            audit_summary,
            last_updated: now,
        })
    }

    /// Get device activity chart data
    async fn get_device_activity_chart(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<DeviceActivityChart> {
        let events = self
            .store
            .query()
            .category("device_state")
            .time_range(start, end)
            .execute()
            .await?
            .events;

        let mut room_activity: HashMap<String, u32> = HashMap::new();
        let mut device_states: HashMap<String, DeviceStatus> = HashMap::new();
        let mut total_changes = 0;

        for event in events {
            if let EventCategory::DeviceState(ref change) = event.category {
                total_changes += 1;

                // Count by room
                if let Some(ref room) = change.room {
                    *room_activity.entry(room.clone()).or_insert(0) += 1;
                }

                // Track current device state
                let is_on = match change.new_state.as_str() {
                    Some(state) => state == "on" || state == "true" || state == "1",
                    None => change.new_state.as_bool().unwrap_or(false),
                };

                device_states.insert(
                    change.device_uuid.clone(),
                    DeviceStatus {
                        name: change.device_name.clone(),
                        room: change.room.clone(),
                        is_active: is_on,
                        last_changed: event.timestamp,
                    },
                );
            }
        }

        let active_count = device_states.values().filter(|d| d.is_active).count();

        Ok(DeviceActivityChart {
            room_activity,
            device_states,
            total_changes,
            active_count,
            timeframe: format!("{} to {}", start.format("%H:%M"), end.format("%H:%M")),
        })
    }

    /// Get sensor trend charts
    async fn get_sensor_trend_charts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SensorTrendChart>> {
        let events = self
            .store
            .query()
            .category("sensor_reading")
            .time_range(start, end)
            .execute()
            .await?
            .events;

        let mut sensor_data: HashMap<String, Vec<SensorDataPoint>> = HashMap::new();
        let mut sensor_info: HashMap<String, SensorInfo> = HashMap::new();

        for event in events {
            if let EventCategory::SensorReading(ref reading) = event.category {
                sensor_data
                    .entry(reading.sensor_uuid.clone())
                    .or_default()
                    .push(SensorDataPoint {
                        timestamp: event.timestamp,
                        value: reading.value,
                    });

                sensor_info.insert(
                    reading.sensor_uuid.clone(),
                    SensorInfo {
                        name: reading.sensor_name.clone(),
                        sensor_type: reading.sensor_type.clone(),
                        unit: reading.unit.clone(),
                        room: reading.room.clone(),
                    },
                );
            }
        }

        let mut charts = Vec::new();

        for (sensor_uuid, mut data_points) in sensor_data {
            data_points.sort_by_key(|p| p.timestamp);

            if let Some(info) = sensor_info.get(&sensor_uuid) {
                let min_value = data_points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::INFINITY, f64::min);
                let max_value = data_points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                let avg_value =
                    data_points.iter().map(|p| p.value).sum::<f64>() / data_points.len() as f64;

                charts.push(SensorTrendChart {
                    sensor_uuid,
                    name: info.name.clone(),
                    sensor_type: info.sensor_type.clone(),
                    unit: info.unit.clone(),
                    room: info.room.clone(),
                    data_points,
                    min_value,
                    max_value,
                    avg_value,
                });
            }
        }

        Ok(charts)
    }

    /// Get system health timeline
    async fn get_system_health_timeline(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<SystemHealthTimeline> {
        let events = self
            .store
            .query()
            .category("system_metric")
            .time_range(start, end)
            .execute()
            .await?
            .events;

        let mut health_points = Vec::new();
        let mut error_count = 0;
        let mut warning_count = 0;

        for event in events {
            if let EventCategory::SystemMetric(ref metric) = event.category {
                let severity = if metric.metric_name.contains("error") {
                    error_count += 1;
                    "error"
                } else if metric.metric_name.contains("warning") {
                    warning_count += 1;
                    "warning"
                } else {
                    "info"
                };

                health_points.push(HealthPoint {
                    timestamp: event.timestamp,
                    metric_name: metric.metric_name.clone(),
                    value: metric.value,
                    severity: severity.to_string(),
                });
            }
        }

        health_points.sort_by_key(|p| p.timestamp);

        // Calculate overall health score
        let total_metrics = health_points.len() as f64;
        let health_score = if total_metrics > 0.0 {
            let error_ratio = error_count as f64 / total_metrics;
            let warning_ratio = warning_count as f64 / total_metrics;
            ((1.0 - error_ratio * 0.5 - warning_ratio * 0.2) * 100.0).max(0.0)
        } else {
            100.0
        };

        Ok(SystemHealthTimeline {
            health_points,
            overall_health_score: health_score,
            error_count,
            warning_count,
        })
    }

    /// Get energy usage graph (placeholder)
    async fn get_energy_usage_graph(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<EnergyUsageGraph> {
        // TODO: Implement when energy metrics are available
        Ok(EnergyUsageGraph {
            daily_usage: Vec::new(),
            total_kwh: 0.0,
            cost_estimate: None,
            peak_usage_time: None,
        })
    }

    /// Get recent events feed
    async fn get_recent_events_feed(&self, limit: usize) -> Result<Vec<RecentEventItem>> {
        let events = self.store.query().limit(limit).execute().await?.events;

        let mut items = Vec::new();

        for event in events {
            let (title, description, icon) = match &event.category {
                EventCategory::DeviceState(state) => (
                    format!("{} changed", state.device_name),
                    format!(
                        "Device in {} changed state",
                        state.room.as_deref().unwrap_or("unknown room")
                    ),
                    "device".to_string(),
                ),
                EventCategory::SensorReading(data) => (
                    format!("{} reading", data.sensor_name),
                    format!(
                        "{} {} in {}",
                        data.value,
                        data.unit,
                        data.room.as_deref().unwrap_or("unknown room")
                    ),
                    "sensor".to_string(),
                ),
                EventCategory::SystemMetric(metric) => (
                    format!("System: {}", metric.metric_name),
                    format!("{} {}", metric.value, metric.unit),
                    "system".to_string(),
                ),
                EventCategory::AuditEvent(audit) => (
                    format!("Audit: {}", audit.action),
                    format!("Action by {}: {:?}", audit.actor, audit.result),
                    "audit".to_string(),
                ),
                EventCategory::DiscoveryEvent(discovery) => (
                    format!("Discovery: {}", discovery.entity_type),
                    format!("Found {} via {}", discovery.entity_id, discovery.method),
                    "discovery".to_string(),
                ),
                EventCategory::ResponseCache(_) => continue, // Skip cache events
            };

            items.push(RecentEventItem {
                id: event.id,
                timestamp: event.timestamp,
                title,
                description,
                icon,
                category: match event.category {
                    EventCategory::DeviceState(_) => "device",
                    EventCategory::SensorReading(_) => "sensor",
                    EventCategory::SystemMetric(_) => "system",
                    EventCategory::AuditEvent(_) => "audit",
                    EventCategory::DiscoveryEvent(_) => "discovery",
                    EventCategory::ResponseCache(_) => "cache",
                }
                .to_string(),
            });
        }

        Ok(items)
    }

    /// Get audit summary
    async fn get_audit_summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AuditSummary> {
        let events = self
            .store
            .query()
            .category("audit_event")
            .time_range(start, end)
            .execute()
            .await?
            .events;

        let mut actions_by_user: HashMap<String, u32> = HashMap::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut recent_failures = Vec::new();

        for event in events {
            if let EventCategory::AuditEvent(ref audit) = event.category {
                *actions_by_user.entry(audit.actor.clone()).or_insert(0) += 1;

                match audit.result {
                    AuditResult::Success => success_count += 1,
                    AuditResult::Failure => {
                        failure_count += 1;
                        if recent_failures.len() < 10 {
                            recent_failures.push(AuditFailure {
                                timestamp: event.timestamp,
                                action: audit.action.clone(),
                                actor: audit.actor.clone(),
                                details: audit.details.to_string(),
                            });
                        }
                    }
                    AuditResult::Partial => {} // Count as neither success nor failure
                }
            }
        }

        recent_failures.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(AuditSummary {
            total_actions: success_count + failure_count,
            success_count,
            failure_count,
            actions_by_user,
            recent_failures,
        })
    }
}

/// Complete dashboard data
#[derive(Debug, Serialize)]
pub struct DashboardHistoryData {
    pub device_activity: DeviceActivityChart,
    pub sensor_trends: Vec<SensorTrendChart>,
    pub system_health: SystemHealthTimeline,
    pub energy_usage: EnergyUsageGraph,
    pub recent_events: Vec<RecentEventItem>,
    pub audit_summary: AuditSummary,
    pub last_updated: DateTime<Utc>,
}

/// Device activity chart data
#[derive(Debug, Serialize)]
pub struct DeviceActivityChart {
    pub room_activity: HashMap<String, u32>,
    pub device_states: HashMap<String, DeviceStatus>,
    pub total_changes: u32,
    pub active_count: usize,
    pub timeframe: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceStatus {
    pub name: String,
    pub room: Option<String>,
    pub is_active: bool,
    pub last_changed: DateTime<Utc>,
}

/// Sensor trend chart
#[derive(Debug, Serialize)]
pub struct SensorTrendChart {
    pub sensor_uuid: String,
    pub name: String,
    pub sensor_type: String,
    pub unit: String,
    pub room: Option<String>,
    pub data_points: Vec<SensorDataPoint>,
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
}

#[derive(Debug, Serialize)]
pub struct SensorDataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug)]
struct SensorInfo {
    pub name: String,
    pub sensor_type: String,
    pub unit: String,
    pub room: Option<String>,
}

/// System health timeline
#[derive(Debug, Serialize)]
pub struct SystemHealthTimeline {
    pub health_points: Vec<HealthPoint>,
    pub overall_health_score: f64,
    pub error_count: u32,
    pub warning_count: u32,
}

#[derive(Debug, Serialize)]
pub struct HealthPoint {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub value: f64,
    pub severity: String,
}

/// Energy usage graph (placeholder)
#[derive(Debug, Serialize)]
pub struct EnergyUsageGraph {
    pub daily_usage: Vec<EnergyDataPoint>,
    pub total_kwh: f64,
    pub cost_estimate: Option<f64>,
    pub peak_usage_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct EnergyDataPoint {
    pub timestamp: DateTime<Utc>,
    pub kwh: f64,
    pub cost: f64,
}

/// Recent event item
#[derive(Debug, Serialize)]
pub struct RecentEventItem {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub category: String,
}

/// Audit summary
#[derive(Debug, Serialize)]
pub struct AuditSummary {
    pub total_actions: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub actions_by_user: HashMap<String, u32>,
    pub recent_failures: Vec<AuditFailure>,
}

#[derive(Debug, Serialize)]
pub struct AuditFailure {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub actor: String,
    pub details: String,
}
