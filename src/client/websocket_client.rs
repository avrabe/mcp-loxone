//! WebSocket client implementation for real-time Loxone communication
//!
//! This module provides WebSocket-based real-time communication with Loxone
//! Miniservers for live state updates, sensor monitoring, and event streaming.
//!
//! Features:
//! - Real-time device state updates
//! - Event filtering and subscription management
//! - Automatic reconnection with exponential backoff
//! - Integration with HTTP clients for hybrid operation
//! - Efficient binary message parsing for sensor data

#[cfg(feature = "websocket")]
use crate::client::{ClientContext, LoxoneClient, LoxoneResponse, LoxoneStructure};
#[cfg(feature = "websocket")]
use crate::config::{credentials::LoxoneCredentials, AuthMethod, LoxoneConfig};
#[cfg(feature = "websocket")]
use crate::error::{LoxoneError, Result};
#[cfg(feature = "websocket")]
use crate::security::encryption::EncryptionManager;
#[cfg(feature = "websocket")]
use async_trait::async_trait;
#[cfg(feature = "websocket")]
use futures_util::SinkExt;
#[cfg(feature = "websocket")]
use rand;
#[cfg(feature = "websocket")]
use regex::Regex;
#[cfg(feature = "websocket")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "websocket")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "websocket")]
use std::io::{Cursor, Read};
#[cfg(feature = "websocket")]
use std::sync::Arc;
#[cfg(feature = "websocket")]
use std::time::Duration;
#[cfg(feature = "websocket")]
use tokio::sync::{mpsc, Mutex, RwLock};
#[cfg(feature = "websocket")]
use tokio::time::{sleep, Instant};
#[cfg(feature = "websocket")]
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
#[cfg(feature = "websocket")]
use tracing::{debug, error, info, warn};
#[cfg(feature = "websocket")]
use url::Url;

#[cfg(feature = "websocket")]
type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(feature = "websocket")]
type SubscriberList = Arc<RwLock<Vec<(mpsc::UnboundedSender<StateUpdate>, FilterType)>>>;

/// Filter type enumeration to support both basic and advanced filters
#[cfg(feature = "websocket")]
#[derive(Debug, Clone)]
pub enum FilterType {
    Basic(EventFilter),
    Advanced(AdvancedEventFilter),
}

/// WebSocket message types from Loxone
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoxoneWebSocketMessage {
    /// Message type (e.g., "text", "binary", "header")
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Message content
    pub data: serde_json::Value,

    /// Timestamp
    #[serde(default)]
    pub timestamp: Option<u64>,
}

/// Event types from Loxone WebSocket
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum LoxoneEventType {
    /// Device state change
    #[default]
    State,
    /// Weather update
    Weather,
    /// Text message
    Text,
    /// Alarm/Security event
    Alarm,
    /// System event
    System,
    /// Binary sensor data
    Sensor,
    /// Unknown event type
    Unknown(String),
}

impl From<String> for LoxoneEventType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "state" => LoxoneEventType::State,
            "weather" => LoxoneEventType::Weather,
            "text" => LoxoneEventType::Text,
            "alarm" => LoxoneEventType::Alarm,
            "system" => LoxoneEventType::System,
            "sensor" => LoxoneEventType::Sensor,
            _ => LoxoneEventType::Unknown(s),
        }
    }
}

impl std::fmt::Display for LoxoneEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoxoneEventType::State => write!(f, "state"),
            LoxoneEventType::Weather => write!(f, "weather"),
            LoxoneEventType::Text => write!(f, "text"),
            LoxoneEventType::Alarm => write!(f, "alarm"),
            LoxoneEventType::System => write!(f, "system"),
            LoxoneEventType::Sensor => write!(f, "sensor"),
            LoxoneEventType::Unknown(s) => write!(f, "unknown({s})"),
        }
    }
}

/// State update from WebSocket
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    /// Device UUID
    pub uuid: String,

    /// State name
    pub state: String,

    /// New value
    pub value: serde_json::Value,

    /// Previous value (if available)
    pub previous_value: Option<serde_json::Value>,

    /// Event type
    pub event_type: LoxoneEventType,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Room name (if available)
    pub room: Option<String>,

    /// Device name (if available)
    pub device_name: Option<String>,
}

/// Event subscription filter
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    /// Device UUIDs to monitor (empty = all devices)
    pub device_uuids: HashSet<String>,

    /// Event types to monitor (empty = all types)
    pub event_types: HashSet<LoxoneEventType>,

    /// Room names to monitor (empty = all rooms)
    pub rooms: HashSet<String>,

    /// State names to monitor (empty = all states)
    pub states: HashSet<String>,

    /// Minimum interval between events for same device (debouncing)
    pub min_interval: Option<Duration>,
}

/// Advanced event filter with regex pattern support
/// Note: This struct cannot be serialized due to Regex fields
#[cfg(feature = "websocket")]
#[derive(Debug, Clone)]
pub struct AdvancedEventFilter {
    /// Basic filter (for backward compatibility)
    pub basic_filter: EventFilter,

    /// Regex pattern for device names (optional, more flexible than UUID matching)
    pub device_name_pattern: Option<Regex>,

    /// Regex pattern for room names (optional, more flexible than exact matching)
    pub room_name_pattern: Option<Regex>,

    /// Regex pattern for state names (optional, more flexible than exact matching)
    pub state_name_pattern: Option<Regex>,

    /// Regex pattern for device values (optional, filter by value content)
    pub value_pattern: Option<Regex>,

    /// Enable case-insensitive matching for all regex patterns
    pub case_insensitive: bool,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            device_uuids: HashSet::new(),
            event_types: HashSet::new(),
            rooms: HashSet::new(),
            states: HashSet::new(),
            min_interval: Some(Duration::from_millis(100)), // 100ms debounce by default
        }
    }
}

impl EventFilter {
    /// Create a filter that matches all events (no filtering)
    pub fn match_all() -> Self {
        Self {
            device_uuids: HashSet::new(),
            event_types: HashSet::new(),
            rooms: HashSet::new(),
            states: HashSet::new(),
            min_interval: None, // No debouncing
        }
    }

