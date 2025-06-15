//! Core unified history store implementation

use super::cold_storage::ColdDataStore;
use super::config::HistoryConfig;
use super::events::*;
use super::hot_storage::HotDataStore;
use super::query::QueryBuilder;
use super::tiering::TieringManager;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

/// Main unified history store
pub struct UnifiedHistoryStore {
    /// Configuration
    config: HistoryConfig,

    /// Hot storage for recent data
    hot_store: Arc<RwLock<HotDataStore>>,

    /// Cold storage for historical data
    cold_store: Arc<ColdDataStore>,

    /// Event broadcast channel
    event_bus: broadcast::Sender<HistoricalEvent>,

    /// Tiering manager
    tiering_manager: Arc<TieringManager>,

    /// Running flag
    running: Arc<RwLock<bool>>,
}

impl UnifiedHistoryStore {
    /// Create new unified history store
    pub async fn new(config: HistoryConfig) -> Result<Self> {
        // Validate config
        config.validate().map_err(|e| {
            crate::error::LoxoneError::config(format!("Invalid history config: {}", e))
        })?;

        // Create hot storage
        let hot_store = Arc::new(RwLock::new(HotDataStore::new(config.hot_storage.clone())));

        // Create cold storage
        let cold_store = Arc::new(ColdDataStore::new(config.cold_storage.clone()).await?);

        // Create event bus
        let (tx, _) = broadcast::channel(config.streaming.buffer_size);

        // Create tiering manager
        let tiering_manager = Arc::new(TieringManager::new(
            hot_store.clone(),
            cold_store.clone(),
            config.performance.tiering_interval_seconds,
        ));

        let store = Self {
            config,
            hot_store,
            cold_store,
            event_bus: tx,
            tiering_manager,
            running: Arc::new(RwLock::new(false)),
        };

        // Start background tasks
        store.start_background_tasks().await;

        Ok(store)
    }

    /// Record a new event
    pub async fn record(&self, event: HistoricalEvent) -> Result<()> {
        // Write to hot storage
        self.hot_store.write().await.insert(event.clone()).await?;

        // Broadcast to subscribers
        let _ = self.event_bus.send(event.clone());

        // Log high-priority events
        match &event.category {
            EventCategory::AuditEvent(audit) if matches!(audit.result, AuditResult::Failure) => {
                info!(
                    "Audit failure recorded: {} by {}",
                    audit.action, audit.actor
                );
            }
            EventCategory::SystemMetric(metric) if metric.metric_name.contains("error") => {
                debug!(
                    "Error metric recorded: {} = {}",
                    metric.metric_name, metric.value
                );
            }
            _ => {}
        }

        Ok(())
    }

