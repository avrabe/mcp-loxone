//! Centralized state management with change detection
//!
//! This module provides a unified state management system that tracks
//! device state changes, manages subscriptions, and enables real-time
//! notifications across the entire system.

use crate::error::Result;
use crate::services::sensor_registry::SensorType;
use crate::services::value_resolution::{ResolvedValue, UnifiedValueResolver};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

/// Centralized state manager with change detection
pub struct StateManager {
    /// Current device states
    device_states: Arc<RwLock<HashMap<String, DeviceState>>>,
    /// Change detection system
    change_detector: Arc<ChangeDetector>,
    /// State history for analysis
    state_history: Arc<RwLock<StateHistory>>,
    /// Subscription manager for notifications
    subscription_manager: Arc<SubscriptionManager>,
    /// Value resolver for processing states
    value_resolver: Arc<UnifiedValueResolver>,
    /// Background task handles
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Device state with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub uuid: String,
    pub name: String,
    pub device_type: String,
    pub room: Option<String>,
    pub resolved_value: Option<ResolvedValue>,
    pub raw_state: serde_json::Value,
    pub last_updated: DateTime<Utc>,
    pub change_count: u64,
    pub state_quality: StateQuality,
}

/// Quality assessment of state data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateQuality {
    Fresh,   // Recently updated, high confidence
    Good,    // Reasonably recent, medium confidence
    Stale,   // Old data, low confidence
    Unknown, // No data or parsing failed
}

/// State change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub device_uuid: String,
    pub device_name: String,
    pub device_type: String,
    pub room: Option<String>,
    pub old_value: Option<ResolvedValue>,
    pub new_value: Option<ResolvedValue>,
    pub change_type: ChangeType,
    pub timestamp: DateTime<Utc>,
    pub significance: ChangeSignificance,
}

/// Type of state change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    ValueChanged,
    DeviceOnline,
    DeviceOffline,
    QualityChanged,
    FirstSeen,
    Error,
}

/// Significance of the change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeSignificance {
    Critical, // Important state changes (alarms, security)
    Major,    // Significant value changes (temperature, doors)
    Minor,    // Small adjustments (dimmer levels)
    Trivial,  // Noise or expected fluctuations
}

/// Change detection system
pub struct ChangeDetector {
    /// Thresholds for different sensor types
    #[allow(dead_code)]
    change_thresholds: HashMap<SensorType, f64>,
    /// Minimum time between change notifications
    change_debounce: Duration,
    /// Last notification time per device
    last_notification: RwLock<HashMap<String, DateTime<Utc>>>,
}

/// State history tracking
pub struct StateHistory {
    /// Recent changes for each device
    device_changes: HashMap<String, VecDeque<StateChangeEvent>>,
    /// Maximum history length per device
    max_history_length: usize,
    /// Global change statistics
    change_statistics: ChangeStatistics,
}

/// Statistics about state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeStatistics {
    pub total_changes: u64,
    pub changes_by_type: HashMap<String, u64>,
    pub changes_by_device_type: HashMap<String, u64>,
    pub changes_by_room: HashMap<String, u64>,
    pub most_active_devices: Vec<(String, u64)>,
    pub change_rate_per_hour: f64,
}

/// Subscription management for state changes
pub struct SubscriptionManager {
    /// Broadcast channel for all state changes
    global_sender: broadcast::Sender<StateChangeEvent>,
    /// Device-specific subscriptions
    device_subscriptions: RwLock<HashMap<String, broadcast::Sender<StateChangeEvent>>>,
    /// Room-specific subscriptions
    room_subscriptions: RwLock<HashMap<String, broadcast::Sender<StateChangeEvent>>>,
    /// Type-specific subscriptions
    type_subscriptions: RwLock<HashMap<String, broadcast::Sender<StateChangeEvent>>>,
}