    /// Create a filter for specific device UUIDs only
    pub fn for_devices(device_uuids: Vec<String>) -> Self {
        Self {
            device_uuids: device_uuids.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Create a filter for specific rooms only
    pub fn for_rooms(rooms: Vec<String>) -> Self {
        Self {
            rooms: rooms.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Create a filter for specific event types only
    pub fn for_event_types(event_types: Vec<LoxoneEventType>) -> Self {
        Self {
            event_types: event_types.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Set minimum interval for debouncing
    pub fn with_debounce(mut self, interval: Duration) -> Self {
        self.min_interval = Some(interval);
        self
    }

    /// Disable debouncing
    pub fn without_debounce(mut self) -> Self {
        self.min_interval = None;
        self
    }
}

impl Default for AdvancedEventFilter {
    fn default() -> Self {
        Self {
            basic_filter: EventFilter::default(),
            device_name_pattern: None,
            room_name_pattern: None,
            state_name_pattern: None,
            value_pattern: None,
            case_insensitive: false,
        }
    }
}

impl AdvancedEventFilter {
    /// Create a new advanced filter from a basic filter
    pub fn from_basic(basic_filter: EventFilter) -> Self {
        Self {
            basic_filter,
            device_name_pattern: None,
            room_name_pattern: None,
            state_name_pattern: None,
            value_pattern: None,
            case_insensitive: false,
        }
    }

    /// Create an advanced filter that matches all events
    pub fn match_all() -> Self {
        Self {
            basic_filter: EventFilter::match_all(),
            device_name_pattern: None,
            room_name_pattern: None,
            state_name_pattern: None,
            value_pattern: None,
            case_insensitive: false,
        }
    }

    /// Add regex pattern for device names
    pub fn with_device_name_pattern(mut self, pattern: &str) -> Result<Self> {
        let flags = if self.case_insensitive { "(?i)" } else { "" };
        let full_pattern = format!("{}{}", flags, pattern);
        self.device_name_pattern = Some(Regex::new(&full_pattern).map_err(|e| {
            LoxoneError::config(format!("Invalid device name regex pattern: {}", e))
        })?);
        Ok(self)
    }

    /// Add regex pattern for room names
    pub fn with_room_name_pattern(mut self, pattern: &str) -> Result<Self> {
        let flags = if self.case_insensitive { "(?i)" } else { "" };
        let full_pattern = format!("{}{}", flags, pattern);
        self.room_name_pattern =
            Some(Regex::new(&full_pattern).map_err(|e| {
                LoxoneError::config(format!("Invalid room name regex pattern: {}", e))
            })?);
        Ok(self)
    }

    /// Add regex pattern for state names
    pub fn with_state_name_pattern(mut self, pattern: &str) -> Result<Self> {
        let flags = if self.case_insensitive { "(?i)" } else { "" };
        let full_pattern = format!("{}{}", flags, pattern);
        self.state_name_pattern = Some(Regex::new(&full_pattern).map_err(|e| {
            LoxoneError::config(format!("Invalid state name regex pattern: {}", e))
        })?);
        Ok(self)
    }

    /// Add regex pattern for device values (matches string representation of value)
    pub fn with_value_pattern(mut self, pattern: &str) -> Result<Self> {
        let flags = if self.case_insensitive { "(?i)" } else { "" };
        let full_pattern = format!("{}{}", flags, pattern);
        self.value_pattern = Some(
            Regex::new(&full_pattern)
                .map_err(|e| LoxoneError::config(format!("Invalid value regex pattern: {}", e)))?,
        );
        Ok(self)
    }

    /// Enable case-insensitive matching for all patterns
    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }

    /// Set minimum interval for debouncing
    pub fn with_debounce(mut self, interval: Duration) -> Self {
        self.basic_filter.min_interval = Some(interval);
        self
    }

    /// Disable debouncing
    pub fn without_debounce(mut self) -> Self {
        self.basic_filter.min_interval = None;
        self
    }

    /// Add specific device UUIDs to monitor
    pub fn with_device_uuids(mut self, uuids: Vec<String>) -> Self {
        self.basic_filter.device_uuids.extend(uuids);
        self
    }

    /// Add specific event types to monitor
    pub fn with_event_types(mut self, event_types: Vec<LoxoneEventType>) -> Self {
        self.basic_filter.event_types.extend(event_types);
        self
    }

    /// Add specific rooms to monitor
    pub fn with_rooms(mut self, rooms: Vec<String>) -> Self {
        self.basic_filter.rooms.extend(rooms);
        self
    }

    /// Add specific states to monitor
    pub fn with_states(mut self, states: Vec<String>) -> Self {
        self.basic_filter.states.extend(states);
        self
    }

    /// Convenience method: Filter devices by name pattern (case-insensitive)
    pub fn with_device_names_matching(pattern: &str) -> Result<Self> {
        AdvancedEventFilter::match_all()
            .case_insensitive()
            .with_device_name_pattern(pattern)
    }

    /// Convenience method: Filter rooms by name pattern (case-insensitive)
    pub fn with_room_names_matching(pattern: &str) -> Result<Self> {
        AdvancedEventFilter::match_all()
            .case_insensitive()
            .with_room_name_pattern(pattern)
    }

    /// Convenience method: Filter by value pattern (case-insensitive)
    pub fn with_values_matching(pattern: &str) -> Result<Self> {
        AdvancedEventFilter::match_all()
            .case_insensitive()
            .with_value_pattern(pattern)
    }

    /// Convenience method: Create a complex filter with multiple patterns
    pub fn create_complex_filter(
        device_name_pattern: Option<&str>,
        room_name_pattern: Option<&str>,
        state_name_pattern: Option<&str>,
        value_pattern: Option<&str>,
        case_insensitive: bool,
    ) -> Result<Self> {
        let mut filter = if case_insensitive {
            AdvancedEventFilter::match_all().case_insensitive()
        } else {
            AdvancedEventFilter::match_all()
        };

        if let Some(pattern) = device_name_pattern {
            filter = filter.with_device_name_pattern(pattern)?;
        }
        if let Some(pattern) = room_name_pattern {
            filter = filter.with_room_name_pattern(pattern)?;
        }
        if let Some(pattern) = state_name_pattern {
            filter = filter.with_state_name_pattern(pattern)?;
        }
        if let Some(pattern) = value_pattern {
            filter = filter.with_value_pattern(pattern)?;
        }

        Ok(filter)
    }
}

/// Reconnection configuration
#[cfg(feature = "websocket")]
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    /// Enable automatic reconnection
    pub enabled: bool,

    /// Initial delay before first reconnection attempt
    pub initial_delay: Duration,

    /// Maximum delay between reconnection attempts
    pub max_delay: Duration,

    /// Backoff multiplier (exponential backoff)
    pub backoff_multiplier: f64,

    /// Maximum number of reconnection attempts (None = infinite)
    pub max_attempts: Option<u32>,

    /// Jitter factor to prevent thundering herd
    pub jitter_factor: f64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            max_attempts: None, // Infinite retries
            jitter_factor: 0.1, // 10% jitter
        }
    }
}

/// WebSocket connection statistics
#[cfg(feature = "websocket")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSocketStats {
    /// Total messages received
    pub messages_received: u64,

    /// Total state updates processed
    pub state_updates: u64,

    /// Total reconnection attempts
    pub reconnection_attempts: u32,

    /// Current connection uptime
    pub connection_start: Option<chrono::DateTime<chrono::Utc>>,

    /// Last message received timestamp
    pub last_message: Option<chrono::DateTime<chrono::Utc>>,

    /// Bytes received
    pub bytes_received: u64,

    /// Events filtered out (due to subscription filters)
    pub events_filtered: u64,

    /// Events debounced (due to min_interval)
    pub events_debounced: u64,
}

/// WebSocket client for real-time Loxone communication
#[cfg(feature = "websocket")]
pub struct LoxoneWebSocketClient {
    /// Base URL for Miniserver
    base_url: Url,

    /// Authentication credentials
    credentials: LoxoneCredentials,

    /// Configuration
    config: LoxoneConfig,

    /// Shared context for caching
    context: ClientContext,

    /// WebSocket stream
    ws_stream: Option<Arc<Mutex<WsStream>>>,

    /// State update channel sender
    state_sender: Option<mpsc::UnboundedSender<StateUpdate>>,

    /// Event subscribers with filters
    subscribers: SubscriberList,

    /// Connection state
    connected: Arc<RwLock<bool>>,

    /// Background task handles
    task_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,

    /// Reconnection configuration
    reconnection_config: ReconnectionConfig,

    /// Last event timestamps for debouncing
    #[allow(dead_code)]
    last_event_times: Arc<RwLock<HashMap<String, Instant>>>,

    /// HTTP client for hybrid operation (structure fetching, commands)
    http_client: Option<Arc<dyn LoxoneClient>>,

    /// Statistics
    stats: Arc<RwLock<WebSocketStats>>,

    /// Encryption manager for secure communication
    encryption_manager: Arc<RwLock<EncryptionManager>>,

    /// Current encryption session (if enabled)
    encryption_session: Arc<RwLock<Option<String>>>, // session_id

    /// WebSocket resilience manager
    resilience_manager:
        Option<Arc<crate::client::websocket_resilience::WebSocketResilienceManager>>,

    /// Weather data storage
    weather_storage: Option<Arc<crate::storage::WeatherStorage>>,
}

#[cfg(feature = "websocket")]
impl LoxoneWebSocketClient {
    /// Create a new WebSocket client
    pub async fn new(config: LoxoneConfig, credentials: LoxoneCredentials) -> Result<Self> {
        Ok(Self {
            base_url: config.url.clone(),
            credentials,
            config,
            context: ClientContext::new(),
            ws_stream: None,
            state_sender: None,
            subscribers: Arc::new(RwLock::new(Vec::new())),
            connected: Arc::new(RwLock::new(false)),
            task_handles: Arc::new(Mutex::new(Vec::new())),
            reconnection_config: ReconnectionConfig::default(),
            last_event_times: Arc::new(RwLock::new(HashMap::new())),
            http_client: None,
            stats: Arc::new(RwLock::new(WebSocketStats::default())),
            encryption_manager: Arc::new(RwLock::new(EncryptionManager::new(10))), // Max 10 sessions
            encryption_session: Arc::new(RwLock::new(None)),
            resilience_manager: None,
            weather_storage: None,
        })
    }

    /// Create WebSocket client with HTTP client for hybrid operation
    pub async fn new_with_http_client(
        config: LoxoneConfig,
        credentials: LoxoneCredentials,
        http_client: Arc<dyn LoxoneClient>,
    ) -> Result<Self> {
        let mut client = Self::new(config, credentials).await?;
        client.http_client = Some(http_client);
        Ok(client)
    }

    /// Configure reconnection behavior
    pub fn set_reconnection_config(&mut self, config: ReconnectionConfig) {
        self.reconnection_config = config;
    }

    /// Enable resilience features with message acknowledgment
    pub async fn enable_resilience(
        &mut self,
        resilience_config: crate::client::websocket_resilience::WebSocketResilienceConfig,
    ) -> Result<()> {
        let ws_url = self.build_ws_url().await?;
        let manager = Arc::new(
            crate::client::websocket_resilience::WebSocketResilienceManager::new(
                ws_url.to_string(),
                resilience_config,
            ),
        );

        manager.start().await?;
        self.resilience_manager = Some(manager);

        info!("WebSocket resilience enabled");
        Ok(())
    }

    /// Disable resilience features
    pub async fn disable_resilience(&mut self) {
        if let Some(manager) = self.resilience_manager.take() {
            manager.stop().await;
            info!("WebSocket resilience disabled");
        }
    }

    /// Enable weather data storage
    pub async fn enable_weather_storage(
        &mut self,
        storage_config: crate::storage::WeatherStorageConfig,
    ) -> Result<()> {
        use crate::storage::WeatherStorage;

        let storage = Arc::new(WeatherStorage::new(storage_config).await?);

        // Update storage with current device structure if available
        if let Ok(devices) = self.context.devices.try_read() {
            storage.update_device_structure(&devices).await;
        }

        self.weather_storage = Some(storage);
        info!("Weather data storage enabled");
        Ok(())
    }

    /// Disable weather data storage
    pub async fn disable_weather_storage(&mut self) {
        if let Some(_) = self.weather_storage.take() {
            info!("Weather data storage disabled");
        }
    }

    /// Check if weather storage is enabled
    pub fn is_weather_storage_enabled(&self) -> bool {
        self.weather_storage.is_some()
    }

    /// Check if resilience is enabled
    pub fn is_resilience_enabled(&self) -> bool {
        self.resilience_manager.is_some()
    }

    /// Get resilience statistics (if enabled)
    pub async fn get_resilience_stats(
        &self,
    ) -> Option<crate::client::websocket_resilience::ResilienceStatistics> {
        if let Some(manager) = &self.resilience_manager {
            Some(manager.get_statistics().await)
        } else {
            None
        }
    }

    /// Subscribe to resilience events (if enabled)
    pub fn subscribe_to_resilience_events(
        &self,
    ) -> Option<
        tokio::sync::broadcast::Receiver<crate::client::websocket_resilience::ResilienceEvent>,
    > {
        self.resilience_manager
            .as_ref()
            .map(|m| m.subscribe_to_events())
    }