    /// Create a query builder
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(self.hot_store.clone(), self.cold_store.clone())
    }

    /// Subscribe to real-time events
    pub fn subscribe(&self) -> broadcast::Receiver<HistoricalEvent> {
        self.event_bus.subscribe()
    }

    /// Get a specific type of event stream
    pub fn subscribe_filtered(
        &self,
        category: String,
    ) -> impl futures::Stream<Item = HistoricalEvent> {
        let mut rx = self.event_bus.subscribe();

        async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                let matches = match &event.category {
                    EventCategory::DeviceState(_) => category == "device_state",
                    EventCategory::SensorReading(_) => category == "sensor_reading",
                    EventCategory::SystemMetric(_) => category == "system_metric",
                    EventCategory::AuditEvent(_) => category == "audit_event",
                    EventCategory::DiscoveryEvent(_) => category == "discovery_event",
                    EventCategory::ResponseCache(_) => category == "response_cache",
                };

                if matches {
                    yield event;
                }
            }
        }
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        *self.running.write().await = true;

        // Start tiering task
        self.start_tiering_task();

        // Start cleanup task
        self.start_cleanup_task();

        // Start metrics collection
        self.start_metrics_task();
    }

    /// Start automatic tiering task
    fn start_tiering_task(&self) {
        let tiering_manager = self.tiering_manager.clone();
        let running = self.running.clone();
        let interval_secs = self.config.performance.tiering_interval_seconds;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            while *running.read().await {
                interval.tick().await;

                if let Err(e) = tiering_manager.run_tiering_cycle().await {
                    error!("Tiering cycle failed: {}", e);
                }
            }
        });
    }

    /// Start cleanup task
    fn start_cleanup_task(&self) {
        let cold_store = self.cold_store.clone();
        let retention = self.config.retention.clone();
        let running = self.running.clone();
        let interval_secs = self.config.performance.cleanup_interval_seconds;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            while *running.read().await {
                interval.tick().await;

                let mut retention_map = std::collections::HashMap::new();
                retention_map.insert("device_state".to_string(), retention.device_states_days);
                retention_map.insert("sensor_reading".to_string(), retention.sensor_data_days);
                retention_map.insert("system_metric".to_string(), retention.system_metrics_days);
                retention_map.insert("audit_event".to_string(), retention.audit_events_days);
                retention_map.insert(
                    "discovery_event".to_string(),
                    retention.discovery_cache_days,
                );

                if let Err(e) = cold_store.cleanup(retention_map).await {
                    error!("Cleanup task failed: {}", e);
                }
            }
        });
    }

    /// Start metrics collection task
    fn start_metrics_task(&self) {
        let hot_store = self.hot_store.clone();
        let cold_store = self.cold_store.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Every minute

            while *running.read().await {
                interval.tick().await;

                // Collect hot storage stats
                let (total_events, events_evicted, device_count, sensor_count) =
                    hot_store.read().await.get_stats().await;

                debug!(
                    "Hot storage: {} events, {} evicted, {} devices, {} sensors",
                    total_events, events_evicted, device_count, sensor_count
                );

                // Collect cold storage stats
                let (file_count, event_count, compression_ratio) = cold_store.get_stats().await;

                debug!(
                    "Cold storage: {} files, {} events, {:.2} compression ratio",
                    file_count, event_count, compression_ratio
                );
            }
        });
    }

    /// Stop the history store
    pub async fn stop(&self) {
        info!("Stopping unified history store");
        *self.running.write().await = false;
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> StorageStats {
        let (hot_total, hot_evicted, device_count, sensor_count) =
            self.hot_store.read().await.get_stats().await;

        let (cold_files, cold_events, compression_ratio) = self.cold_store.get_stats().await;

        StorageStats {
            hot_storage: HotStorageStats {
                total_events: hot_total,
                events_evicted: hot_evicted,
                device_count,
                sensor_count,
            },
            cold_storage: ColdStorageStats {
                file_count: cold_files,
                event_count: cold_events,
                compression_ratio,
            },
            total_events: hot_total + cold_events,
            subscribers: self.event_bus.receiver_count(),
        }
    }
}

/// Storage statistics
#[derive(Debug, serde::Serialize)]
pub struct StorageStats {
    pub hot_storage: HotStorageStats,
    pub cold_storage: ColdStorageStats,
    pub total_events: u64,
    pub subscribers: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct HotStorageStats {
    pub total_events: u64,
    pub events_evicted: u64,
    pub device_count: usize,
    pub sensor_count: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct ColdStorageStats {
    pub file_count: usize,
    pub event_count: u64,
    pub compression_ratio: f64,
}

/// Convenience methods for common queries
impl UnifiedHistoryStore {
    /// Get recent device state changes
    pub async fn get_recent_device_changes(&self, limit: usize) -> Result<Vec<HistoricalEvent>> {
        Ok(self
            .query()
            .category("device_state")
            .limit(limit)
            .execute()
            .await?
            .events)
    }

    /// Get sensor readings for a specific sensor
    pub async fn get_sensor_history(
        &self,
        sensor_uuid: &str,
        hours: u32,
    ) -> Result<Vec<HistoricalEvent>> {
        let since = chrono::Utc::now() - chrono::Duration::hours(hours as i64);

        Ok(self
            .query()
            .category("sensor_reading")
            .entity_id(sensor_uuid)
            .since(since)
            .execute()
            .await?
            .events)
    }

    /// Get audit trail for a user
    pub async fn get_user_audit_trail(&self, user_id: &str) -> Result<Vec<HistoricalEvent>> {
        Ok(self
            .query()
            .category("audit_event")
            .source_type(EventSource::User(user_id.to_string()))
            .execute()
            .await?
            .events)
    }

    /// Get system metrics for a time range
    pub async fn get_system_metrics(
        &self,
        metric_name: &str,
        hours: u32,
    ) -> Result<Vec<HistoricalEvent>> {
        let since = chrono::Utc::now() - chrono::Duration::hours(hours as i64);

        let events = self
            .query()
            .category("system_metric")
            .since(since)
            .execute()
            .await?
            .events;

        // Filter by metric name
        Ok(events
            .into_iter()
            .filter(|e| {
                if let EventCategory::SystemMetric(ref data) = e.category {
                    data.metric_name == metric_name
                } else {
                    false
                }
            })
            .collect())
    }
}

// Add async-stream dependency to Cargo.toml for the filtered subscription
