//! Loxone-specific statistics collection and integration
//!
//! This module collects Loxone device and sensor statistics, integrates with
//! the existing MetricsCollector, and pushes data to InfluxDB for historical analysis.

use crate::client::{ClientContext, LoxoneClient, LoxoneDevice};
use crate::error::Result;
use crate::tools::sensors::SensorType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info};

#[cfg(feature = "influxdb")]
use super::influxdb::{DeviceStateData, InfluxManager, LoxoneSensorData};
use super::metrics::MetricsCollector;

/// Device usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceUsageStats {
    /// Device UUID
    pub uuid: String,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: String,
    /// Room assignment
    pub room: Option<String>,
    /// Total on/off cycles
    pub power_cycles: u64,
    /// Total time device was on (seconds)
    pub total_on_time: u64,
    /// Last state change timestamp
    pub last_state_change: DateTime<Utc>,
    /// Current state
    pub current_state: String,
    /// Energy consumption if available (kWh)
    pub energy_consumed: Option<f64>,
    /// Average power when on (W)
    pub average_power: Option<f64>,
}

/// Room climate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomClimateStats {
    /// Room name
    pub room: String,
    /// Current temperature
    pub current_temperature: Option<f64>,
    /// Average temperature over period
    pub avg_temperature: f64,
    /// Min temperature
    pub min_temperature: f64,
    /// Max temperature
    pub max_temperature: f64,
    /// Current humidity
    pub current_humidity: Option<f64>,
    /// Average humidity
    pub avg_humidity: Option<f64>,
    /// Comfort index (0-100)
    pub comfort_index: f64,
    /// Time in comfort range (percentage)
    pub comfort_time_percent: f64,
}

/// Automation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStats {
    /// Automation/scene name
    pub name: String,
    /// Trigger count
    pub trigger_count: u64,
    /// Last triggered
    pub last_triggered: Option<DateTime<Utc>>,
    /// Average execution time (ms)
    pub avg_execution_time: f64,
    /// Success rate (percentage)
    pub success_rate: f64,
    /// Most common trigger time
    pub common_trigger_hour: Option<u8>,
}

/// Energy consumption statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyStats {
    /// Total consumption (kWh)
    pub total_consumption: f64,
    /// Current power usage (W)
    pub current_power: f64,
    /// Peak power today (W)
    pub peak_power_today: f64,
    /// Average power today (W)
    pub avg_power_today: f64,
    /// Cost estimate (if configured)
    pub estimated_cost: Option<f64>,
    /// Breakdown by room
    pub room_consumption: HashMap<String, f64>,
    /// Breakdown by device type
    pub type_consumption: HashMap<String, f64>,
}

/// Aggregated Loxone statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneStats {
    /// Collection timestamp
    pub timestamp: DateTime<Utc>,
    /// Device usage statistics
    pub device_usage: Vec<DeviceUsageStats>,
    /// Room climate statistics
    pub room_climate: Vec<RoomClimateStats>,
    /// Automation statistics
    pub automations: Vec<AutomationStats>,
    /// Energy statistics
    pub energy: Option<EnergyStats>,
    /// Total active devices
    pub active_devices: usize,
    /// Total sensors
    pub total_sensors: usize,
    /// System health score (0-100)
    pub health_score: f64,
}

/// Device state tracking for statistics
#[derive(Debug, Clone)]
pub struct DeviceStateTracker {
    pub uuid: String,
    pub name: String,
    pub device_type: String,
    pub room: Option<String>,
    pub last_state: String,
    pub last_change: Instant,
    pub on_since: Option<Instant>,
    pub power_cycles: u64,
    pub total_on_time: Duration,
}

/// Loxone statistics collector
pub struct LoxoneStatsCollector {
    /// Loxone client
    client: Arc<dyn LoxoneClient>,
    /// Client context
    context: Arc<ClientContext>,
    /// Metrics collector integration
    metrics_collector: Arc<MetricsCollector>,
    /// InfluxDB manager (optional)
    #[cfg(feature = "influxdb")]
    influx_manager: Option<Arc<InfluxManager>>,
    /// Device state trackers
    device_trackers: Arc<RwLock<HashMap<String, DeviceStateTracker>>>,
    /// Climate data buffer
    #[allow(clippy::type_complexity)]
    climate_buffer: Arc<RwLock<HashMap<String, Vec<(DateTime<Utc>, f64, Option<f64>)>>>>,
    /// Automation counters
    #[allow(clippy::type_complexity)]
    automation_counters: Arc<RwLock<HashMap<String, (u64, Option<DateTime<Utc>>)>>>,
    /// Collection interval
    collection_interval: Duration,
    /// Running flag
    running: Arc<RwLock<bool>>,
}