    /// Send a resilient message with acknowledgment (if resilience is enabled)
    pub async fn send_resilient_message(
        &self,
        payload: String,
        message_type: crate::client::websocket_resilience::MessageType,
        priority: crate::client::websocket_resilience::MessagePriority,
    ) -> Result<String> {
        if let Some(manager) = &self.resilience_manager {
            manager.send_message(payload, message_type, priority).await
        } else {
            Err(LoxoneError::config("Resilience not enabled"))
        }
    }

    /// Build WebSocket URL with authentication
    async fn build_ws_url(&self) -> Result<Url> {
        let mut ws_url = self.base_url.clone();

        // Convert HTTP(S) to WS(S)
        match ws_url.scheme() {
            "http" => ws_url
                .set_scheme("ws")
                .map_err(|_| LoxoneError::connection("Failed to convert HTTP to WebSocket URL"))?,
            "https" => ws_url
                .set_scheme("wss")
                .map_err(|_| LoxoneError::connection("Failed to convert HTTPS to WebSocket URL"))?,
            _ => {
                return Err(LoxoneError::connection(
                    "Unsupported URL scheme for WebSocket",
                ))
            }
        }

        // Add WebSocket endpoint path
        ws_url.set_path("/ws/rfc6455");

        // Add authentication based on method
        match self.config.auth_method {
            AuthMethod::Basic => {
                // Basic auth via query parameters (legacy)
                ws_url
                    .query_pairs_mut()
                    .append_pair("user", &self.credentials.username)
                    .append_pair("password", &self.credentials.password);
            }
            AuthMethod::Token => {
                // Use proper token authentication from HTTP client
                match self.get_token_auth_params().await {
                    Ok(Some(auth_params)) => {
                        // Use token-based authentication
                        ws_url.set_query(Some(&auth_params));
                        debug!("Using token authentication for WebSocket connection");
                    }
                    Ok(None) => {
                        // No token available, fall back to basic auth
                        warn!("No token available from HTTP client, falling back to basic auth for WebSocket");
                        ws_url
                            .query_pairs_mut()
                            .append_pair("user", &self.credentials.username)
                            .append_pair("password", &self.credentials.password);
                    }
                    Err(e) => {
                        // Token extraction failed, fall back to basic auth
                        warn!("Failed to get token authentication parameters: {}, falling back to basic auth", e);
                        ws_url
                            .query_pairs_mut()
                            .append_pair("user", &self.credentials.username)
                            .append_pair("password", &self.credentials.password);
                    }
                }
            }
            #[cfg(feature = "websocket")]
            AuthMethod::WebSocket => {
                // WebSocket native authentication - use basic auth for simplicity
                ws_url
                    .query_pairs_mut()
                    .append_pair("user", &self.credentials.username)
                    .append_pair("password", &self.credentials.password);
            }
        }

        Ok(ws_url)
    }

    /// Get token authentication parameters from the HTTP client
    async fn get_token_auth_params(&self) -> Result<Option<String>> {
        if let Some(http_client) = &self.http_client {
            // Try to downcast to TokenHttpClient to access token authentication
            #[cfg(feature = "crypto-openssl")]
            if let Some(token_client) = http_client
                .as_ref()
                .as_any()
                .downcast_ref::<crate::client::TokenHttpClient>()
            {
                // Get auth parameters (this method ensures authentication internally)
                match token_client.get_auth_params().await {
                    Ok(auth_params) => {
                        debug!("Successfully extracted token auth parameters from HTTP client");
                        return Ok(Some(auth_params));
                    }
                    Err(e) => {
                        warn!("Failed to get auth parameters from TokenHttpClient: {}", e);
                        return Ok(None);
                    }
                }
            }

            // If not a TokenHttpClient or crypto-openssl feature not enabled, no token auth available
            debug!("HTTP client is not a TokenHttpClient, token authentication not available");
        }

        // No HTTP client or no token authentication available
        Ok(None)
    }

    /// Start background tasks for message processing and reconnection
    async fn start_background_tasks(&mut self) -> Result<()> {
        let (state_tx, mut state_rx) = mpsc::unbounded_channel::<StateUpdate>();
        self.state_sender = Some(state_tx);

        let context = self.context.clone();
        let subscribers = self.subscribers.clone();
        let stats = self.stats.clone();
        let last_event_times = self.last_event_times.clone();

        // Task 1: Process state updates and distribute to subscribers
        let state_task = tokio::spawn(async move {
            while let Some(update) = state_rx.recv().await {
                debug!(
                    "Processing state update: {} = {:?}",
                    update.uuid, update.value
                );

                // Update device state in context
                {
                    let mut devices = context.devices.write().await;
                    if let Some(device) = devices.get_mut(&update.uuid) {
                        let previous_value = device.states.get(&update.state).cloned();
                        device
                            .states
                            .insert(update.state.clone(), update.value.clone());

                        // Log state change if sensor logger is available
                        if let Some(logger) = context.get_sensor_logger().await {
                            logger
                                .log_state_change(
                                    update.uuid.clone(),
                                    previous_value.unwrap_or_default(),
                                    update.value.clone(),
                                    Some(device.name.clone()),
                                    Some(Self::map_device_type_to_sensor_type(&device.device_type)),
                                    device.room.clone(),
                                )
                                .await;
                        }
                    }
                }

                // Update statistics
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.state_updates += 1;
                    stats_guard.last_message = Some(chrono::Utc::now());
                }

                // Distribute to subscribers with debouncing and filtering
                let subscribers_guard = subscribers.read().await;
                let mut filtered_count = 0;
                let mut debounced_count = 0;

                for (sender, filter) in subscribers_guard.iter() {
                    // Apply filter first
                    if !Self::matches_filter(&update, filter).await {
                        filtered_count += 1;
                        continue;
                    }

                    // Apply debouncing if configured
                    let min_interval = match filter {
                        FilterType::Basic(basic_filter) => basic_filter.min_interval,
                        FilterType::Advanced(advanced_filter) => {
                            advanced_filter.basic_filter.min_interval
                        }
                    };

                    if let Some(min_interval) = min_interval {
                        let mut last_times = last_event_times.write().await;
                        let key = format!("{}:{}", update.uuid, update.state);
                        let now = Instant::now();

                        if let Some(last_time) = last_times.get(&key) {
                            if now.duration_since(*last_time) < min_interval {
                                debounced_count += 1;
                                continue;
                            }
                        }

                        last_times.insert(key, now);
                    }

                    // Send to subscriber
                    let _ = sender.send(update.clone());
                }

                // Update filtering/debouncing statistics
                if filtered_count > 0 || debounced_count > 0 {
                    let mut stats_guard = stats.write().await;
                    stats_guard.events_filtered += filtered_count;
                    stats_guard.events_debounced += debounced_count;
                }
            }
        });

        // Task 2: WebSocket message processor
        let ws_stream = self.ws_stream.clone();
        let state_sender_clone = self.state_sender.clone();
        let stats_clone = self.stats.clone();
        let connected_clone = self.connected.clone();

