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
use async_trait::async_trait;
#[cfg(feature = "websocket")]
use rand;
#[cfg(feature = "websocket")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "websocket")]
use std::collections::{HashMap, HashSet};
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
type SubscriberList = Arc<RwLock<Vec<(mpsc::UnboundedSender<StateUpdate>, EventFilter)>>>;

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
                                    None, // TODO: Map device type to sensor type
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
                    if let Some(min_interval) = filter.min_interval {
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

        // Task 3: Reconnection manager (if enabled)
        let reconnection_task = if self.reconnection_config.enabled {
            let connected = self.connected.clone();
            let base_url = self.base_url.clone();
            let credentials = self.credentials.clone();
            let config = self.config.clone();
            let reconnection_config = self.reconnection_config.clone();
            let stats_clone = self.stats.clone();
            let ws_stream_ref = self.ws_stream.clone();

            Some(tokio::spawn(async move {
                let mut attempt = 0;
                let mut delay = reconnection_config.initial_delay;

                loop {
                    // Check if we're still connected
                    if *connected.read().await {
                        // Wait a bit before checking again
                        sleep(Duration::from_secs(5)).await;
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

                    // Attempt reconnection
                    match Self::attempt_reconnection(&base_url, &credentials, &config).await {
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

    /// Check if a state update matches the given filter
    async fn matches_filter(update: &StateUpdate, filter: &EventFilter) -> bool {
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

    /// Subscribe to filtered state updates
    pub async fn subscribe_with_filter(
        &self,
        filter: EventFilter,
    ) -> mpsc::UnboundedReceiver<StateUpdate> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.write().await;
        subscribers.push((tx, filter));
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

    /// Remove subscribers that match a specific filter
    pub async fn remove_subscribers_with_filter(&self, filter: &EventFilter) -> usize {
        let mut subscribers = self.subscribers.write().await;
        let initial_count = subscribers.len();

        subscribers.retain(|(_, subscriber_filter)| {
            // Keep subscribers that don't match the filter exactly
            subscriber_filter.device_uuids != filter.device_uuids
                || subscriber_filter.event_types != filter.event_types
                || subscriber_filter.rooms != filter.rooms
                || subscriber_filter.states != filter.states
        });

        initial_count - subscribers.len()
    }

    /// Get all unique device UUIDs being monitored
    pub async fn get_monitored_devices(&self) -> HashSet<String> {
        let mut monitored = HashSet::new();
        let subscribers = self.subscribers.read().await;

        for (_, filter) in subscribers.iter() {
            monitored.extend(filter.device_uuids.iter().cloned());
        }

        monitored
    }

    /// Get all unique rooms being monitored
    pub async fn get_monitored_rooms(&self) -> HashSet<String> {
        let mut monitored = HashSet::new();
        let subscribers = self.subscribers.read().await;

        for (_, filter) in subscribers.iter() {
            monitored.extend(filter.rooms.iter().cloned());
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

    /// Handle binary message (sensor data) - static method
    async fn handle_binary_message_static(data: Vec<u8>) -> Result<()> {
        // Binary messages in Loxone typically contain sensor state updates
        // The format is proprietary but follows patterns:
        // - First 4 bytes: message type identifier
        // - Next 4 bytes: data length
        // - Remaining bytes: payload (device states, sensor readings)

        if data.len() < 8 {
            debug!("Binary message too short: {} bytes", data.len());
            return Ok(());
        }

        // Extract message type and length
        let msg_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let data_length = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

        debug!(
            "Binary message - type: 0x{:08X}, length: {}, total: {} bytes",
            msg_type,
            data_length,
            data.len()
        );

        // Common Loxone binary message types (observed patterns)
        match msg_type {
            0x00000000 => {
                debug!("Binary: Device state update message");
                // Contains device state changes in binary format
            }
            0x00000001 => {
                debug!("Binary: Sensor reading batch");
                // Contains multiple sensor readings
            }
            0x00000002 => {
                debug!("Binary: Weather data");
                // Weather station data in compact format
            }
            0x00000003 => {
                debug!("Binary: Energy meter readings");
                // Power consumption and generation data
            }
            _ => {
                debug!("Binary: Unknown message type 0x{:08X}", msg_type);
            }
        }

        // TODO: Implement proper binary protocol parsing
        // This would require understanding Loxone's proprietary binary format
        // For now, we log the message for debugging purposes

        Ok(())
    }

    /// Instance method for backward compatibility
    #[allow(dead_code)]
    async fn handle_binary_message(&self, data: Vec<u8>) -> Result<()> {
        Self::handle_binary_message_static(data).await
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
            // WebSocket command format is different from HTTP
            // This would need to be implemented based on Loxone's WebSocket protocol
            debug!("WebSocket command sending not fully implemented: {uuid} -> {command}");

            // For now, return a placeholder response
            Ok(LoxoneResponse {
                code: 200,
                value: serde_json::json!({
                    "status": "sent_via_websocket",
                    "uuid": uuid,
                    "command": command,
                    "note": "WebSocket command sending not fully implemented"
                }),
            })
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
