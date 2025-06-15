//! Hot storage implementation - in-memory storage for recent data

use super::config::HotStorageConfig;
use super::events::*;
use crate::error::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Hot data store for recent events in memory
pub struct HotDataStore {
    /// Configuration
    config: HotStorageConfig,

    /// Device state events by device UUID
    device_states: Arc<RwLock<HashMap<String, VecDeque<HistoricalEvent>>>>,

    /// Sensor readings by sensor UUID
    sensor_readings: Arc<RwLock<HashMap<String, VecDeque<HistoricalEvent>>>>,

    /// System metrics
    system_metrics: Arc<RwLock<VecDeque<HistoricalEvent>>>,

    /// Audit events
    audit_events: Arc<RwLock<VecDeque<HistoricalEvent>>>,

    /// Discovery events
    discovery_events: Arc<RwLock<VecDeque<HistoricalEvent>>>,

    /// Response cache events
    response_cache: Arc<RwLock<HashMap<String, HistoricalEvent>>>,

    /// Statistics
    stats: Arc<RwLock<HotStorageStats>>,
}

/// Statistics for hot storage
#[derive(Debug, Default)]
struct HotStorageStats {
    total_events: u64,
    events_evicted: u64,
    device_count: usize,
    sensor_count: usize,
}

impl HotDataStore {
    /// Create new hot data store
    pub fn new(config: HotStorageConfig) -> Self {
        Self {
            config,
            device_states: Arc::new(RwLock::new(HashMap::new())),
            sensor_readings: Arc::new(RwLock::new(HashMap::new())),
            system_metrics: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            audit_events: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            discovery_events: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            response_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HotStorageStats::default())),
        }
    }

    /// Insert a new event
    pub async fn insert(&self, event: HistoricalEvent) -> Result<()> {
        trace!("Inserting event: {:?}", event.id);

        match &event.category {
            EventCategory::DeviceState(state) => {
                self.insert_device_event(state.device_uuid.clone(), event)
                    .await?;
            }
            EventCategory::SensorReading(data) => {
                self.insert_sensor_event(data.sensor_uuid.clone(), event)
                    .await?;
            }
            EventCategory::SystemMetric(_) => {
                self.insert_system_metric(event).await?;
            }
            EventCategory::AuditEvent(_) => {
                self.insert_audit_event(event).await?;
            }
            EventCategory::DiscoveryEvent(_) => {
                self.insert_discovery_event(event).await?;
            }
            EventCategory::ResponseCache(data) => {
                self.insert_response_cache(data.key.clone(), event).await?;
            }
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_events += 1;

        Ok(())
    }

    /// Insert device state event
    async fn insert_device_event(&self, device_uuid: String, event: HistoricalEvent) -> Result<()> {
        let mut device_states = self.device_states.write().await;
        let queue = device_states
            .entry(device_uuid)
            .or_insert_with(|| VecDeque::with_capacity(self.config.device_events_limit));

        // Maintain size limit
        if queue.len() >= self.config.device_events_limit {
            queue.pop_front();
            self.stats.write().await.events_evicted += 1;
        }

        queue.push_back(event);

        // Update device count
        self.stats.write().await.device_count = device_states.len();

        Ok(())
    }

    /// Insert sensor reading event
    async fn insert_sensor_event(&self, sensor_uuid: String, event: HistoricalEvent) -> Result<()> {
        let mut sensor_readings = self.sensor_readings.write().await;
        let queue = sensor_readings.entry(sensor_uuid).or_insert_with(|| {
            // Calculate capacity based on retention time (assuming 1Hz updates)
            let capacity = self.config.sensor_retention_seconds as usize;
            VecDeque::with_capacity(capacity)
        });

        // Remove old events based on time
        let cutoff = chrono::Utc::now()
            - chrono::Duration::seconds(self.config.sensor_retention_seconds as i64);
        while let Some(front) = queue.front() {
            if front.timestamp < cutoff {
                queue.pop_front();
                self.stats.write().await.events_evicted += 1;
            } else {
                break;
            }
        }

        queue.push_back(event);

        // Update sensor count
        self.stats.write().await.sensor_count = sensor_readings.len();

        Ok(())
    }

    /// Insert system metric event
    async fn insert_system_metric(&self, event: HistoricalEvent) -> Result<()> {
        let mut metrics = self.system_metrics.write().await;

        // Remove old events based on time
        let cutoff = chrono::Utc::now()
            - chrono::Duration::seconds(self.config.metrics_retention_seconds as i64);
        while let Some(front) = metrics.front() {
            if front.timestamp < cutoff {
                metrics.pop_front();
                self.stats.write().await.events_evicted += 1;
            } else {
                break;
            }
        }

        metrics.push_back(event);
        Ok(())
    }

    /// Insert audit event
    async fn insert_audit_event(&self, event: HistoricalEvent) -> Result<()> {
        let mut audit_events = self.audit_events.write().await;

        if audit_events.len() >= self.config.audit_events_limit {
            audit_events.pop_front();
            self.stats.write().await.events_evicted += 1;
        }

        audit_events.push_back(event);
        Ok(())
    }

    /// Insert discovery event
    async fn insert_discovery_event(&self, event: HistoricalEvent) -> Result<()> {
        let mut discovery_events = self.discovery_events.write().await;

        if discovery_events.len() >= 100 {
            discovery_events.pop_front();
            self.stats.write().await.events_evicted += 1;
        }

        discovery_events.push_back(event);
        Ok(())
    }

    /// Insert response cache event
    async fn insert_response_cache(&self, key: String, event: HistoricalEvent) -> Result<()> {
        let mut cache = self.response_cache.write().await;

        // Remove expired entries
        let now = chrono::Utc::now();
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, e)| {
                if let EventCategory::ResponseCache(data) = &e.category {
                    e.timestamp + chrono::Duration::seconds(data.ttl as i64) < now
                } else {
                    false
                }
            })
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired_keys {
            cache.remove(&key);
            self.stats.write().await.events_evicted += 1;
        }

        cache.insert(key, event);
        Ok(())
    }

    /// Get recent device events
    pub async fn get_device_events(
        &self,
        device_uuid: &str,
        limit: Option<usize>,
    ) -> Vec<HistoricalEvent> {
        let device_states = self.device_states.read().await;

        if let Some(queue) = device_states.get(device_uuid) {
            let events: Vec<_> = queue.iter().cloned().collect();
            match limit {
                Some(n) => events.into_iter().rev().take(n).collect(),
                None => events,
            }
        } else {
            Vec::new()
        }
    }

    /// Get recent sensor readings
    pub async fn get_sensor_readings(
        &self,
        sensor_uuid: &str,
        since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Vec<HistoricalEvent> {
        let sensor_readings = self.sensor_readings.read().await;

        if let Some(queue) = sensor_readings.get(sensor_uuid) {
            match since {
                Some(cutoff) => queue
                    .iter()
                    .filter(|e| e.timestamp >= cutoff)
                    .cloned()
                    .collect(),
                None => queue.iter().cloned().collect(),
            }
        } else {
            Vec::new()
        }
    }

    /// Get all events of a specific category
    pub async fn get_events_by_category(
        &self,
        category: &str,
        limit: Option<usize>,
    ) -> Vec<HistoricalEvent> {
        let mut events = Vec::new();

        match category {
            "device_state" => {
                let device_states = self.device_states.read().await;
                for queue in device_states.values() {
                    events.extend(queue.iter().cloned());
                }
            }
            "sensor_reading" => {
                let sensor_readings = self.sensor_readings.read().await;
                for queue in sensor_readings.values() {
                    events.extend(queue.iter().cloned());
                }
            }
            "system_metric" => {
                let metrics = self.system_metrics.read().await;
                events.extend(metrics.iter().cloned());
            }
            "audit_event" => {
                let audit = self.audit_events.read().await;
                events.extend(audit.iter().cloned());
            }
            "discovery_event" => {
                let discovery = self.discovery_events.read().await;
                events.extend(discovery.iter().cloned());
            }
            _ => {}
        }

        // Sort by timestamp (newest first)
        events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        match limit {
            Some(n) => events.into_iter().take(n).collect(),
            None => events,
        }
    }

    /// Check if we need to tier data to cold storage
    pub async fn needs_tiering(&self) -> bool {
        let stats = self.stats.read().await;

        // Tier if we've evicted more than 10% of total events
        if stats.events_evicted as f64 > stats.total_events as f64 * 0.1 {
            return true;
        }

        // Check individual storage pressure
        let device_states = self.device_states.read().await;
        for queue in device_states.values() {
            if queue.len() > self.config.device_events_limit * 9 / 10 {
                return true;
            }
        }

        false
    }

    /// Get events ready for tiering
    pub async fn get_tiering_candidates(&self) -> Vec<HistoricalEvent> {
        let mut candidates = Vec::new();

        // Get oldest device events
        let device_states = self.device_states.read().await;
        for (_, queue) in device_states.iter() {
            if queue.len() > self.config.device_events_limit / 2 {
                // Take the oldest half
                let take_count = queue.len() / 2;
                candidates.extend(queue.iter().take(take_count).cloned());
            }
        }

        // Get old sensor readings
        let cutoff = chrono::Utc::now()
            - chrono::Duration::seconds((self.config.sensor_retention_seconds / 2) as i64);
        let sensor_readings = self.sensor_readings.read().await;
        for (_, queue) in sensor_readings.iter() {
            candidates.extend(queue.iter().filter(|e| e.timestamp < cutoff).cloned());
        }

        debug!("Found {} events ready for tiering", candidates.len());
        candidates
    }

    /// Remove events after successful tiering
    pub async fn remove_tiered_events(&self, event_ids: &[uuid::Uuid]) -> Result<()> {
        let event_id_set: std::collections::HashSet<_> = event_ids.iter().collect();

        // Remove from device states
        let mut device_states = self.device_states.write().await;
        for queue in device_states.values_mut() {
            queue.retain(|e| !event_id_set.contains(&e.id));
        }

        // Remove from sensor readings
        let mut sensor_readings = self.sensor_readings.write().await;
        for queue in sensor_readings.values_mut() {
            queue.retain(|e| !event_id_set.contains(&e.id));
        }

        // Remove from other collections
        let mut system_metrics = self.system_metrics.write().await;
        system_metrics.retain(|e| !event_id_set.contains(&e.id));

        let mut audit_events = self.audit_events.write().await;
        audit_events.retain(|e| !event_id_set.contains(&e.id));

        debug!("Removed {} tiered events from hot storage", event_ids.len());
        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> (u64, u64, usize, usize) {
        let stats = self.stats.read().await;
        (
            stats.total_events,
            stats.events_evicted,
            stats.device_count,
            stats.sensor_count,
        )
    }
}
