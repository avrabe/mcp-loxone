//! WebSocket resilience and message acknowledgment system
//!
//! This module provides reliable WebSocket communication with message acknowledgment,
//! automatic reconnection, message queuing, and duplicate detection.

use crate::error::{LoxoneError, Result};
use chrono::{DateTime, Duration, Utc};
use md5;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::time::{interval, sleep, Duration as TokioDuration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[cfg(feature = "websocket")]
/// WebSocket resilience configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketResilienceConfig {
    /// Message acknowledgment timeout
    pub ack_timeout: Duration,
    /// Maximum number of pending unacknowledged messages
    pub max_pending_messages: usize,
    /// Reconnection configuration
    pub reconnection: ReconnectionConfig,
    /// Message retry configuration
    pub retry_config: MessageRetryConfig,
    /// Heartbeat configuration
    pub heartbeat: HeartbeatConfig,
    /// Enable message deduplication
    pub enable_deduplication: bool,
    /// Maximum size of message history for deduplication
    pub dedup_history_size: usize,
}

/// Reconnection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionConfig {
    /// Enable automatic reconnection
    pub enabled: bool,
    /// Initial reconnection delay
    pub initial_delay: Duration,
    /// Maximum reconnection delay
    pub max_delay: Duration,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Maximum number of reconnection attempts
    pub max_attempts: Option<u32>,
    /// Jitter factor to avoid thundering herd (0.0-1.0)
    pub jitter_factor: f64,
}

/// Message retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRetryConfig {
    /// Maximum number of retry attempts per message
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Enable exponential backoff for retries
    pub exponential_backoff: bool,
}

/// Heartbeat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// Enable heartbeat
    pub enabled: bool,
    /// Heartbeat interval
    pub interval: Duration,
    /// Heartbeat timeout (time to wait for pong response)
    pub timeout: Duration,
    /// Maximum missed heartbeats before declaring connection dead
    pub max_missed: u32,
}

/// Resilient message with acknowledgment tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilientMessage {
    /// Unique message ID
    pub id: String,
    /// Message payload
    pub payload: String,
    /// Message type
    pub message_type: MessageType,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Last attempt timestamp
    pub last_attempt: DateTime<Utc>,
    /// Priority level
    pub priority: MessagePriority,
    /// Acknowledgment required
    pub requires_ack: bool,
    /// Expiration timestamp (optional)
    pub expires_at: Option<DateTime<Utc>>,
}

/// Message types for different purposes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageType {
    /// Command message to Loxone
    Command,
    /// Heartbeat ping
    Heartbeat,
    /// Acknowledgment response
    Acknowledgment,
    /// Data query
    Query,
    /// Subscription message
    Subscription,
    /// Custom application message
    Custom(String),
}

/// Message priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// WebSocket resilience manager
pub struct WebSocketResilienceManager {
    /// Configuration
    config: WebSocketResilienceConfig,
    /// WebSocket URL
    url: String,
    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Pending messages awaiting acknowledgment
    pending_messages: Arc<RwLock<HashMap<String, ResilientMessage>>>,
    /// Message queue for outgoing messages
    outgoing_queue: Arc<Mutex<VecDeque<ResilientMessage>>>,
    /// Message history for deduplication
    message_history: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// Message ID counter
    message_counter: Arc<AtomicU64>,
    /// Connection attempt counter
    connection_attempts: Arc<AtomicU64>,
    /// Last successful connection timestamp
    last_connected: Arc<RwLock<Option<DateTime<Utc>>>>,
    /// Heartbeat manager
    heartbeat_manager: Arc<HeartbeatManager>,
    /// Event broadcaster for connection events
    event_sender: broadcast::Sender<ResilienceEvent>,
    /// Message sender for outgoing messages
    message_sender: Arc<RwLock<Option<mpsc::UnboundedSender<ResilientMessage>>>>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Statistics
    stats: Arc<RwLock<ResilienceStatistics>>,
}

/// Heartbeat manager for connection health monitoring
struct HeartbeatManager {
    config: HeartbeatConfig,
    last_ping: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_pong: Arc<RwLock<Option<DateTime<Utc>>>>,
    missed_count: Arc<AtomicU64>,
    enabled: Arc<AtomicBool>,
}