impl StateManager {
    /// Create new state manager
    pub async fn new(value_resolver: Arc<UnifiedValueResolver>) -> Result<Self> {
        let (global_sender, _) = broadcast::channel(1000);

        let subscription_manager = Arc::new(SubscriptionManager {
            global_sender,
            device_subscriptions: RwLock::new(HashMap::new()),
            room_subscriptions: RwLock::new(HashMap::new()),
            type_subscriptions: RwLock::new(HashMap::new()),
        });

        let change_detector = Arc::new(ChangeDetector::new());
        let state_history = Arc::new(RwLock::new(StateHistory::new()));

        let state_manager = Self {
            device_states: Arc::new(RwLock::new(HashMap::new())),
            change_detector,
            state_history,
            subscription_manager,
            value_resolver,
            _background_tasks: Vec::new(),
        };

        Ok(state_manager)
    }

    /// Start the state manager background tasks
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting centralized state manager");

        // Start periodic state polling
        let state_poll_task = self.start_state_polling().await;
        self._background_tasks.push(state_poll_task);

        // Start change analysis task
        let analysis_task = self.start_change_analysis().await;
        self._background_tasks.push(analysis_task);

        // Start cleanup task
        let cleanup_task = self.start_cleanup_task().await;
        self._background_tasks.push(cleanup_task);