impl LoxoneStatsCollector {
    /// Create new Loxone statistics collector
    pub fn new(
        client: Arc<dyn LoxoneClient>,
        context: Arc<ClientContext>,
        metrics_collector: Arc<MetricsCollector>,
        #[cfg(feature = "influxdb")] influx_manager: Option<Arc<InfluxManager>>,
    ) -> Self {
        Self {
            client,
            context,
            metrics_collector,
            #[cfg(feature = "influxdb")]
            influx_manager,
            device_trackers: Arc::new(RwLock::new(HashMap::new())),
            climate_buffer: Arc::new(RwLock::new(HashMap::new())),
            automation_counters: Arc::new(RwLock::new(HashMap::new())),
            collection_interval: Duration::from_secs(60), // Collect every minute
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start statistics collection
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Initialize metrics
        self.init_metrics().await?;

        // Start collection task
        self.start_collection_task();

        info!("Loxone statistics collector started");
        Ok(())
    }

    /// Initialize Loxone-specific metrics
    pub async fn init_metrics(&self) -> Result<()> {
        let metrics = &self.metrics_collector;

        // Device metrics
        metrics
            .register_counter(
                "loxone_device_power_cycles_total",
                "Total device power cycles",
                HashMap::new(),
            )
            .await;

        metrics
            .register_gauge(
                "loxone_device_on_time_seconds",
                "Total time device has been on",
                HashMap::new(),
            )
            .await;

        metrics
            .register_gauge(
                "loxone_active_devices",
                "Number of currently active devices",
                HashMap::new(),
            )
            .await;

        // Energy metrics
        metrics
            .register_gauge(
                "loxone_energy_consumption_kwh",
                "Total energy consumption in kWh",
                HashMap::new(),
            )
            .await;

        metrics
            .register_gauge(
                "loxone_current_power_w",
                "Current power usage in watts",
                HashMap::new(),
            )
            .await;

        // Climate metrics
        metrics
            .register_gauge(
                "loxone_room_temperature_celsius",
                "Room temperature in Celsius",
                HashMap::new(),
            )
            .await;

        metrics
            .register_gauge(
                "loxone_room_humidity_percent",
                "Room humidity percentage",
                HashMap::new(),
            )
            .await;

        metrics
            .register_gauge(
                "loxone_room_comfort_index",
                "Room comfort index (0-100)",
                HashMap::new(),
            )
            .await;

        // Automation metrics
        metrics
            .register_counter(
                "loxone_automation_triggers_total",
                "Total automation trigger count",
                HashMap::new(),
            )
            .await;

        // System health
        metrics
            .register_gauge(
                "loxone_system_health_score",
                "Loxone system health score (0-100)",
                HashMap::new(),
            )
            .await;

        Ok(())
    }

    /// Start background collection task
    fn start_collection_task(&self) {
        let collector = self.clone();

        tokio::spawn(async move {
            let mut collection_interval = interval(collector.collection_interval);
            collection_interval.tick().await; // Skip first immediate tick

            while *collector.running.read().await {
                collection_interval.tick().await;

                if let Err(e) = collector.collect_statistics().await {
                    error!("Failed to collect Loxone statistics: {}", e);
                }
            }
        });
    }

    /// Collect all statistics
    async fn collect_statistics(&self) -> Result<()> {
        debug!("Collecting Loxone statistics");

        // Check connection status
        match self.client.is_connected().await {
            Ok(true) => {
                debug!("Connected to Loxone, collecting statistics");
            }
            Ok(false) => {
                debug!("Connection check returned false, attempting collection anyway");
                // Continue anyway - the connection might be established but state not synced
            }
            Err(e) => {
                debug!(
                    "Connection check failed: {}, attempting collection anyway",
                    e
                );
                // Continue anyway - we might still be able to collect stats
            }
        }

        // Collect device states
        let device_stats = self.collect_device_statistics().await?;

        // Collect climate data
        let climate_stats = self.collect_climate_statistics().await?;

        // Collect energy data
        let energy_stats = self.collect_energy_statistics().await?;

        // Collect automation data
        let automation_stats = self.collect_automation_statistics().await?;

        // Calculate system health
        let health_score = self
            .calculate_system_health(&device_stats, &climate_stats)
            .await;

        // Create aggregated stats
        let stats = LoxoneStats {
            timestamp: Utc::now(),
            device_usage: device_stats,
            room_climate: climate_stats,
            automations: automation_stats,
            energy: energy_stats,
            active_devices: self.count_active_devices().await,
            total_sensors: self.context.devices.read().await.len(),
            health_score,
        };

        // Update metrics
        self.update_metrics(&stats).await?;

        // Push to InfluxDB if configured
        #[cfg(feature = "influxdb")]
        if let Some(influx) = &self.influx_manager {
            self.push_to_influx(&stats, influx).await?;
        }

        // Update dashboard data
        self.update_dashboard_data(&stats).await?;

        Ok(())
    }

    /// Collect device usage statistics
    async fn collect_device_statistics(&self) -> Result<Vec<DeviceUsageStats>> {
        let devices = self.context.devices.read().await;
        let mut trackers = self.device_trackers.write().await;
        let mut stats = Vec::new();

        for device in devices.values() {
            // Skip non-controllable devices
            if !self.is_controllable_device(device) {
                continue;
            }

            // Get current state
            let current_state = self.get_device_state(device).await?;

            // Get or create tracker
            let tracker =
                trackers
                    .entry(device.uuid.clone())
                    .or_insert_with(|| DeviceStateTracker {
                        uuid: device.uuid.clone(),
                        name: device.name.clone(),
                        device_type: device.device_type.clone(),
                        room: device.room.clone(),
                        last_state: current_state.clone(),
                        last_change: Instant::now(),
                        on_since: None,
                        power_cycles: 0,
                        total_on_time: Duration::ZERO,
                    });

            // Update tracker if state changed
            if tracker.last_state != current_state {
                tracker.last_change = Instant::now();

                // Track on/off cycles
                if self.is_on_state(&current_state) && !self.is_on_state(&tracker.last_state) {
                    tracker.power_cycles += 1;
                    tracker.on_since = Some(Instant::now());
                } else if !self.is_on_state(&current_state) && self.is_on_state(&tracker.last_state)
                {
                    if let Some(on_since) = tracker.on_since {
                        tracker.total_on_time += Instant::now().duration_since(on_since);
                        tracker.on_since = None;
                    }
                }

                tracker.last_state = current_state.clone();
            }

            // Update current on time
            let mut total_on_time = tracker.total_on_time;
            if let Some(on_since) = tracker.on_since {
                total_on_time += Instant::now().duration_since(on_since);
            }

            stats.push(DeviceUsageStats {
                uuid: device.uuid.clone(),
                name: device.name.clone(),
                device_type: device.device_type.clone(),
                room: device.room.clone(),
                power_cycles: tracker.power_cycles,
                total_on_time: total_on_time.as_secs(),
                last_state_change: Utc::now()
                    - chrono::Duration::seconds(tracker.last_change.elapsed().as_secs() as i64),
                current_state,
                energy_consumed: None, // TODO: Get from energy meters if available
                average_power: None,   // TODO: Calculate from energy data
            });
        }

        Ok(stats)
    }

    /// Collect room climate statistics
    async fn collect_climate_statistics(&self) -> Result<Vec<RoomClimateStats>> {
        let devices = self.context.devices.read().await;
        let mut climate_buffer = self.climate_buffer.write().await;
        let mut room_stats: HashMap<String, RoomClimateStats> = HashMap::new();

        // Find temperature and humidity sensors
        for device in devices.values() {
            if !self.is_climate_sensor(device) {
                continue;
            }

            if let Some(room) = &device.room {
                let temp = self.get_temperature_value(device).await;
                let humidity = self.get_humidity_value(device).await;

                if temp.is_some() || humidity.is_some() {
                    // Buffer data for historical calculations
                    let entry = climate_buffer.entry(room.clone()).or_insert_with(Vec::new);
                    entry.push((Utc::now(), temp.unwrap_or(0.0), humidity));

                    // Keep only last hour of data
                    let cutoff = Utc::now() - chrono::Duration::hours(1);
                    entry.retain(|(ts, _, _)| *ts > cutoff);

                    // Calculate statistics
                    if !entry.is_empty() {
                        let temps: Vec<f64> = entry
                            .iter()
                            .filter_map(|(_, t, _)| if *t > 0.0 { Some(*t) } else { None })
                            .collect();
                        let humidities: Vec<f64> =
                            entry.iter().filter_map(|(_, _, h)| *h).collect();

                        if !temps.is_empty() {
                            let avg_temp = temps.iter().sum::<f64>() / temps.len() as f64;
                            let min_temp = temps.iter().cloned().fold(f64::INFINITY, f64::min);
                            let max_temp = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                            let avg_humidity = if !humidities.is_empty() {
                                Some(humidities.iter().sum::<f64>() / humidities.len() as f64)
                            } else {
                                None
                            };

                            // Calculate comfort index (simple formula)
                            let comfort_index =
                                self.calculate_comfort_index(avg_temp, avg_humidity);
                            let comfort_time_percent =
                                self.calculate_comfort_time_percent(&temps, &humidities);

                            room_stats.insert(
                                room.clone(),
                                RoomClimateStats {
                                    room: room.clone(),
                                    current_temperature: temp,
                                    avg_temperature: avg_temp,
                                    min_temperature: min_temp,
                                    max_temperature: max_temp,
                                    current_humidity: humidity,
                                    avg_humidity,
                                    comfort_index,
                                    comfort_time_percent,
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(room_stats.into_values().collect())
    }

    /// Collect energy statistics
    async fn collect_energy_statistics(&self) -> Result<Option<EnergyStats>> {
        // TODO: Implement energy meter reading
        // This would require identifying energy meter devices in the Loxone structure
        // and reading their values

        Ok(None)
    }

    /// Collect automation statistics
    async fn collect_automation_statistics(&self) -> Result<Vec<AutomationStats>> {
        // TODO: Implement automation/scene tracking
        // This would require monitoring scene activations and tracking their execution

        Ok(Vec::new())
    }

    /// Update metrics collector with Loxone stats
    async fn update_metrics(&self, stats: &LoxoneStats) -> Result<()> {
        let metrics = &self.metrics_collector;

        // Update device metrics
        let total_power_cycles: u64 = stats.device_usage.iter().map(|d| d.power_cycles).sum();
        let total_on_time: u64 = stats.device_usage.iter().map(|d| d.total_on_time).sum();

        metrics
            .set_gauge(
                "loxone_device_power_cycles_total",
                total_power_cycles as f64,
            )
            .await;
        metrics
            .set_gauge("loxone_device_on_time_seconds", total_on_time as f64)
            .await;
        metrics
            .set_gauge("loxone_active_devices", stats.active_devices as f64)
            .await;

        // Update climate metrics
        for climate in &stats.room_climate {
            if let Some(temp) = climate.current_temperature {
                let mut labels = HashMap::new();
                labels.insert("room".to_string(), climate.room.clone());
                metrics
                    .register_gauge(
                        &format!("loxone_room_temperature_{}", climate.room.replace(' ', "_")),
                        "Room temperature",
                        labels,
                    )
                    .await;
                metrics
                    .set_gauge(
                        &format!("loxone_room_temperature_{}", climate.room.replace(' ', "_")),
                        temp,
                    )
                    .await;
            }
        }

        // Update system health
        metrics
            .set_gauge("loxone_system_health_score", stats.health_score)
            .await;

        Ok(())
    }

    /// Push statistics to InfluxDB
    #[cfg(feature = "influxdb")]
    async fn push_to_influx(&self, stats: &LoxoneStats, influx: &InfluxManager) -> Result<()> {
        // Push device states
        for device in &stats.device_usage {
            influx
                .write_device_state(DeviceStateData {
                    uuid: device.uuid.clone(),
                    name: device.name.clone(),
                    device_type: device.device_type.clone(),
                    room: device.room.clone(),
                    state: device.current_state.clone(),
                    value: if self.is_on_state(&device.current_state) {
                        Some(1.0)
                    } else {
                        Some(0.0)
                    },
                    timestamp: stats.timestamp,
                })
                .await?;
        }

        // Push climate data
        for climate in &stats.room_climate {
            if let Some(temp) = climate.current_temperature {
                influx
                    .write_sensor_data(LoxoneSensorData {
                        uuid: format!("{}_temperature", climate.room),
                        name: format!("{} Temperature", climate.room),
                        sensor_type: "temperature".to_string(),
                        room: Some(climate.room.clone()),
                        value: temp,
                        unit: "°C".to_string(),
                        timestamp: stats.timestamp,
                    })
                    .await?;
            }

            if let Some(humidity) = climate.current_humidity {
                influx
                    .write_sensor_data(LoxoneSensorData {
                        uuid: format!("{}_humidity", climate.room),
                        name: format!("{} Humidity", climate.room),
                        sensor_type: "humidity".to_string(),
                        room: Some(climate.room.clone()),
                        value: humidity,
                        unit: "%".to_string(),
                        timestamp: stats.timestamp,
                    })
                    .await?;
            }
        }

        Ok(())
    }

    /// Update dashboard with Loxone-specific data
    async fn update_dashboard_data(&self, _stats: &LoxoneStats) -> Result<()> {
        // Dashboard data is updated via the metrics collector
        // The dashboard will automatically pick up the new metrics
        Ok(())
    }

    /// Check if device is controllable
    pub fn is_controllable_device(&self, device: &LoxoneDevice) -> bool {
        matches!(
            device.device_type.to_lowercase().as_str(),
            t if t.contains("light") ||
                 t.contains("switch") ||
                 t.contains("dimmer") ||
                 t.contains("jalousie") ||
                 t.contains("blind")
        )
    }

    /// Check if device is a climate sensor
    pub fn is_climate_sensor(&self, device: &LoxoneDevice) -> bool {
        device.device_type.to_lowercase().contains("temperature")
            || device.device_type.to_lowercase().contains("humidity")
            || device.device_type.to_lowercase().contains("climate")
    }

    /// Get device state
    async fn get_device_state(&self, device: &LoxoneDevice) -> Result<String> {
        // Try to get state from device states
        if let Some(state) = device.states.get("value") {
            return Ok(state.to_string());
        }

        // For lights/switches, check active state
        if let Some(active) = device.states.get("active") {
            return Ok(if active.as_bool().unwrap_or(false) {
                "on".to_string()
            } else {
                "off".to_string()
            });
        }

        // For blinds, check position
        if let Some(position) = device.states.get("position") {
            return Ok(format!("{}%", position.as_f64().unwrap_or(0.0) * 100.0));
        }

        Ok("unknown".to_string())
    }

    /// Check if state represents "on"
    pub fn is_on_state(&self, state: &str) -> bool {
        state == "on" || state == "1" || state == "true"
    }

    /// Get temperature value from device
    pub async fn get_temperature_value(&self, device: &LoxoneDevice) -> Option<f64> {
        device
            .states
            .get("value")
            .and_then(|v| v.as_f64())
            .or_else(|| device.states.get("temperature").and_then(|v| v.as_f64()))
    }

    /// Get humidity value from device
    async fn get_humidity_value(&self, device: &LoxoneDevice) -> Option<f64> {
        device.states.get("humidity").and_then(|v| v.as_f64())
    }

    /// Calculate comfort index based on temperature and humidity
    pub fn calculate_comfort_index(&self, temp: f64, humidity: Option<f64>) -> f64 {
        // Simple comfort index calculation
        // Ideal temp: 20-24°C, ideal humidity: 40-60%
        let temp_score: f64 = if (20.0..=24.0).contains(&temp) {
            100.0
        } else if (18.0..=26.0).contains(&temp) {
            80.0
        } else if (16.0..=28.0).contains(&temp) {
            60.0
        } else {
            40.0
        };

        let humidity_score: f64 = if let Some(h) = humidity {
            if (40.0..=60.0).contains(&h) {
                100.0
            } else if (30.0..=70.0).contains(&h) {
                80.0
            } else {
                60.0
            }
        } else {
            80.0 // No humidity data, assume ok
        };

        (temp_score * 0.7 + humidity_score * 0.3).min(100.0)
    }

    /// Calculate percentage of time in comfort range
    fn calculate_comfort_time_percent(&self, temps: &[f64], humidities: &[f64]) -> f64 {
        if temps.is_empty() {
            return 0.0;
        }

        let comfort_count = temps
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                let humidity = humidities.get(*i).copied();
                self.calculate_comfort_index(**t, humidity) >= 80.0
            })
            .count();

        (comfort_count as f64 / temps.len() as f64) * 100.0
    }

    /// Count active devices
    async fn count_active_devices(&self) -> usize {
        let trackers = self.device_trackers.read().await;
        trackers
            .values()
            .filter(|t| self.is_on_state(&t.last_state))
            .count()
    }

    /// Calculate system health score
    async fn calculate_system_health(
        &self,
        device_stats: &[DeviceUsageStats],
        climate_stats: &[RoomClimateStats],
    ) -> f64 {
        let mut score: f64 = 100.0;

        // Penalize if too many devices are constantly on
        let always_on_devices = device_stats
            .iter()
            .filter(|d| {
                d.total_on_time > 86400 * 7 // On for more than 7 days straight
            })
            .count();
        if always_on_devices > 5 {
            score -= 10.0;
        }

        // Check climate comfort
        let avg_comfort: f64 = if !climate_stats.is_empty() {
            climate_stats.iter().map(|c| c.comfort_index).sum::<f64>() / climate_stats.len() as f64
        } else {
            80.0
        };
        if avg_comfort < 70.0 {
            score -= 15.0;
        }

        // TODO: Add more health checks
        // - Check for unresponsive devices
        // - Check for abnormal energy consumption
        // - Check for failed automations

        score.max(0.0f64)
    }

    /// Stop statistics collection
    pub async fn stop(&self) {
        *self.running.write().await = false;
        info!("Loxone statistics collector stopped");
    }
}

impl Clone for LoxoneStatsCollector {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            context: self.context.clone(),
            metrics_collector: self.metrics_collector.clone(),
            #[cfg(feature = "influxdb")]
            influx_manager: self.influx_manager.clone(),
            device_trackers: self.device_trackers.clone(),
            climate_buffer: self.climate_buffer.clone(),
            automation_counters: self.automation_counters.clone(),
            collection_interval: self.collection_interval,
            running: self.running.clone(),
        }
    }
}

/// Helper function to check if a sensor value represents an "open" state
#[allow(dead_code)]
fn is_open_state(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|v| v > 0.5).unwrap_or(false),
        serde_json::Value::String(s) => {
            matches!(s.to_lowercase().as_str(), "open" | "on" | "true" | "1")
        }
        _ => false,
    }
}

/// Convert sensor value to human-readable string
#[allow(dead_code)]
fn human_readable_state(value: &serde_json::Value, sensor_type: Option<&SensorType>) -> String {
    match sensor_type {
        Some(SensorType::DoorWindow) => {
            if is_open_state(value) {
                "Open".to_string()
            } else {
                "Closed".to_string()
            }
        }
        Some(SensorType::Motion) => {
            if value.as_bool().unwrap_or(false) {
                "Motion detected".to_string()
            } else {
                "No motion".to_string()
            }
        }
        Some(SensorType::Temperature) => {
            if let Some(temp) = value.as_f64() {
                format!("{temp:.1}°C")
            } else {
                "Unknown".to_string()
            }
        }
        Some(SensorType::Analog) => {
            if let Some(val) = value.as_f64() {
                format!("{val:.1}")
            } else {
                "Unknown".to_string()
            }
        }
        Some(SensorType::Light) => {
            if let Some(lux) = value.as_f64() {
                format!("{lux:.0} lux")
            } else {
                "Unknown".to_string()
            }
        }
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comfort_index_calculation() {
        let collector = LoxoneStatsCollector {
            client: Arc::new(crate::mock::MockLoxoneClient::new()),
            context: Arc::new(ClientContext::new()),
            metrics_collector: Arc::new(MetricsCollector::new()),
            #[cfg(feature = "influxdb")]
            influx_manager: None,
            device_trackers: Arc::new(RwLock::new(HashMap::new())),
            climate_buffer: Arc::new(RwLock::new(HashMap::new())),
            automation_counters: Arc::new(RwLock::new(HashMap::new())),
            collection_interval: Duration::from_secs(60),
            running: Arc::new(RwLock::new(false)),
        };

        // Perfect conditions
        assert_eq!(collector.calculate_comfort_index(22.0, Some(50.0)), 100.0);

        // Good conditions
        assert!(collector.calculate_comfort_index(25.0, Some(55.0)) > 80.0);

        // Poor conditions
        assert!(collector.calculate_comfort_index(30.0, Some(80.0)) < 70.0);
    }

    #[test]
    fn test_is_open_state() {
        assert!(is_open_state(&serde_json::json!(true)));
        assert!(is_open_state(&serde_json::json!(1)));
        assert!(is_open_state(&serde_json::json!("open")));
        assert!(is_open_state(&serde_json::json!("Open")));
        assert!(!is_open_state(&serde_json::json!(false)));
        assert!(!is_open_state(&serde_json::json!(0)));
        assert!(!is_open_state(&serde_json::json!("closed")));
    }
}