        #[allow(clippy::manual_map)]
        let message_task = if let Some(ws_stream) = ws_stream {
            Some(tokio::spawn(async move {
                loop {
                    let message = {
                        use futures_util::StreamExt;
                        let mut stream = ws_stream.lock().await;
                        stream.next().await
                    };

                    match message {
                        Some(Ok(msg)) => {
                            // Update message statistics
                            {
                                let mut stats_guard = stats_clone.write().await;
                                stats_guard.messages_received += 1;
                                stats_guard.last_message = Some(chrono::Utc::now());

                                if let tokio_tungstenite::tungstenite::Message::Binary(ref data) =
                                    msg
                                {
                                    stats_guard.bytes_received += data.len() as u64;
                                } else if let tokio_tungstenite::tungstenite::Message::Text(
                                    ref text,
                                ) = msg
                                {
                                    stats_guard.bytes_received += text.len() as u64;
                                }
                            }

                            // Process the message
                            if let Err(e) = Self::process_ws_message(msg, &state_sender_clone).await
                            {
                                warn!("Error processing WebSocket message: {}", e);
                            }
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            *connected_clone.write().await = false;
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            *connected_clone.write().await = false;
                            break;
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Task 3: Reconnection and token refresh manager (if enabled)
        let reconnection_task = if self.reconnection_config.enabled {
            let connected = self.connected.clone();
            let base_url = self.base_url.clone();
            let credentials = self.credentials.clone();
            let config = self.config.clone();
            let reconnection_config = self.reconnection_config.clone();
            let stats_clone = self.stats.clone();
            let ws_stream_ref = self.ws_stream.clone();
            let http_client = self.http_client.clone();

            Some(tokio::spawn(async move {
                let mut attempt = 0;
                let mut delay = reconnection_config.initial_delay;

                loop {
                    // Check if we're still connected
                    if *connected.read().await {
                        // Check if we need to refresh tokens proactively
                        #[cfg(feature = "crypto-openssl")]
                        if let Some(http_client) = &http_client {
                            if let Some(token_client) = http_client
                                .as_ref()
                                .as_any()
                                .downcast_ref::<crate::client::TokenHttpClient>(
                            ) {
                                // Try to get auth params - this will internally handle token expiration and refresh
                                match token_client.get_auth_params().await {
                                    Ok(_) => {
                                        debug!("WebSocket token authentication verified and refreshed if needed");
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to refresh authentication for WebSocket: {}",
                                            e
                                        );
                                        // Token authentication failed, force WebSocket reconnection with new token
                                        *connected.write().await = false;
                                        continue;
                                    }
                                }
                            }
                        }

                        // Wait before checking again (30 seconds for token checks)
                        sleep(Duration::from_secs(30)).await;
                        continue;
                    }

                    // Check if we've exceeded max attempts
                    if let Some(max_attempts) = reconnection_config.max_attempts {
                        if attempt >= max_attempts {
                            error!("Max reconnection attempts ({}) exceeded", max_attempts);
                            break;
                        }
                    }

                    attempt += 1;
                    {
                        let mut stats_guard = stats_clone.write().await;
                        stats_guard.reconnection_attempts = attempt;
                    }

                    info!("Attempting WebSocket reconnection #{}", attempt);

                    // Add jitter to prevent thundering herd
                    let jitter =
                        (delay.as_millis() as f64 * reconnection_config.jitter_factor) as u64;
                    let random_jitter = if jitter > 0 {
                        rand::random::<u64>() % jitter
                    } else {
                        0
                    };
                    let jittered_delay = delay + Duration::from_millis(random_jitter);

                    sleep(jittered_delay).await;

                    // Attempt reconnection - try with token first if available
                    let reconnection_result = if let Some(http_client) = &http_client {
                        #[cfg(feature = "crypto-openssl")]
                        if let Some(token_client) = http_client
                            .as_ref()
                            .as_any()
                            .downcast_ref::<crate::client::TokenHttpClient>(
                        ) {
                            // Try reconnection with token authentication
                            match Self::attempt_token_reconnection(&base_url, token_client, &config)
                                .await
                            {
                                Ok(stream) => {
                                    info!("WebSocket reconnection successful with token authentication");
                                    Ok(stream)
                                }
                                Err(e) => {
                                    warn!("Token-based reconnection failed: {}, falling back to basic auth", e);
                                    Self::attempt_reconnection(&base_url, &credentials, &config)
                                        .await
                                }
                            }
                        } else {
                            Self::attempt_reconnection(&base_url, &credentials, &config).await
                        }

                        #[cfg(not(feature = "crypto-openssl"))]
                        Self::attempt_reconnection(&base_url, &credentials, &config).await
                    } else {
                        Self::attempt_reconnection(&base_url, &credentials, &config).await
                    };

                    match reconnection_result {
                        Ok(new_stream) => {
                            info!("✅ WebSocket reconnection successful");

                            // Replace the WebSocket stream
                            if let Some(ws_stream_arc) = &ws_stream_ref {
                                *ws_stream_arc.lock().await = new_stream;
                            }

                            *connected.write().await = true;
                            attempt = 0; // Reset attempt counter
                            delay = reconnection_config.initial_delay; // Reset delay
                        }
                        Err(e) => {
                            warn!("Reconnection attempt #{} failed: {}", attempt, e);

                            // Exponential backoff
                            delay = Duration::from_millis(
                                (delay.as_millis() as f64 * reconnection_config.backoff_multiplier)
                                    as u64,
                            )
                            .min(reconnection_config.max_delay);
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Store task handles
        let mut handles = self.task_handles.lock().await;
        handles.push(state_task);
        if let Some(message_task) = message_task {
            handles.push(message_task);
        }
        if let Some(reconnection_task) = reconnection_task {
            handles.push(reconnection_task);
        }

        Ok(())
    }

    /// Check if a state update matches the given filter (basic EventFilter)
    async fn matches_basic_filter(update: &StateUpdate, filter: &EventFilter) -> bool {
        // Check device UUID filter
        if !filter.device_uuids.is_empty() && !filter.device_uuids.contains(&update.uuid) {
            return false;
        }

        // Check event type filter
        if !filter.event_types.is_empty() && !filter.event_types.contains(&update.event_type) {
            return false;
        }

        // Check room filter
        if !filter.rooms.is_empty() {
            if let Some(room) = &update.room {
                if !filter.rooms.contains(room) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check state name filter
        if !filter.states.is_empty() && !filter.states.contains(&update.state) {
            return false;
        }

        true
    }

    /// Check if a state update matches the given advanced filter
    async fn matches_advanced_filter(update: &StateUpdate, filter: &AdvancedEventFilter) -> bool {
        // First check basic filter criteria
        if !Self::matches_basic_filter(update, &filter.basic_filter).await {
            return false;
        }

        // Check device name pattern
        if let Some(pattern) = &filter.device_name_pattern {
            if let Some(device_name) = &update.device_name {
                if !pattern.is_match(device_name) {
                    return false;
                }
            } else {
                // No device name available but pattern is required
                return false;
            }
        }

        // Check room name pattern
        if let Some(pattern) = &filter.room_name_pattern {
            if let Some(room) = &update.room {
                if !pattern.is_match(room) {
                    return false;
                }
            } else {
                // No room available but pattern is required
                return false;
            }
        }

        // Check state name pattern
        if let Some(pattern) = &filter.state_name_pattern {
            if !pattern.is_match(&update.state) {
                return false;
            }
        }

        // Check value pattern (convert value to string for matching)
        if let Some(pattern) = &filter.value_pattern {
            let value_str = match &update.value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "null".to_string(),
                _ => serde_json::to_string(&update.value).unwrap_or_default(),
            };

            if !pattern.is_match(&value_str) {
                return false;
            }
        }

        true
    }

    /// Check if a state update matches the given filter (supports both basic and advanced)
    async fn matches_filter(update: &StateUpdate, filter: &FilterType) -> bool {
        match filter {
            FilterType::Basic(basic_filter) => {
                Self::matches_basic_filter(update, basic_filter).await
            }
            FilterType::Advanced(advanced_filter) => {
                Self::matches_advanced_filter(update, advanced_filter).await
            }
        }
    }

    /// Subscribe to filtered state updates (basic filter)
    pub async fn subscribe_with_filter(
        &self,
        filter: EventFilter,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.write().await;
        subscribers.push((tx, FilterType::Basic(filter)));
        rx
    }

    /// Subscribe to filtered state updates (advanced filter with regex support)
    pub async fn subscribe_with_advanced_filter(
        &self,
        filter: AdvancedEventFilter,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.write().await;
        subscribers.push((tx, FilterType::Advanced(filter)));
        rx
    }

    /// Subscribe to all state updates
    pub async fn subscribe(&self) -> mpsc::UnboundedReceiver<StateUpdate> {
        self.subscribe_with_filter(EventFilter::default()).await
    }

    /// Subscribe to specific device UUIDs
    pub async fn subscribe_to_devices(
        &self,
        device_uuids: HashSet<String>,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let filter = EventFilter {
            device_uuids,
            ..Default::default()
        };
        self.subscribe_with_filter(filter).await
    }

    /// Subscribe to specific rooms
    pub async fn subscribe_to_rooms(
        &self,
        rooms: HashSet<String>,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let filter = EventFilter {
            rooms,
            ..Default::default()
        };
        self.subscribe_with_filter(filter).await
    }

    /// Subscribe to specific event types
    pub async fn subscribe_to_event_types(
        &self,
        event_types: HashSet<LoxoneEventType>,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let filter = EventFilter {
            event_types,
            ..Default::default()
        };
        self.subscribe_with_filter(filter).await
    }

    /// Subscribe to devices matching a name pattern (regex, case-insensitive)
    /// Example: subscribe_to_devices_matching(".*light.*") - all devices with "light" in name
    pub async fn subscribe_to_devices_matching(
        &self,
        pattern: &str,
    ) -> Result<mpsc::UnboundedReceiver<StateUpdate>> {
        let filter = AdvancedEventFilter::with_device_names_matching(pattern)?;
        Ok(self.subscribe_with_advanced_filter(filter).await)
    }

    /// Subscribe to rooms matching a name pattern (regex, case-insensitive)
    /// Example: subscribe_to_rooms_matching("(kitchen|bathroom)") - kitchen or bathroom
    pub async fn subscribe_to_rooms_matching(
        &self,
        pattern: &str,
    ) -> Result<mpsc::UnboundedReceiver<StateUpdate>> {
        let filter = AdvancedEventFilter::with_room_names_matching(pattern)?;
        Ok(self.subscribe_with_advanced_filter(filter).await)
    }

    /// Subscribe to values matching a pattern (regex, case-insensitive)
    /// Example: subscribe_to_values_matching(r"\d+\.?\d*") - numeric values
    pub async fn subscribe_to_values_matching(
        &self,
        pattern: &str,
    ) -> Result<mpsc::UnboundedReceiver<StateUpdate>> {
        let filter = AdvancedEventFilter::with_values_matching(pattern)?;
        Ok(self.subscribe_with_advanced_filter(filter).await)
    }

    /// Subscribe with complex regex filtering
    /// Example: All temperature sensors in bedrooms with values above 20
    /// subscribe_with_regex_filter(
    ///     Some(r".*temp.*"),     // device name contains "temp"
    ///     Some(r".*bedroom.*"),  // room contains "bedroom"
    ///     None,                  // any state name
    ///     Some(r"^[2-9]\d+"),    // value starts with 2-9 (>= 20)
    ///     true                   // case insensitive
    /// )
    pub async fn subscribe_with_regex_filter(
        &self,
        device_name_pattern: Option<&str>,
        room_name_pattern: Option<&str>,
        state_name_pattern: Option<&str>,
        value_pattern: Option<&str>,
        case_insensitive: bool,
    ) -> Result<mpsc::UnboundedReceiver<StateUpdate>> {
        let filter = AdvancedEventFilter::create_complex_filter(
            device_name_pattern,
            room_name_pattern,
            state_name_pattern,
            value_pattern,
            case_insensitive,
        )?;
        Ok(self.subscribe_with_advanced_filter(filter).await)
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> WebSocketStats {
        self.stats.read().await.clone()
    }

    /// Clear all subscribers
    pub async fn clear_subscribers(&self) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.clear();
    }

    /// Get active subscriber count
    pub async fn get_subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }

    /// Remove subscribers that match a specific basic filter
    pub async fn remove_subscribers_with_filter(&self, filter: &EventFilter) -> usize {
        let mut subscribers = self.subscribers.write().await;
        let initial_count = subscribers.len();

        subscribers.retain(|(_, subscriber_filter)| {
            // Keep subscribers that don't match the filter exactly
            match subscriber_filter {
                FilterType::Basic(basic_filter) => {
                    basic_filter.device_uuids != filter.device_uuids
                        || basic_filter.event_types != filter.event_types
                        || basic_filter.rooms != filter.rooms
                        || basic_filter.states != filter.states
                }
                FilterType::Advanced(_) => true, // Don't remove advanced filters
            }
        });

        initial_count - subscribers.len()
    }

    /// Get all unique device UUIDs being monitored
    pub async fn get_monitored_devices(&self) -> HashSet<String> {
        let mut monitored = HashSet::new();
        let subscribers = self.subscribers.read().await;

        for (_, filter) in subscribers.iter() {
            let device_uuids = match filter {
                FilterType::Basic(basic_filter) => &basic_filter.device_uuids,
                FilterType::Advanced(advanced_filter) => &advanced_filter.basic_filter.device_uuids,
            };
            monitored.extend(device_uuids.iter().cloned());
        }

        monitored
    }

    /// Get all unique rooms being monitored
    pub async fn get_monitored_rooms(&self) -> HashSet<String> {
        let mut monitored = HashSet::new();
        let subscribers = self.subscribers.read().await;

        for (_, filter) in subscribers.iter() {
            let rooms = match filter {
                FilterType::Basic(basic_filter) => &basic_filter.rooms,
                FilterType::Advanced(advanced_filter) => &advanced_filter.basic_filter.rooms,
            };
            monitored.extend(rooms.iter().cloned());
        }

        monitored
    }

    /// Helper method for reconnection attempts
    async fn attempt_reconnection(
        base_url: &Url,
        credentials: &LoxoneCredentials,
        _config: &LoxoneConfig,
    ) -> Result<WsStream> {
        use tokio_tungstenite::connect_async;

        // Build WebSocket URL
        let mut ws_url = base_url.clone();

        // Convert HTTP(S) to WS(S)
        match ws_url.scheme() {
            "http" => {
                ws_url.set_scheme("ws").map_err(|_| {
                    LoxoneError::connection("Failed to convert HTTP to WebSocket URL")
                })?;
            }
            "https" => {
                ws_url.set_scheme("wss").map_err(|_| {
                    LoxoneError::connection("Failed to convert HTTPS to WebSocket URL")
                })?;
            }
            _ => {
                return Err(LoxoneError::connection(
                    "Unsupported URL scheme for WebSocket",
                ))
            }
        }

        // Add WebSocket endpoint path
        ws_url.set_path("/ws/rfc6455");

        // Add authentication
        ws_url
            .query_pairs_mut()
            .append_pair("user", &credentials.username)
            .append_pair("password", &credentials.password);

        // Attempt connection
        let (ws_stream, response) = connect_async(&ws_url)
            .await
            .map_err(|e| LoxoneError::connection(format!("WebSocket reconnection failed: {e}")))?;

        debug!("WebSocket reconnected, response: {:?}", response.status());
        Ok(ws_stream)
    }

    /// Helper method for token-based reconnection attempts
    #[cfg(feature = "crypto-openssl")]
    async fn attempt_token_reconnection(
        base_url: &Url,
        token_client: &crate::client::TokenHttpClient,
        _config: &LoxoneConfig,
    ) -> Result<WsStream> {
        use tokio_tungstenite::connect_async;

        // Build WebSocket URL
        let mut ws_url = base_url.clone();

        // Convert HTTP(S) to WS(S)
        match ws_url.scheme() {
            "http" => {
                ws_url.set_scheme("ws").map_err(|_| {
                    LoxoneError::connection("Failed to convert HTTP to WebSocket URL")
                })?;
            }
            "https" => {
                ws_url.set_scheme("wss").map_err(|_| {
                    LoxoneError::connection("Failed to convert HTTPS to WebSocket URL")
                })?;
            }
            _ => {
                return Err(LoxoneError::connection(
                    "Unsupported URL scheme for WebSocket",
                ))
            }
        }

        // Add WebSocket endpoint path
        ws_url.set_path("/ws/rfc6455");

        // Get fresh authentication parameters from the token client
        let auth_params = token_client.get_auth_params().await.map_err(|e| {
            LoxoneError::connection(format!(
                "Failed to get auth params for WebSocket reconnection: {e}"
            ))
        })?;

        // Add token authentication
        ws_url.set_query(Some(&auth_params));

        // Attempt connection
        let (ws_stream, response) = connect_async(&ws_url).await.map_err(|e| {
            LoxoneError::connection(format!("WebSocket token reconnection failed: {e}"))
        })?;

        debug!(
            "WebSocket reconnected with token auth, response: {:?}",
            response.status()
        );
        Ok(ws_stream)
    }

    /// Process WebSocket messages (static method for background task)
    async fn process_ws_message(
        message: tokio_tungstenite::tungstenite::Message,
        state_sender: &Option<mpsc::UnboundedSender<StateUpdate>>,
    ) -> Result<()> {
        use tokio_tungstenite::tungstenite::Message;

        match message {
            Message::Text(text) => {
                debug!("Received text message: {}", text);

                // Try parsing as Loxone message
                if let Ok(loxone_msg) = serde_json::from_str::<LoxoneWebSocketMessage>(&text) {
                    Self::handle_loxone_message_static(loxone_msg, state_sender).await?;
                }
            }
            Message::Binary(data) => {
                debug!("Received binary message: {} bytes", data.len());
                Self::handle_binary_message_static(data).await?;
            }
            Message::Ping(_data) => {
                debug!("Received ping - pong will be sent automatically by tungstenite");
                // tungstenite handles ping/pong automatically
            }
            Message::Pong(_) => {
                debug!("Received pong");
            }
            Message::Close(close_frame) => {
                if let Some(frame) = close_frame {
                    warn!(
                        "WebSocket connection closed by server: {} - {}",
                        frame.code, frame.reason
                    );
                } else {
                    warn!("WebSocket connection closed by server");
                }
                return Err(LoxoneError::connection("WebSocket closed by server"));
            }
            Message::Frame(_) => {
                // Raw frame, usually not handled directly
            }
        }

        Ok(())
    }

    /// Instance method for backward compatibility
    #[allow(dead_code)]
    async fn process_message(
        &self,
        message: tokio_tungstenite::tungstenite::Message,
    ) -> Result<()> {
        Self::process_ws_message(message, &self.state_sender).await
    }

    /// Handle Loxone-specific message (static method for background task)
    async fn handle_loxone_message_static(
        message: LoxoneWebSocketMessage,
        state_sender: &Option<mpsc::UnboundedSender<StateUpdate>>,
    ) -> Result<()> {
        match message.msg_type.as_str() {
            "text" | "state" => {
                // Handle text-based state updates
                if let Some(uuid) = message.data.get("uuid").and_then(|v| v.as_str()) {
                    if let Some(value) = message.data.get("value") {
                        // Determine event type based on message content
                        let event_type = if message.data.get("type").and_then(|v| v.as_str())
                            == Some("weather")
                        {
                            LoxoneEventType::Weather
                        } else if message.data.get("type").and_then(|v| v.as_str()) == Some("alarm")
                        {
                            LoxoneEventType::Alarm
                        } else {
                            LoxoneEventType::State
                        };

                        let state_name = message
                            .data
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("value")
                            .to_string();

                        let update = StateUpdate {
                            uuid: uuid.to_string(),
                            state: state_name,
                            value: value.clone(),
                            previous_value: message.data.get("previous").cloned(),
                            event_type,
                            timestamp: message
                                .timestamp
                                .and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0))
                                .unwrap_or_else(chrono::Utc::now),
                            room: message
                                .data
                                .get("room")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            device_name: message
                                .data
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                        };

                        if let Some(sender) = state_sender {
                            if sender.send(update).is_err() {
                                warn!("Failed to send state update - receiver may be closed");
                            }
                        }
                    }
                }
            }
            "header" => {
                debug!("Received header message: {:?}", message.data);
                // Header messages often contain initialization data
                if let Some(version) = message.data.get("version") {
                    info!("Loxone Miniserver version: {}", version);
                }
            }
            "keepalive" | "ping" => {
                debug!("Received keepalive/ping message");
                // Connection is alive - no action needed
            }
            "weather" => {
                debug!("Received weather update: {:?}", message.data);
                // Handle weather-specific data
                if let Some(uuid) = message.data.get("uuid").and_then(|v| v.as_str()) {
                    let update = StateUpdate {
                        uuid: uuid.to_string(),
                        state: "weather".to_string(),
                        value: message.data.clone(),
                        previous_value: None,
                        event_type: LoxoneEventType::Weather,
                        timestamp: chrono::Utc::now(),
                        room: None,
                        device_name: None,
                    };

                    if let Some(sender) = state_sender {
                        let _ = sender.send(update);
                    }
                }
            }
            "text_message" => {
                debug!("Received text message: {:?}", message.data);
                if let Some(text) = message.data.get("text").and_then(|v| v.as_str()) {
                    info!("Loxone message: {}", text);
                }
            }
            _ => {
                debug!(
                    "Unknown message type '{}': {:?}",
                    message.msg_type, message.data
                );
            }
        }

        Ok(())
    }

    /// Instance method for backward compatibility
    #[allow(dead_code)]
    async fn handle_loxone_message(&self, message: LoxoneWebSocketMessage) -> Result<()> {
        Self::handle_loxone_message_static(message, &self.state_sender).await
    }

    /// Handle binary message (sensor data) - static method with enhanced parsing
    async fn handle_binary_message_static(data: Vec<u8>) -> Result<()> {
        // Binary messages in Loxone follow the Miniserver binary protocol
        // Header format (8 bytes):
        // - Bytes 0-3: Message type (little-endian u32)
        // - Bytes 4-7: Data length (little-endian u32)
        // Payload: Variable length data depending on message type

        if data.len() < 8 {
            debug!("Binary message too short: {} bytes", data.len());
            return Ok(());
        }

        // Extract header
        let msg_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let data_length = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

        debug!(
            "Binary message - type: 0x{:08X}, length: {}, total: {} bytes",
            msg_type,
            data_length,
            data.len()
        );

        // Validate data length
        if data.len() < 8 + data_length {
            warn!(
                "Binary message data length mismatch: expected {}, got {}",
                8 + data_length,
                data.len()
            );
            return Ok(());
        }

        // Extract payload
        let payload = &data[8..8 + data_length];

        // Parse based on Loxone binary protocol specification
        match msg_type {
            // Header message (connection established)
            0x03000000 => {
                debug!("Binary: Connection header received");
                Self::parse_header_message(payload).await?;
            }

            // Event table definition
            0x04000000 => {
                debug!("Binary: Event table definition");
                Self::parse_event_table(payload).await?;
            }

            // Value state updates (most common)
            0x00000000 => {
                debug!("Binary: Value state updates");
                Self::parse_value_states(payload).await?;
            }

            // Text state updates
            0x01000000 => {
                debug!("Binary: Text state updates");
                Self::parse_text_states(payload).await?;
            }

            // Daylight saving info
            0x02000000 => {
                debug!("Binary: Daylight saving information");
                Self::parse_daylight_saving(payload).await?;
            }

            // Weather data
            0x05000000 => {
                debug!("Binary: Weather data");
                return Err(LoxoneError::internal("Weather data parsing requires instance method - use handle_binary_message_instance"));
            }

            // Out-of-service indicator
            0x06000000 => {
                debug!("Binary: Out-of-service indicator");
                Self::parse_out_of_service(payload).await?;
            }

            // Keep-alive response
            0x07000000 => {
                debug!("Binary: Keep-alive response");
                // No additional data expected
            }

            // Unknown message type
            _ => {
                debug!(
                    "Binary: Unknown message type 0x{:08X}, payload: {} bytes",
                    msg_type,
                    payload.len()
                );
                // Log payload as hex for debugging
                if payload.len() <= 64 {
                    debug!("Payload hex: {}", hex::encode(payload));
                }
            }
        }

        Ok(())
    }

    /// Parse header message (connection info)
    async fn parse_header_message(payload: &[u8]) -> Result<()> {
        if payload.len() >= 4 {
            let version = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            info!("Loxone Miniserver protocol version: {}", version);
        }
        Ok(())
    }

    /// Parse event table definition
    async fn parse_event_table(payload: &[u8]) -> Result<()> {
        // Event table contains UUID to index mappings
        debug!(
            "Event table with {} bytes - contains UUID mappings",
            payload.len()
        );
        // Implementation would parse UUID->index mappings for efficient state updates
        Ok(())
    }

    /// Parse value state updates (double values)
    async fn parse_value_states(payload: &[u8]) -> Result<()> {
        let mut cursor = Cursor::new(payload);
        let mut states_parsed = 0;

        // Each value state entry: 4 bytes (index) + 8 bytes (double value)
        while cursor.position() + 12 <= payload.len() as u64 {
            let mut index_bytes = [0u8; 4];
            let mut value_bytes = [0u8; 8];

            if cursor.read_exact(&mut index_bytes).is_ok()
                && cursor.read_exact(&mut value_bytes).is_ok()
            {
                let index = u32::from_le_bytes(index_bytes);
                let value = f64::from_le_bytes(value_bytes);

                debug!("Value state update - index: {}, value: {}", index, value);
                states_parsed += 1;
            } else {
                break;
            }
        }

        debug!("Parsed {} value state updates", states_parsed);
        Ok(())
    }

    /// Parse text state updates (string values)
    async fn parse_text_states(payload: &[u8]) -> Result<()> {
        let mut cursor = Cursor::new(payload);
        let mut states_parsed = 0;

        // Each text state entry: 4 bytes (index) + 4 bytes (text length) + text data
        while cursor.position() + 8 <= payload.len() as u64 {
            let mut index_bytes = [0u8; 4];
            let mut length_bytes = [0u8; 4];

            if cursor.read_exact(&mut index_bytes).is_ok()
                && cursor.read_exact(&mut length_bytes).is_ok()
            {
                let index = u32::from_le_bytes(index_bytes);
                let text_length = u32::from_le_bytes(length_bytes) as usize;

                if cursor.position() + text_length as u64 <= payload.len() as u64 {
                    let mut text_bytes = vec![0u8; text_length];
                    if cursor.read_exact(&mut text_bytes).is_ok() {
                        if let Ok(text) = String::from_utf8(text_bytes) {
                            debug!("Text state update - index: {}, text: '{}'", index, text);
                            states_parsed += 1;
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        debug!("Parsed {} text state updates", states_parsed);
        Ok(())
    }

    /// Parse daylight saving information
    async fn parse_daylight_saving(payload: &[u8]) -> Result<()> {
        if payload.len() >= 8 {
            let dst_offset = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let timezone_offset =
                u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            debug!(
                "Daylight saving - DST offset: {}, timezone offset: {}",
                dst_offset, timezone_offset
            );
        }
        Ok(())
    }

    /// Parse weather data
    async fn parse_weather_data(&self, payload: &[u8]) -> Result<()> {
        debug!("Weather data received: {} bytes", payload.len());

        if payload.is_empty() {
            return Ok(());
        }

        let mut cursor = Cursor::new(payload);
        let mut weather_updates_parsed = 0;

        // Loxone weather data format:
        // Each weather data entry: 4 bytes (UUID index) + 8 bytes (double value) + 4 bytes (timestamp)
        while cursor.position() + 16 <= payload.len() as u64 {
            let mut uuid_index_bytes = [0u8; 4];
            let mut value_bytes = [0u8; 8];
            let mut timestamp_bytes = [0u8; 4];

            if cursor.read_exact(&mut uuid_index_bytes).is_ok()
                && cursor.read_exact(&mut value_bytes).is_ok()
                && cursor.read_exact(&mut timestamp_bytes).is_ok()
            {
                let uuid_index = u32::from_le_bytes(uuid_index_bytes);
                let value = f64::from_le_bytes(value_bytes);
                let timestamp = u32::from_le_bytes(timestamp_bytes);

                debug!(
                    "Weather update - UUID index: {}, value: {:.2}, timestamp: {}",
                    uuid_index, value, timestamp
                );

                // Store weather data for retrieval by weather resources
                self.store_weather_update(uuid_index, value, timestamp)
                    .await?;

                weather_updates_parsed += 1;
            } else {
                break;
            }
        }

        if weather_updates_parsed > 0 {
            debug!("Parsed {} weather data updates", weather_updates_parsed);
        } else {
            // Try alternative weather data format (some stations use different layouts)
            Self::parse_alternative_weather_format(payload).await?;
        }

        Ok(())
    }

    /// Store weather update data for later retrieval
    async fn store_weather_update(
        &self,
        uuid_index: u32,
        value: f64,
        timestamp: u32,
    ) -> Result<()> {
        if let Some(storage) = &self.weather_storage {
            // Store weather data with automatic UUID resolution
            storage
                .store_weather_update(
                    uuid_index,
                    value,
                    timestamp,
                    Some("weather_value"), // Default parameter name
                    None,                  // Unit will be determined by device type
                    Some(1.0),             // Default quality score
                )
                .await?;

            debug!(
                "Stored weather data: index={}, value={:.2}, ts={}",
                uuid_index, value, timestamp
            );
        } else {
            debug!(
                "Weather storage not enabled - logging only: index={}, value={:.2}, ts={}",
                uuid_index, value, timestamp
            );
        }

        Ok(())
    }

    /// Parse alternative weather data format for different weather station types
    async fn parse_alternative_weather_format(payload: &[u8]) -> Result<()> {
        // Some weather stations send data in different formats
        // Try parsing as structured weather data packet
        if payload.len() >= 8 {
            let mut cursor = Cursor::new(payload);

            // Check for weather data packet header
            let mut header_bytes = [0u8; 4];
            if cursor.read_exact(&mut header_bytes).is_ok() {
                let header = u32::from_le_bytes(header_bytes);

                match header {
                    // Weather station data packet
                    0x57455448 => {
                        // "WETH" in ASCII
                        debug!("Found structured weather data packet");
                        Self::parse_structured_weather_packet(&payload[4..]).await?;
                    }
                    _ => {
                        // Unknown format, log as hex for debugging
                        if payload.len() <= 64 {
                            debug!("Unknown weather data format, hex: {}", hex::encode(payload));
                        } else {
                            debug!(
                                "Unknown weather data format, {} bytes, header: 0x{:08X}",
                                payload.len(),
                                header
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse structured weather data packet
    async fn parse_structured_weather_packet(payload: &[u8]) -> Result<()> {
        let mut cursor = Cursor::new(payload);

        // Structured weather packet format:
        // 4 bytes: station ID
        // 8 bytes: temperature (double)
        // 8 bytes: humidity (double)
        // 8 bytes: pressure (double)
        // 8 bytes: wind speed (double)
        // 8 bytes: wind direction (double)
        // 8 bytes: precipitation (double)
        // 4 bytes: timestamp

        if payload.len() >= 52 {
            // Minimum size for complete weather packet
            let mut station_id_bytes = [0u8; 4];
            let mut temp_bytes = [0u8; 8];
            let mut humidity_bytes = [0u8; 8];
            let mut pressure_bytes = [0u8; 8];
            let mut wind_speed_bytes = [0u8; 8];
            let mut wind_dir_bytes = [0u8; 8];
            let mut precipitation_bytes = [0u8; 8];
            let mut timestamp_bytes = [0u8; 4];

            if cursor.read_exact(&mut station_id_bytes).is_ok()
                && cursor.read_exact(&mut temp_bytes).is_ok()
                && cursor.read_exact(&mut humidity_bytes).is_ok()
                && cursor.read_exact(&mut pressure_bytes).is_ok()
                && cursor.read_exact(&mut wind_speed_bytes).is_ok()
                && cursor.read_exact(&mut wind_dir_bytes).is_ok()
                && cursor.read_exact(&mut precipitation_bytes).is_ok()
                && cursor.read_exact(&mut timestamp_bytes).is_ok()
            {
                let station_id = u32::from_le_bytes(station_id_bytes);
                let temperature = f64::from_le_bytes(temp_bytes);
                let humidity = f64::from_le_bytes(humidity_bytes);
                let pressure = f64::from_le_bytes(pressure_bytes);
                let wind_speed = f64::from_le_bytes(wind_speed_bytes);
                let wind_direction = f64::from_le_bytes(wind_dir_bytes);
                let precipitation = f64::from_le_bytes(precipitation_bytes);
                let timestamp = u32::from_le_bytes(timestamp_bytes);

                debug!(
                    "Weather station {} - temp: {:.1}°C, humidity: {:.1}%, pressure: {:.1}hPa, wind: {:.1}km/h@{:.0}°, rain: {:.1}mm, ts: {}",
                    station_id, temperature, humidity, pressure, wind_speed, wind_direction, precipitation, timestamp
                );

                // Store structured weather data
                Self::store_structured_weather_data(
                    station_id,
                    temperature,
                    humidity,
                    pressure,
                    wind_speed,
                    wind_direction,
                    precipitation,
                    timestamp,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Store structured weather data
    async fn store_structured_weather_data(
        station_id: u32,
        temperature: f64,
        humidity: f64,
        pressure: f64,
        wind_speed: f64,
        wind_direction: f64,
        precipitation: f64,
        timestamp: u32,
    ) -> Result<()> {
        // Store structured weather data using existing weather storage infrastructure
        debug!(
            "Storing structured weather data for station {}: T={:.1}°C, H={:.1}%, P={:.1}hPa, Wind={:.1}km/h@{:.0}°, Rain={:.1}mm",
            station_id, temperature, humidity, pressure, wind_speed, wind_direction, precipitation
        );

        // Note: This is a static method, so we can't access self.weather_storage
        // The actual weather data storage is handled by the store_weather_update method
        // which is called from the WebSocket message processing loop
        info!(
            "Weather station {} data: {}°C, {}% humidity, {}hPa pressure at timestamp {}",
            station_id, temperature, humidity, pressure, timestamp
        );
        Ok(())
    }

    /// Parse out-of-service indicator
    async fn parse_out_of_service(payload: &[u8]) -> Result<()> {
        if payload.len() >= 4 {
            let service_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            warn!("Service {} is out of service", service_id);
        }
        Ok(())
    }

    /// Instance method for backward compatibility
    #[allow(dead_code)]
    async fn handle_binary_message(&self, data: Vec<u8>) -> Result<()> {
        // Check if this is weather data which requires instance access
        if data.len() >= 8 {
            let msg_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            if msg_type == 0x05000000 {
                // Weather data - handle with instance method
                return self.handle_binary_message_instance(data).await;
            }
        }

        // For all other binary messages, use static method
        Self::handle_binary_message_static(data).await
    }

    /// Instance method for binary messages that need access to weather storage
    async fn handle_binary_message_instance(&self, data: Vec<u8>) -> Result<()> {
        if data.len() < 8 {
            debug!("Binary message too short: {} bytes", data.len());
            return Ok(());
        }

        let msg_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let data_length = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

        debug!(
            "Binary message (instance) - type: 0x{:08X}, length: {}, total: {} bytes",
            msg_type,
            data_length,
            data.len()
        );

        // Validate data length
        if data.len() < 8 + data_length {
            warn!(
                "Binary message data length mismatch: expected {}, got {}",
                8 + data_length,
                data.len()
            );
            return Ok(());
        }

        // Extract payload
        let payload = &data[8..8 + data_length];

        // Parse based on Loxone binary protocol specification
        match msg_type {
            // Weather data
            0x05000000 => {
                debug!("Binary: Weather data");
                self.parse_weather_data(payload).await?;
            }

            _ => {
                debug!(
                    "Unhandled message type in instance method: 0x{:08X}",
                    msg_type
                );
            }
        }

        Ok(())
    }

    /// Get public context for external access
    pub fn context(&self) -> &ClientContext {
        &self.context
    }

    /// Legacy method for backward compatibility
    pub async fn subscribe_to_updates(&self) -> mpsc::UnboundedReceiver<StateUpdate> {
        self.subscribe().await
    }

    /// Subscribe to specific device state changes
    pub async fn subscribe_to_device_state(
        &self,
        device_uuid: String,
        state_name: String,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let mut device_uuids = HashSet::new();
        device_uuids.insert(device_uuid);

        let mut states = HashSet::new();
        states.insert(state_name);

        let filter = EventFilter {
            device_uuids,
            states,
            ..Default::default()
        };

        self.subscribe_with_filter(filter).await
    }

    /// Subscribe to all state changes in specific rooms
    pub async fn subscribe_to_room_updates(
        &self,
        room_names: Vec<String>,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let rooms = room_names.into_iter().collect();
        let filter = EventFilter {
            rooms,
            ..Default::default()
        };

        self.subscribe_with_filter(filter).await
    }

    /// Subscribe to specific event types with optional device filtering
    pub async fn subscribe_to_events(
        &self,
        event_types: Vec<LoxoneEventType>,
        device_uuids: Option<Vec<String>>,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let event_types_set = event_types.into_iter().collect();
        let device_uuids_set = device_uuids
            .map(|uuids| uuids.into_iter().collect())
            .unwrap_or_default();

        let filter = EventFilter {
            event_types: event_types_set,
            device_uuids: device_uuids_set,
            ..Default::default()
        };

        self.subscribe_with_filter(filter).await
    }

    /// Configure and start automatic device state subscriptions for all known devices
    pub async fn enable_full_monitoring(&self) -> Result<()> {
        let devices = self.context.devices.read().await;
        let device_uuids: HashSet<String> = devices.keys().cloned().collect();

        let filter = EventFilter {
            device_uuids,
            event_types: [LoxoneEventType::State, LoxoneEventType::Sensor]
                .iter()
                .cloned()
                .collect(),
            min_interval: Some(Duration::from_millis(100)), // 100ms debounce
            ..Default::default()
        };

        // Create subscription but don't store the receiver (fire-and-forget monitoring)
        let _receiver = self.subscribe_with_filter(filter).await;
        info!(
            "Full device monitoring enabled for {} devices",
            devices.len()
        );

        Ok(())
    }

    /// Initialize an encrypted WebSocket session
    pub async fn init_encryption_session(&self, session_duration_hours: u32) -> Result<String> {
        use crate::security::encryption::EncryptionSession;

        debug!(
            "Initializing encryption session for {} hours",
            session_duration_hours
        );

        // Create new encryption session
        let session = EncryptionSession::new(session_duration_hours);
        let session_id = session.session_id.clone();

        // Add session to manager
        {
            let mut manager = self.encryption_manager.write().await;
            manager.add_session(session).map_err(|e| {
                LoxoneError::connection(&format!("Failed to add encryption session: {}", e))
            })?;
        }

        // Store current session ID
        {
            let mut current_session = self.encryption_session.write().await;
            *current_session = Some(session_id.clone());
        }

        info!("✅ Encryption session initialized: {}", &session_id[..8]);
        Ok(session_id)
    }

    /// Send encrypted message via WebSocket
    pub async fn send_encrypted_message(&self, message: &[u8]) -> Result<()> {
        let session_id = {
            let current_session = self.encryption_session.read().await;
            current_session
                .as_ref()
                .ok_or_else(|| LoxoneError::connection("No active encryption session"))?
                .clone()
        };

        // Encrypt the message
        let encrypted_msg = {
            let mut manager = self.encryption_manager.write().await;
            let session = manager
                .get_session_mut(&session_id)
                .ok_or_else(|| LoxoneError::connection("Encryption session not found"))?;

            session
                .encrypt_message(message)
                .map_err(|e| LoxoneError::connection(&format!("Encryption failed: {}", e)))?
        };

        // Send encrypted message via WebSocket
        if let Some(stream) = &self.ws_stream {
            use tokio_tungstenite::tungstenite::Message;

            let json_payload = serde_json::to_string(&encrypted_msg).map_err(|e| {
                LoxoneError::connection(&format!("Failed to serialize encrypted message: {}", e))
            })?;

            let mut stream_guard = stream.lock().await;
            stream_guard
                .send(Message::Text(json_payload))
                .await
                .map_err(|e| {
                    LoxoneError::connection(&format!("Failed to send encrypted message: {}", e))
                })?;

            debug!("Sent encrypted message: {} bytes", message.len());
        } else {
            return Err(LoxoneError::connection("WebSocket not connected"));
        }

        Ok(())
    }

    /// Decrypt received message
    pub async fn decrypt_message(
        &self,
        encrypted_msg: &crate::security::encryption::EncryptedMessage,
    ) -> Result<Vec<u8>> {
        let session_id = {
            let current_session = self.encryption_session.read().await;
            current_session
                .as_ref()
                .ok_or_else(|| LoxoneError::connection("No active encryption session"))?
                .clone()
        };

        // Decrypt the message
        let plaintext = {
            let mut manager = self.encryption_manager.write().await;
            let session = manager
                .get_session_mut(&session_id)
                .ok_or_else(|| LoxoneError::connection("Encryption session not found"))?;

            session
                .decrypt_message(encrypted_msg)
                .map_err(|e| LoxoneError::connection(&format!("Decryption failed: {}", e)))?
        };

        debug!("Decrypted message: {} bytes", plaintext.len());
        Ok(plaintext)
    }

    /// Check if encryption session is active and valid
    pub async fn is_encryption_active(&self) -> bool {
        let session_id = {
            let current_session = self.encryption_session.read().await;
            match current_session.as_ref() {
                Some(id) => id.clone(),
                None => return false,
            }
        };

        let manager = self.encryption_manager.read().await;
        if let Some(session) = manager.get_session(&session_id) {
            session.is_valid()
        } else {
            false
        }
    }

    /// Get encryption statistics
    pub async fn get_encryption_stats(
        &self,
    ) -> Option<crate::security::encryption::EncryptionStats> {
        let session_id = {
            let current_session = self.encryption_session.read().await;
            current_session.as_ref()?.clone()
        };

        let manager = self.encryption_manager.read().await;
        manager
            .get_session(&session_id)
            .map(|session| session.get_stats().clone())
    }

    /// Cleanup expired encryption sessions
    pub async fn cleanup_encryption_sessions(&self) -> usize {
        let mut manager = self.encryption_manager.write().await;
        manager.cleanup_expired_sessions()
    }

    /// Terminate current encryption session
    pub async fn terminate_encryption_session(&self) -> Result<()> {
        let session_id = {
            let mut current_session = self.encryption_session.write().await;
            current_session.take()
        };

        if let Some(session_id) = session_id {
            let mut manager = self.encryption_manager.write().await;
            manager.remove_session(&session_id);
            info!("Encryption session terminated: {}", &session_id[..8]);
        }

        Ok(())
    }
}

#[cfg(feature = "websocket")]
#[async_trait]
impl LoxoneClient for LoxoneWebSocketClient {
    async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting WebSocket to Loxone Miniserver at {}",
            self.base_url
        );

        let ws_url = self.build_ws_url().await?;
        debug!("WebSocket URL: {}", ws_url);

        // Connect to WebSocket
        let (ws_stream, response) = connect_async(&ws_url)
            .await
            .map_err(|e| LoxoneError::connection(format!("WebSocket connection failed: {e}")))?;

        debug!("WebSocket connected, response: {:?}", response.status());

        self.ws_stream = Some(Arc::new(Mutex::new(ws_stream)));
        *self.connected.write().await = true;
        *self.context.connected.write().await = true;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.connection_start = Some(chrono::Utc::now());
        }

        // Start background tasks
        self.start_background_tasks().await?;

        info!("✅ Connected to Loxone WebSocket");
        Ok(())
    }

    async fn is_connected(&self) -> Result<bool> {
        Ok(*self.connected.read().await && *self.context.connected.read().await)
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Abort all background tasks
        {
            let mut handles = self.task_handles.lock().await;
            for handle in handles.drain(..) {
                handle.abort();
            }
        }

        self.ws_stream = None;
        self.state_sender = None;
        *self.connected.write().await = false;
        *self.context.connected.write().await = false;

        // Clear subscribers
        self.clear_subscribers().await;

        info!("Disconnected from Loxone WebSocket");
        Ok(())
    }

    async fn send_command(&self, uuid: &str, command: &str) -> Result<LoxoneResponse> {
        if !*self.connected.read().await {
            return Err(LoxoneError::connection("WebSocket not connected"));
        }

        // For WebSocket, we can either send commands via WebSocket or delegate to HTTP client
        if let Some(http_client) = &self.http_client {
            // Use HTTP client for commands (more reliable)
            http_client.send_command(uuid, command).await
        } else {
            // Implement WebSocket command sending based on Loxone protocol
            // Format: "jdev/sps/io/{uuid}/{command}"
            let ws_command = format!("jdev/sps/io/{}/{}", uuid, command);
            debug!("Sending WebSocket command: {}", ws_command);

            // Send command via WebSocket
            if let Some(ws_stream) = &self.ws_stream {
                let mut stream = ws_stream.lock().await;

                match stream
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        ws_command.clone(),
                    ))
                    .await
                {
                    Ok(_) => {
                        debug!("Successfully sent WebSocket command: {}", ws_command);

                        // Return success response
                        Ok(LoxoneResponse {
                            code: 200,
                            value: serde_json::json!({
                                "status": "success",
                                "uuid": uuid,
                                "command": command,
                                "sent_via": "websocket",
                                "ws_command": ws_command
                            }),
                        })
                    }
                    Err(e) => {
                        error!("Failed to send WebSocket command: {}", e);
                        Err(LoxoneError::connection(format!(
                            "WebSocket command failed: {}",
                            e
                        )))
                    }
                }
            } else {
                Err(LoxoneError::connection("WebSocket stream not available"))
            }
        }
    }

    async fn get_structure(&self) -> Result<LoxoneStructure> {
        // Use HTTP client if available, otherwise error
        if let Some(http_client) = &self.http_client {
            http_client.get_structure().await
        } else {
            Err(LoxoneError::connection(
                "Structure file not available via WebSocket - HTTP client required",
            ))
        }
    }

    async fn get_device_states(
        &self,
        uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        let devices = self.context.devices.read().await;
        let mut states = HashMap::new();

        for uuid in uuids {
            if let Some(device) = devices.get(uuid) {
                // Return current cached states
                for (state_name, value) in &device.states {
                    states.insert(format!("{uuid}:{state_name}"), value.clone());
                }
            }
        }

        Ok(states)
    }

    async fn get_state_values(
        &self,
        state_uuids: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Use HTTP client if available for state UUID resolution
        if let Some(http_client) = &self.http_client {
            http_client.get_state_values(state_uuids).await
        } else {
            // Fallback: try to resolve from cached device states
            let mut state_values = HashMap::new();
            let devices = self.context.devices.read().await;

            for state_uuid in state_uuids {
                // Look for the state UUID in device states
                for device in devices.values() {
                    for state_value in device.states.values() {
                        if let Some(uuid_str) = state_value.as_str() {
                            if uuid_str == state_uuid {
                                // Found the state UUID, but we need the actual value
                                // For now, return the UUID itself - this is a limitation without HTTP client
                                state_values.insert(state_uuid.clone(), state_value.clone());
                                break;
                            }
                        }
                    }
                }
            }

            Ok(state_values)
        }
    }

    async fn get_system_info(&self) -> Result<serde_json::Value> {
        // Use HTTP client if available, otherwise error
        if let Some(http_client) = &self.http_client {
            http_client.get_system_info().await
        } else {
            Err(LoxoneError::connection(
                "System info not available via WebSocket - HTTP client required",
            ))
        }
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(*self.connected.read().await)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LoxoneWebSocketClient {
    /// Map Loxone device type to sensor type for logging
    fn map_device_type_to_sensor_type(device_type: &str) -> crate::tools::sensors::SensorType {
        use crate::tools::sensors::SensorType;

        match device_type {
            // Door/Window sensors
            "InfoOnlyDigital" | "DigitalInput" | "GateController" => SensorType::DoorWindow,

            // Motion/Presence sensors
            "PresenceDetector" | "MotionSensor" | "PresenceSensor" => SensorType::Motion,

            // Temperature sensors
            "TemperatureSensor" | "Thermometer" => SensorType::Temperature,

            // Humidity sensors
            "HumiditySensor" => SensorType::Humidity,

            // Light sensors
            "LightSensor" | "LightController" | "LightControllerV2" => SensorType::Light,

            // Air quality sensors
            "AirQualitySensor" | "CO2Sensor" => SensorType::AirQuality,

            // Weather devices (general analog)
            "WeatherStation" | "InfoOnlyAnalog" => SensorType::Analog,

            // Energy/Power sensors
            "PowerMeter" | "EnergyMeter" | "Meter" => SensorType::Energy,

            // HVAC sensors (temperature)
            "IRoomControllerV2" | "RoomController" | "ThermostatController" => {
                SensorType::Temperature
            }

            // Security sensors (door/window as fallback)
            "AlarmController" | "SecurityZone" | "AccessControl" => SensorType::DoorWindow,

            // Sound/Audio sensors (generic analog)
            "AudioZone" | "MusicServer" | "SpeakerController" => SensorType::Analog,

            // Blinds/Shade sensors (analog for position)
            "Jalousie" | "BlindController" | "SunshadeController" => SensorType::Analog,

            // Pool sensors (analog for measurements)
            "PoolController" | "SaunaController" => SensorType::Analog,

            // Irrigation sensors (analog)
            "IrrigationController" | "GardenController" => SensorType::Analog,

            // Generic fallback
            _ => SensorType::Analog,
        }
    }
}

// Placeholder implementations for when websocket feature is disabled
#[cfg(not(feature = "websocket"))]
pub struct LoxoneWebSocketClient;

#[cfg(not(feature = "websocket"))]
impl LoxoneWebSocketClient {
    pub async fn new(
        _config: crate::config::LoxoneConfig,
        _credentials: crate::config::LoxoneCredentials,
    ) -> Result<Self> {
        Err(LoxoneError::config(
            "WebSocket client not available - enable 'websocket' feature",
        ))
    }
}