/// Resilience events for monitoring and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResilienceEvent {
    Connected {
        attempt: u64,
        duration: Duration,
    },
    Disconnected {
        reason: String,
    },
    ReconnectionStarted {
        attempt: u64,
        delay: Duration,
    },
    MessageSent {
        message_id: String,
        message_type: MessageType,
    },
    MessageAcknowledged {
        message_id: String,
        response_time: Duration,
    },
    MessageTimeout {
        message_id: String,
        retry_count: u32,
    },
    MessageFailed {
        message_id: String,
        error: String,
    },
    HeartbeatMissed {
        count: u64,
    },
    ConnectionHealthy,
    QueueOverflow {
        dropped_messages: usize,
    },
}

/// Resilience statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResilienceStatistics {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages acknowledged
    pub messages_acknowledged: u64,
    /// Total messages failed
    pub messages_failed: u64,
    /// Total connection attempts
    pub connection_attempts: u64,
    /// Total successful connections
    pub successful_connections: u64,
    /// Current pending messages
    pub pending_messages: usize,
    /// Current queue size
    pub queue_size: usize,
    /// Average acknowledgment time (milliseconds)
    pub avg_ack_time_ms: f64,
    /// Connection uptime percentage
    pub uptime_percentage: f64,
    /// Last connection timestamp
    pub last_connected: Option<DateTime<Utc>>,
    /// Duplicate messages detected
    pub duplicates_detected: u64,
}

impl Default for WebSocketResilienceConfig {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::seconds(30),
            max_pending_messages: 1000,
            reconnection: ReconnectionConfig::default(),
            retry_config: MessageRetryConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            enable_deduplication: true,
            dedup_history_size: 10000,
        }
    }
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: Duration::seconds(1),
            max_delay: Duration::minutes(5),
            backoff_multiplier: 2.0,
            max_attempts: None, // Unlimited
            jitter_factor: 0.1,
        }
    }
}

impl Default for MessageRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::seconds(5),
            exponential_backoff: true,
        }
    }
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::seconds(30),
            timeout: Duration::seconds(10),
            max_missed: 3,
        }
    }
}

impl HeartbeatManager {
    fn new(config: HeartbeatConfig) -> Self {
        let enabled = config.enabled;
        Self {
            config,
            last_ping: Arc::new(RwLock::new(None)),
            last_pong: Arc::new(RwLock::new(None)),
            missed_count: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    async fn send_ping(&self) -> ResilientMessage {
        *self.last_ping.write().await = Some(Utc::now());

        ResilientMessage {
            id: format!("ping-{}", Uuid::new_v4()),
            payload: "ping".to_string(),
            message_type: MessageType::Heartbeat,
            created_at: Utc::now(),
            retry_count: 0,
            last_attempt: Utc::now(),
            priority: MessagePriority::High,
            requires_ack: true,
            expires_at: Some(Utc::now() + self.config.timeout),
        }
    }

    #[allow(dead_code)]
    async fn handle_pong(&self) {
        *self.last_pong.write().await = Some(Utc::now());
        self.missed_count.store(0, Ordering::Relaxed);
    }

    async fn check_health(&self) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return true;
        }

        let now = Utc::now();
        let last_pong = *self.last_pong.read().await;

        match last_pong {
            Some(pong_time) => {
                let elapsed = now - pong_time;
                if elapsed > self.config.timeout + self.config.interval {
                    let missed = self.missed_count.fetch_add(1, Ordering::Relaxed) + 1;
                    missed < self.config.max_missed as u64
                } else {
                    true
                }
            }
            None => {
                // No pong received yet, check if we've been trying long enough
                let last_ping = *self.last_ping.read().await;
                match last_ping {
                    Some(ping_time) => {
                        let elapsed = now - ping_time;
                        elapsed < self.config.timeout + Duration::seconds(5) // Grace period
                    }
                    None => true, // Haven't started pinging yet
                }
            }
        }
    }
}

impl WebSocketResilienceManager {
    /// Create new WebSocket resilience manager
    pub fn new(url: String, config: WebSocketResilienceConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let heartbeat_manager = Arc::new(HeartbeatManager::new(config.heartbeat.clone()));

        Self {
            config,
            url,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            pending_messages: Arc::new(RwLock::new(HashMap::new())),
            outgoing_queue: Arc::new(Mutex::new(VecDeque::new())),
            message_history: Arc::new(RwLock::new(HashMap::new())),
            message_counter: Arc::new(AtomicU64::new(0)),
            connection_attempts: Arc::new(AtomicU64::new(0)),
            last_connected: Arc::new(RwLock::new(None)),
            heartbeat_manager,
            event_sender,
            message_sender: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(RwLock::new(ResilienceStatistics::default())),
        }
    }