        info!("State manager started successfully");
        Ok(())
    }

    /// Update device state and detect changes
    pub async fn update_device_state(
        &self,
        uuid: &str,
        raw_state: serde_json::Value,
    ) -> Result<Option<StateChangeEvent>> {
        // Resolve the state value
        let resolved_value = match self.value_resolver.resolve_device_value(uuid).await {
            Ok(resolved) => Some(resolved),
            Err(e) => {
                warn!("Failed to resolve value for device {}: {}", uuid, e);
                None
            }
        };

        let now = Utc::now();
        let mut states = self.device_states.write().await;

        // Get previous state
        let previous_state = states.get(uuid).cloned();

        // Determine state quality
        let quality = self.assess_state_quality(&resolved_value, &raw_state);

        // Create new device state
        let new_state = DeviceState {
            uuid: uuid.to_string(),
            name: resolved_value
                .as_ref()
                .map(|r| r.device_name.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            device_type: resolved_value
                .as_ref()
                .map(|r| {
                    if let Some(sensor_type) = &r.sensor_type {
                        format!("{:?}", sensor_type)
                    } else {
                        "Unknown".to_string()
                    }
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            room: resolved_value.as_ref().and_then(|r| r.room.clone()),
            resolved_value: resolved_value.clone(),
            raw_state,
            last_updated: now,
            change_count: previous_state
                .as_ref()
                .map(|s| s.change_count + 1)
                .unwrap_or(1),
            state_quality: quality,
        };

        // Detect if this is a significant change
        let change_event = if let Some(prev) = &previous_state {
            self.detect_change(prev, &new_state).await
        } else {
            // First time seeing this device
            Some(StateChangeEvent {
                device_uuid: uuid.to_string(),
                device_name: new_state.name.clone(),
                device_type: new_state.device_type.clone(),
                room: new_state.room.clone(),
                old_value: None,
                new_value: resolved_value,
                change_type: ChangeType::FirstSeen,
                timestamp: now,
                significance: ChangeSignificance::Minor,
            })
        };

        // Update state
        states.insert(uuid.to_string(), new_state);
        drop(states);

        // Process change event if significant
        if let Some(event) = &change_event {
            self.process_change_event(event.clone()).await?;
        }

        Ok(change_event)
    }

    /// Get current state for a device
    pub async fn get_device_state(&self, uuid: &str) -> Option<DeviceState> {
        let states = self.device_states.read().await;
        states.get(uuid).cloned()
    }

    /// Get all current device states
    pub async fn get_all_device_states(&self) -> HashMap<String, DeviceState> {
        let states = self.device_states.read().await;
        states.clone()
    }

    /// Get recent changes for a device
    pub async fn get_device_history(
        &self,
        uuid: &str,
        limit: Option<usize>,
    ) -> Vec<StateChangeEvent> {
        let history = self.state_history.read().await;
        if let Some(device_history) = history.device_changes.get(uuid) {
            let limit = limit.unwrap_or(50);
            device_history.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Subscribe to state changes for a specific device
    pub async fn subscribe_to_device(&self, uuid: &str) -> broadcast::Receiver<StateChangeEvent> {
        let mut subscriptions = self.subscription_manager.device_subscriptions.write().await;

        if let Some(sender) = subscriptions.get(uuid) {
            sender.subscribe()
        } else {
            let (sender, receiver) = broadcast::channel(100);
            subscriptions.insert(uuid.to_string(), sender);
            receiver
        }
    }

    /// Subscribe to state changes for a specific room
    pub async fn subscribe_to_room(&self, room: &str) -> broadcast::Receiver<StateChangeEvent> {
        let mut subscriptions = self.subscription_manager.room_subscriptions.write().await;

        if let Some(sender) = subscriptions.get(room) {
            sender.subscribe()
        } else {
            let (sender, receiver) = broadcast::channel(100);
            subscriptions.insert(room.to_string(), sender);
            receiver
        }
    }

    /// Subscribe to all state changes
    pub async fn subscribe_to_all(&self) -> broadcast::Receiver<StateChangeEvent> {
        self.subscription_manager.global_sender.subscribe()
    }

    /// Get change statistics
    pub async fn get_change_statistics(&self) -> ChangeStatistics {
        let history = self.state_history.read().await;
        history.change_statistics.clone()
    }

    /// Assess the quality of state data
    fn assess_state_quality(
        &self,
        resolved_value: &Option<ResolvedValue>,
        _raw_state: &serde_json::Value,
    ) -> StateQuality {
        match resolved_value {
            Some(resolved) => {
                if resolved.confidence > 0.8 {
                    StateQuality::Fresh
                } else if resolved.confidence > 0.5 {
                    StateQuality::Good
                } else {
                    StateQuality::Stale
                }
            }
            None => StateQuality::Unknown,
        }
    }

    /// Detect if a state change is significant
    async fn detect_change(
        &self,
        old_state: &DeviceState,
        new_state: &DeviceState,
    ) -> Option<StateChangeEvent> {
        // Check if enough time has passed since last notification
        if !self.change_detector.should_notify(&new_state.uuid).await {
            return None;
        }

        let change_type = self.determine_change_type(old_state, new_state);
        let significance = self.determine_change_significance(old_state, new_state);

        // Only report significant changes
        if significance == ChangeSignificance::Trivial {
            return None;
        }

        Some(StateChangeEvent {
            device_uuid: new_state.uuid.clone(),
            device_name: new_state.name.clone(),
            device_type: new_state.device_type.clone(),
            room: new_state.room.clone(),
            old_value: old_state.resolved_value.clone(),
            new_value: new_state.resolved_value.clone(),
            change_type,
            timestamp: new_state.last_updated,
            significance,
        })
    }

    /// Determine the type of change
    fn determine_change_type(
        &self,
        old_state: &DeviceState,
        new_state: &DeviceState,
    ) -> ChangeType {
        match (&old_state.resolved_value, &new_state.resolved_value) {
            (None, Some(_)) => ChangeType::DeviceOnline,
            (Some(_), None) => ChangeType::DeviceOffline,
            (Some(old), Some(new)) => {
                if old.numeric_value != new.numeric_value {
                    ChangeType::ValueChanged
                } else if old_state.state_quality != new_state.state_quality {
                    ChangeType::QualityChanged
                } else {
                    ChangeType::ValueChanged
                }
            }
            (None, None) => {
                if old_state.state_quality != new_state.state_quality {
                    ChangeType::QualityChanged
                } else {
                    ChangeType::Error
                }
            }
        }
    }

    /// Determine the significance of a change
    fn determine_change_significance(
        &self,
        old_state: &DeviceState,
        new_state: &DeviceState,
    ) -> ChangeSignificance {
        // Security and safety devices are always critical
        if new_state.device_type.contains("security")
            || new_state.device_type.contains("alarm")
            || new_state.device_type.contains("smoke")
        {
            return ChangeSignificance::Critical;
        }

        // Door/window sensors are major
        if new_state.device_type.contains("door") || new_state.device_type.contains("window") {
            return ChangeSignificance::Major;
        }

        // Check numeric value changes
        if let (Some(old), Some(new)) = (&old_state.resolved_value, &new_state.resolved_value) {
            if let (Some(old_val), Some(new_val)) = (old.numeric_value, new.numeric_value) {
                let change_magnitude = (new_val - old_val).abs();

                // Temperature changes
                if old
                    .sensor_type
                    .as_ref()
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_default()
                    .contains("Temperature")
                {
                    if change_magnitude > 2.0 {
                        return ChangeSignificance::Major;
                    } else if change_magnitude > 0.5 {
                        return ChangeSignificance::Minor;
                    }
                }

                // Percentage-based sensors (humidity, dimmer, etc.)
                if old.unit.as_ref().map(|u| u.contains('%')).unwrap_or(false) {
                    if change_magnitude > 20.0 {
                        return ChangeSignificance::Major;
                    } else if change_magnitude > 5.0 {
                        return ChangeSignificance::Minor;
                    }
                }

                // Binary sensors (on/off, open/closed)
                if (old_val == 0.0 && new_val > 0.0) || (old_val > 0.0 && new_val == 0.0) {
                    return ChangeSignificance::Major;
                }
            }
        }

        ChangeSignificance::Trivial
    }

    /// Process a change event (notifications, history, etc.)
    async fn process_change_event(&self, event: StateChangeEvent) -> Result<()> {
        // Update history
        {
            let mut history = self.state_history.write().await;
            history.add_change_event(event.clone());
        }

        // Send notifications
        self.send_notifications(event).await?;

        Ok(())
    }

    /// Send notifications to subscribers
    async fn send_notifications(&self, event: StateChangeEvent) -> Result<()> {
        // Global notification
        if let Err(e) = self.subscription_manager.global_sender.send(event.clone()) {
            debug!("No global subscribers: {}", e);
        }

        // Device-specific notification
        let device_subs = self.subscription_manager.device_subscriptions.read().await;
        if let Some(sender) = device_subs.get(&event.device_uuid) {
            if let Err(e) = sender.send(event.clone()) {
                debug!("No device subscribers for {}: {}", event.device_uuid, e);
            }
        }

        // Room-specific notification
        if let Some(room) = &event.room {
            let room_subs = self.subscription_manager.room_subscriptions.read().await;
            if let Some(sender) = room_subs.get(room) {
                if let Err(e) = sender.send(event.clone()) {
                    debug!("No room subscribers for {}: {}", room, e);
                }
            }
        }

        // Type-specific notification
        let type_subs = self.subscription_manager.type_subscriptions.read().await;
        if let Some(sender) = type_subs.get(&event.device_type) {
            if let Err(e) = sender.send(event.clone()) {
                debug!("No type subscribers for {}: {}", event.device_type, e);
            }
        }

        Ok(())
    }

    /// Start periodic state polling task
    async fn start_state_polling(&self) -> tokio::task::JoinHandle<()> {
        let value_resolver = self.value_resolver.clone();
        let device_states = self.device_states.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Poll states for devices that haven't been updated recently
                let states = device_states.read().await;
                let stale_devices: Vec<String> = states
                    .iter()
                    .filter(|(_, state)| {
                        let age = Utc::now() - state.last_updated;
                        age > chrono::Duration::seconds(300) // 5 minutes
                    })
                    .map(|(uuid, _)| uuid.clone())
                    .collect();
                drop(states);

                if !stale_devices.is_empty() {
                    debug!("Refreshing {} stale device states", stale_devices.len());
                    if let Err(e) = value_resolver.resolve_batch_values(&stale_devices).await {
                        warn!("Failed to refresh stale states: {}", e);
                    }
                }
            }
        })
    }

    /// Start change analysis task
    async fn start_change_analysis(&self) -> tokio::task::JoinHandle<()> {
        let state_history = self.state_history.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                // Analyze patterns and update statistics
                let mut history = state_history.write().await;
                history.update_statistics();
            }
        })
    }

    /// Start cleanup task
    async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let state_history = self.state_history.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // 1 hour

            loop {
                interval.tick().await;

                // Clean up old history entries
                let mut history = state_history.write().await;
                history.cleanup_old_entries();
            }
        })
    }
}