    /// Start the resilience manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting WebSocket resilience manager");

        // Start background tasks
        self.start_connection_manager().await;
        self.start_message_processor().await;
        self.start_heartbeat_monitor().await;
        self.start_cleanup_task().await;

        Ok(())
    }

    /// Stop the resilience manager
    pub async fn stop(&self) {
        info!("Stopping WebSocket resilience manager");
        self.shutdown.store(true, Ordering::Relaxed);

        // Update state
        *self.state.write().await = ConnectionState::Disconnected;

        // Clear message sender
        *self.message_sender.write().await = None;

        // Clear pending messages
        let pending_messages = {
            let mut pending = self.pending_messages.write().await;
            let messages: Vec<_> = pending.drain().collect();
            messages
        };

        // Send failure events for pending messages
        for (message_id, _) in pending_messages {
            let _ = self.event_sender.send(ResilienceEvent::MessageFailed {
                message_id,
                error: "Connection stopped".to_string(),
            });
        }
    }

    /// Send a message with resilience features
    pub async fn send_message(
        &self,
        payload: String,
        message_type: MessageType,
        priority: MessagePriority,
    ) -> Result<String> {
        let message_id = self.generate_message_id();

        // Check for duplicates if enabled
        if self.config.enable_deduplication {
            let payload_hash = format!("{:x}", md5::compute(&payload));
            let mut history = self.message_history.write().await;

            if let Some(last_sent) = history.get(&payload_hash) {
                let time_since = Utc::now() - *last_sent;
                if time_since < Duration::seconds(60) {
                    // 1-minute dedup window
                    let mut stats = self.stats.write().await;
                    stats.duplicates_detected += 1;

                    warn!("Duplicate message detected and ignored: {}", payload_hash);
                    return Ok(message_id);
                }
            }

            history.insert(payload_hash, Utc::now());

            // Cleanup old history entries
            if history.len() > self.config.dedup_history_size {
                let cutoff = Utc::now() - Duration::hours(1);
                history.retain(|_, timestamp| *timestamp > cutoff);
            }
        }

        let message = ResilientMessage {
            id: message_id.clone(),
            payload,
            message_type: message_type.clone(),
            created_at: Utc::now(),
            retry_count: 0,
            last_attempt: Utc::now(),
            priority,
            requires_ack: message_type != MessageType::Heartbeat,
            expires_at: None,
        };

        // Add to outgoing queue
        {
            let mut queue = self.outgoing_queue.lock().await;

            // Check queue size limit
            if queue.len() >= self.config.max_pending_messages {
                // Remove oldest low-priority message to make room
                if let Some(pos) = queue
                    .iter()
                    .position(|m| m.priority == MessagePriority::Low)
                {
                    let dropped = queue.remove(pos).unwrap();
                    let _ = self.event_sender.send(ResilienceEvent::QueueOverflow {
                        dropped_messages: 1,
                    });
                    warn!(
                        "Dropped low-priority message due to queue overflow: {}",
                        dropped.id
                    );
                } else {
                    return Err(LoxoneError::resource_exhausted("Message queue is full"));
                }
            }

            // Insert message based on priority
            let insert_pos = queue
                .iter()
                .position(|m| m.priority < message.priority)
                .unwrap_or(queue.len());
            queue.insert(insert_pos, message);
        }

        let _ = self.event_sender.send(ResilienceEvent::MessageSent {
            message_id: message_id.clone(),
            message_type,
        });

        Ok(message_id)
    }

    /// Acknowledge a received message
    pub async fn acknowledge_message(&self, message_id: &str) -> Result<()> {
        let mut pending = self.pending_messages.write().await;

        if let Some(message) = pending.remove(message_id) {
            let response_time = Utc::now() - message.created_at;

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.messages_acknowledged += 1;

                // Update average acknowledgment time
                let total_ack_time =
                    stats.avg_ack_time_ms * (stats.messages_acknowledged - 1) as f64;
                stats.avg_ack_time_ms = (total_ack_time + response_time.num_milliseconds() as f64)
                    / stats.messages_acknowledged as f64;
            }

            let _ = self
                .event_sender
                .send(ResilienceEvent::MessageAcknowledged {
                    message_id: message_id.to_string(),
                    response_time,
                });

            debug!(
                "Message acknowledged: {} (response time: {}ms)",
                message_id,
                response_time.num_milliseconds()
            );
        }

        Ok(())
    }

    /// Get current connection state
    pub async fn get_state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get current statistics
    pub async fn get_statistics(&self) -> ResilienceStatistics {
        let mut stats = self.stats.read().await.clone();
        stats.pending_messages = self.pending_messages.read().await.len();
        stats.queue_size = self.outgoing_queue.lock().await.len();
        stats
    }

    /// Subscribe to resilience events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ResilienceEvent> {
        self.event_sender.subscribe()
    }

    /// Generate unique message ID
    fn generate_message_id(&self) -> String {
        let counter = self.message_counter.fetch_add(1, Ordering::Relaxed);
        format!(
            "msg-{}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0),
            counter
        )
    }

    /// Start connection manager task
    async fn start_connection_manager(&self) {
        let state = self.state.clone();
        let url = self.url.clone();
        let config = self.config.clone();
        let connection_attempts = self.connection_attempts.clone();
        let last_connected = self.last_connected.clone();
        let event_sender = self.event_sender.clone();
        let message_sender = self.message_sender.clone();
        let shutdown = self.shutdown.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            while !shutdown.load(Ordering::Relaxed) {
                let current_state = *state.read().await;

                match current_state {
                    ConnectionState::Disconnected => {
                        if config.reconnection.enabled {
                            *state.write().await = ConnectionState::Connecting;

                            let attempt = connection_attempts.fetch_add(1, Ordering::Relaxed) + 1;
                            let start_time = Instant::now();

                            info!("Attempting WebSocket connection (attempt {})", attempt);

                            // Simulate WebSocket connection (in real implementation, use actual WebSocket library)
                            #[cfg(feature = "websocket")]
                            match Self::establish_connection(&url).await {
                                Ok(sender) => {
                                    *message_sender.write().await = Some(sender);
                                    *state.write().await = ConnectionState::Connected;
                                    *last_connected.write().await = Some(Utc::now());

                                    let duration = Duration::milliseconds(
                                        start_time.elapsed().as_millis() as i64,
                                    );
                                    let _ = event_sender
                                        .send(ResilienceEvent::Connected { attempt, duration });

                                    {
                                        let mut stats = stats.write().await;
                                        stats.successful_connections += 1;
                                        stats.connection_attempts = attempt;
                                    }

                                    info!("WebSocket connected successfully");
                                }
                                Err(e) => {
                                    error!("WebSocket connection failed: {}", e);
                                    *state.write().await = ConnectionState::Reconnecting;
                                }
                            }

                            #[cfg(not(feature = "websocket"))]
                            {
                                // Mock successful connection for testing
                                let (sender, _) = mpsc::unbounded_channel();
                                *message_sender.write().await = Some(sender);
                                *state.write().await = ConnectionState::Connected;
                                *last_connected.write().await = Some(Utc::now());

                                let duration =
                                    Duration::milliseconds(start_time.elapsed().as_millis() as i64);
                                let _ = event_sender
                                    .send(ResilienceEvent::Connected { attempt, duration });

                                {
                                    let mut stats = stats.write().await;
                                    stats.successful_connections += 1;
                                    stats.connection_attempts = attempt;
                                }

                                info!("WebSocket connected successfully (mock)");
                            }
                        }
                    }
                    ConnectionState::Reconnecting => {
                        let attempt = connection_attempts.load(Ordering::Relaxed);
                        let delay =
                            Self::calculate_reconnection_delay(&config.reconnection, attempt);

                        let _ = event_sender
                            .send(ResilienceEvent::ReconnectionStarted { attempt, delay });

                        info!("Reconnecting in {:?} (attempt {})", delay, attempt);
                        sleep(TokioDuration::from_millis(delay.num_milliseconds() as u64)).await;

                        *state.write().await = ConnectionState::Disconnected;
                    }
                    _ => {
                        sleep(TokioDuration::from_millis(1000)).await;
                    }
                }

                sleep(TokioDuration::from_millis(100)).await;
            }
        });
    }

    /// Start message processor task
    async fn start_message_processor(&self) {
        let outgoing_queue = self.outgoing_queue.clone();
        let pending_messages = self.pending_messages.clone();
        let message_sender = self.message_sender.clone();
        let config = self.config.clone();
        let event_sender = self.event_sender.clone();
        let shutdown = self.shutdown.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_millis(100));

            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;

                // Process outgoing messages
                let message = {
                    let mut queue = outgoing_queue.lock().await;
                    queue.pop_front()
                };

                if let Some(mut message) = message {
                    let sender = message_sender.read().await.clone();

                    if let Some(sender) = sender {
                        message.last_attempt = Utc::now();

                        // Send message (in real implementation, this would send via WebSocket)
                        match sender.send(message.clone()) {
                            Ok(()) => {
                                // Add to pending if acknowledgment required
                                if message.requires_ack {
                                    pending_messages
                                        .write()
                                        .await
                                        .insert(message.id.clone(), message);
                                }

                                let mut stats = stats.write().await;
                                stats.messages_sent += 1;
                            }
                            Err(_) => {
                                // Connection broken, re-queue message if retries available
                                if message.retry_count < config.retry_config.max_retries {
                                    message.retry_count += 1;
                                    let _delay = if config.retry_config.exponential_backoff {
                                        config.retry_config.retry_delay
                                            * (2_u32.pow(message.retry_count)) as i32
                                    } else {
                                        config.retry_config.retry_delay
                                    };

                                    // Re-add to queue with delay (simplified)
                                    let mut queue = outgoing_queue.lock().await;
                                    queue.push_back(message);
                                    debug!("Message re-queued for retry with delay: {:?}", _delay);
                                } else {
                                    let _ = event_sender.send(ResilienceEvent::MessageFailed {
                                        message_id: message.id,
                                        error: "Max retries exceeded".to_string(),
                                    });

                                    let mut stats = stats.write().await;
                                    stats.messages_failed += 1;
                                }
                            }
                        }
                    } else {
                        // No connection, re-queue message
                        let mut queue = outgoing_queue.lock().await;
                        queue.push_front(message);
                    }
                }
            }
        });
    }

    /// Start heartbeat monitor task
    async fn start_heartbeat_monitor(&self) {
        let heartbeat_manager = self.heartbeat_manager.clone();
        let outgoing_queue = self.outgoing_queue.clone();
        let event_sender = self.event_sender.clone();
        let shutdown = self.shutdown.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_millis(
                heartbeat_manager.config.interval.num_milliseconds() as u64,
            ));

            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;

                let current_state = *state.read().await;
                if current_state == ConnectionState::Connected {
                    // Send heartbeat
                    let ping_message = heartbeat_manager.send_ping().await;
                    let mut queue = outgoing_queue.lock().await;
                    queue.push_front(ping_message); // High priority

                    // Check connection health
                    if !heartbeat_manager.check_health().await {
                        let missed_count = heartbeat_manager.missed_count.load(Ordering::Relaxed);
                        let _ = event_sender.send(ResilienceEvent::HeartbeatMissed {
                            count: missed_count,
                        });

                        warn!("Connection unhealthy: {} missed heartbeats", missed_count);

                        // Mark connection as failed
                        *state.write().await = ConnectionState::Reconnecting;
                    }
                }
            }
        });
    }

    /// Start cleanup task for expired messages
    async fn start_cleanup_task(&self) {
        let pending_messages = self.pending_messages.clone();
        let config = self.config.clone();
        let event_sender = self.event_sender.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(30));

            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;

                let now = Utc::now();
                let mut expired_messages = Vec::new();

                {
                    let mut pending = pending_messages.write().await;
                    pending.retain(|id, message| {
                        let is_expired = if let Some(expires_at) = message.expires_at {
                            now > expires_at
                        } else {
                            (now - message.created_at) > config.ack_timeout
                        };

                        if is_expired {
                            expired_messages.push((id.clone(), message.clone()));
                            false
                        } else {
                            true
                        }
                    });
                }

                // Send timeout events for expired messages
                for (message_id, message) in expired_messages {
                    let _ = event_sender.send(ResilienceEvent::MessageTimeout {
                        message_id,
                        retry_count: message.retry_count,
                    });
                }
            }
        });
    }

    /// Calculate reconnection delay with exponential backoff and jitter
    fn calculate_reconnection_delay(config: &ReconnectionConfig, attempt: u64) -> Duration {
        let base_delay = if attempt == 1 {
            config.initial_delay
        } else {
            let delay_ms = config.initial_delay.num_milliseconds() as f64
                * config.backoff_multiplier.powi((attempt - 1) as i32);
            let capped_delay = delay_ms.min(config.max_delay.num_milliseconds() as f64);
            Duration::milliseconds(capped_delay as i64)
        };

        // Add jitter
        let jitter = config.jitter_factor * base_delay.num_milliseconds() as f64;
        let jitter_offset = (rand::random::<f64>() - 0.5) * 2.0 * jitter;
        let final_delay = base_delay.num_milliseconds() as f64 + jitter_offset;

        Duration::milliseconds(final_delay.max(0.0) as i64)
    }

    /// Establish WebSocket connection
    #[cfg(feature = "websocket")]
    async fn establish_connection(url: &str) -> Result<mpsc::UnboundedSender<ResilientMessage>> {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        // Establish actual WebSocket connection
        let (ws_stream, _) = connect_async(url).await.map_err(|e| {
            crate::error::LoxoneError::connection(format!("WebSocket connection failed: {e}"))
        })?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (sender, mut receiver) = mpsc::unbounded_channel::<ResilientMessage>();

        // Spawn task to handle sending messages
        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                if let Ok(text) = serde_json::to_string(&msg) {
                    if let Err(e) = ws_sender.send(Message::Text(text)).await {
                        tracing::error!("Failed to send WebSocket message: {}", e);
                        break;
                    }
                }
            }
        });

        // Spawn task to handle receiving messages (for future use)
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        tracing::debug!("Received WebSocket message: {}", text);

                        // Handle Loxone WebSocket protocol messages
                        if let Ok(loxone_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            // Check for Loxone message types
                            if let Some(msg_type) = loxone_msg.get("LL") {
                                match msg_type.as_i64() {
                                    Some(200) => {
                                        // Success response - update statistics
                                        tracing::debug!("Loxone success response received");
                                    }
                                    Some(code) if code >= 400 => {
                                        // Error response - log and potentially trigger reconnection
                                        tracing::warn!("Loxone error response: {}", code);
                                    }
                                    _ => {
                                        tracing::debug!(
                                            "Unknown Loxone response code: {:?}",
                                            msg_type
                                        );
                                    }
                                }
                            } else if loxone_msg.get("keepalive").is_some() {
                                // Keepalive message - respond with pong
                                tracing::debug!("Received Loxone keepalive");
                            } else {
                                // State update or other message
                                tracing::debug!("Received Loxone state update or other message");
                            }
                        } else {
                            tracing::debug!("Received non-JSON WebSocket message: {}", text);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        tracing::info!("WebSocket connection closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resilience_manager_creation() {
        let config = WebSocketResilienceConfig::default();
        let manager = WebSocketResilienceManager::new("ws://localhost:8080".to_string(), config);

        assert_eq!(manager.get_state().await, ConnectionState::Disconnected);
        assert_eq!(manager.message_counter.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_message_queuing() {
        let config = WebSocketResilienceConfig::default();
        let manager = WebSocketResilienceManager::new("ws://localhost:8080".to_string(), config);

        let message_id = manager
            .send_message(
                "test message".to_string(),
                MessageType::Command,
                MessagePriority::Normal,
            )
            .await
            .unwrap();

        assert!(!message_id.is_empty());
        assert_eq!(manager.outgoing_queue.lock().await.len(), 1);
    }

    #[test]
    fn test_reconnection_delay_calculation() {
        let config = ReconnectionConfig {
            initial_delay: Duration::seconds(1),
            max_delay: Duration::seconds(60),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            ..Default::default()
        };

        let delay1 = WebSocketResilienceManager::calculate_reconnection_delay(&config, 1);
        let delay2 = WebSocketResilienceManager::calculate_reconnection_delay(&config, 2);
        let delay3 = WebSocketResilienceManager::calculate_reconnection_delay(&config, 3);

        assert!(delay1 >= Duration::seconds(1) - Duration::milliseconds(100));
        assert!(delay1 <= Duration::seconds(1) + Duration::milliseconds(100));
        assert!(delay2.num_milliseconds() >= 1800); // 2s - 10% jitter
        assert!(delay2.num_milliseconds() <= 2200); // 2s + 10% jitter
        assert!(delay3.num_milliseconds() >= 3600); // 4s - 10% jitter
        assert!(delay3.num_milliseconds() <= 4400); // 4s + 10% jitter
    }
}