impl ChangeDetector {
    pub fn new() -> Self {
        Self {
            change_thresholds: HashMap::new(),
            change_debounce: Duration::from_secs(5),
            last_notification: RwLock::new(HashMap::new()),
        }
    }

    pub async fn should_notify(&self, device_uuid: &str) -> bool {
        let mut last_times = self.last_notification.write().await;
        let now = Utc::now();

        if let Some(last_time) = last_times.get(device_uuid) {
            let elapsed = (now - *last_time)
                .to_std()
                .unwrap_or(Duration::from_secs(0));
            if elapsed < self.change_debounce {
                return false;
            }
        }

        last_times.insert(device_uuid.to_string(), now);
        true
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHistory {
    pub fn new() -> Self {
        Self {
            device_changes: HashMap::new(),
            max_history_length: 100,
            change_statistics: ChangeStatistics::default(),
        }
    }

    pub fn add_change_event(&mut self, event: StateChangeEvent) {
        let device_history = self
            .device_changes
            .entry(event.device_uuid.clone())
            .or_default();

        device_history.push_back(event.clone());

        if device_history.len() > self.max_history_length {
            device_history.pop_front();
        }

        self.change_statistics.total_changes += 1;

        let change_type_key = format!("{:?}", event.change_type);
        *self
            .change_statistics
            .changes_by_type
            .entry(change_type_key)
            .or_insert(0) += 1;

        *self
            .change_statistics
            .changes_by_device_type
            .entry(event.device_type)
            .or_insert(0) += 1;

        if let Some(room) = &event.room {
            *self
                .change_statistics
                .changes_by_room
                .entry(room.clone())
                .or_insert(0) += 1;
        }
    }

    pub fn update_statistics(&mut self) {
        // Calculate change rate and most active devices
        let total_devices = self.device_changes.len() as f64;
        if total_devices > 0.0 {
            self.change_statistics.change_rate_per_hour =
                self.change_statistics.total_changes as f64 / total_devices;
        }

        // Update most active devices
        let mut device_activity: Vec<(String, u64)> = self
            .device_changes
            .iter()
            .map(|(uuid, changes)| (uuid.clone(), changes.len() as u64))
            .collect();

        device_activity.sort_by(|a, b| b.1.cmp(&a.1));
        self.change_statistics.most_active_devices = device_activity.into_iter().take(10).collect();
    }

    pub fn cleanup_old_entries(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::hours(24);

        for device_history in self.device_changes.values_mut() {
            device_history.retain(|event| event.timestamp > cutoff);
        }

        self.device_changes.retain(|_, history| !history.is_empty());
    }
}

impl Default for StateHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ChangeStatistics {
    fn default() -> Self {
        Self {
            total_changes: 0,
            changes_by_type: HashMap::new(),
            changes_by_device_type: HashMap::new(),
            changes_by_room: HashMap::new(),
            most_active_devices: Vec::new(),
            change_rate_per_hour: 0.0,
        }
    }
}
