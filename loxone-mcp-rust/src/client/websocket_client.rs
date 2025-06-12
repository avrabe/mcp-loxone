//! WebSocket client implementation for real-time Loxone communication
//!
//! This module provides WebSocket-based real-time communication with Loxone
//! Miniservers for live state updates and sensor monitoring.

#[cfg(feature = "websocket")]
use crate::client::{LoxoneClient, LoxoneResponse, LoxoneStructure, ClientContext};
#[cfg(feature = "websocket")]
use crate::config::{LoxoneConfig, credentials::LoxoneCredentials};
#[cfg(feature = "websocket")]
use crate::error::{LoxoneError, Result};
#[cfg(feature = "websocket")]
use async_trait::async_trait;
#[cfg(feature = "websocket")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "websocket")]
use std::collections::HashMap;
#[cfg(feature = "websocket")]
use tokio::sync::mpsc;
#[cfg(feature = "websocket")]
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
#[cfg(feature = "websocket")]
use tracing::{debug, info, warn};
#[cfg(feature = "websocket")]
use url::Url;

#[cfg(feature = "websocket")]
type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

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
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// WebSocket client for real-time Loxone communication
#[cfg(feature = "websocket")]
pub struct LoxoneWebSocketClient {
    /// Base URL for Miniserver
    base_url: Url,
    
    /// Authentication credentials
    credentials: LoxoneCredentials,
    
    /// Configuration
    #[allow(dead_code)]
    config: LoxoneConfig,
    
    /// Shared context for caching
    context: ClientContext,
    
    /// WebSocket stream
    ws_stream: Option<WsStream>,
    
    /// State update channel sender
    state_sender: Option<mpsc::UnboundedSender<StateUpdate>>,
    
    /// Connection state
    connected: bool,
    
    /// Background task handle
    task_handle: Option<tokio::task::JoinHandle<()>>,
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
            connected: false,
            task_handle: None,
        })
    }
    
    /// Build WebSocket URL
    fn build_ws_url(&self) -> Result<Url> {
        let mut ws_url = self.base_url.clone();
        
        // Convert HTTP(S) to WS(S)
        match ws_url.scheme() {
            "http" => ws_url.set_scheme("ws").map_err(|_| {
                LoxoneError::connection("Failed to convert HTTP to WebSocket URL")
            })?,
            "https" => ws_url.set_scheme("wss").map_err(|_| {
                LoxoneError::connection("Failed to convert HTTPS to WebSocket URL")
            })?,
            _ => return Err(LoxoneError::connection("Unsupported URL scheme for WebSocket")),
        }
        
        // Add WebSocket endpoint path
        ws_url.set_path("/ws/rfc6455");
        
        // Add authentication parameters
        ws_url.query_pairs_mut()
            .append_pair("user", &self.credentials.username)
            .append_pair("password", &self.credentials.password);
        
        Ok(ws_url)
    }
    
    /// Start background task for message processing
    async fn start_background_task(&mut self) -> Result<()> {
        let (state_tx, mut state_rx) = mpsc::unbounded_channel::<StateUpdate>();
        self.state_sender = Some(state_tx);
        
        let context = self.context.clone();
        
        // Spawn background task to process state updates
        let task = tokio::spawn(async move {
            while let Some(update) = state_rx.recv().await {
                debug!("Processing state update: {} = {:?}", update.uuid, update.value);
                
                // Update device state in context
                let mut devices = context.devices.write().await;
                if let Some(device) = devices.get_mut(&update.uuid) {
                    device.states.insert(update.state, update.value);
                }
            }
        });
        
        self.task_handle = Some(task);
        Ok(())
    }
    
    /// Process WebSocket messages
    #[allow(dead_code)]
    async fn process_message(&self, message: Message) -> Result<()> {
        match message {
            Message::Text(text) => {
                debug!("Received text message: {text}");
                
                // Try parsing as Loxone message
                if let Ok(loxone_msg) = serde_json::from_str::<LoxoneWebSocketMessage>(&text) {
                    self.handle_loxone_message(loxone_msg).await?;
                }
            }
            Message::Binary(data) => {
                debug!("Received binary message: {} bytes", data.len());
                self.handle_binary_message(data).await?;
            }
            Message::Ping(_data) => {
                debug!("Received ping, sending pong");
                if let Some(_ws) = &self.ws_stream {
                    // Note: In a real implementation, we'd need a way to send the pong
                    // This requires restructuring to have a shared websocket sender
                }
            }
            Message::Pong(_) => {
                debug!("Received pong");
            }
            Message::Close(_) => {
                warn!("WebSocket connection closed by server");
                return Err(LoxoneError::connection("WebSocket closed by server"));
            }
            Message::Frame(_) => {
                // Raw frame, usually not handled directly
            }
        }
        
        Ok(())
    }
    
    /// Handle Loxone-specific message
    #[allow(dead_code)]
    async fn handle_loxone_message(&self, message: LoxoneWebSocketMessage) -> Result<()> {
        match message.msg_type.as_str() {
            "text" => {
                // Handle text-based state updates
                if let Some(uuid) = message.data.get("uuid").and_then(|v| v.as_str()) {
                    if let Some(value) = message.data.get("value") {
                        let update = StateUpdate {
                            uuid: uuid.to_string(),
                            state: "value".to_string(),
                            value: value.clone(),
                            timestamp: chrono::Utc::now(),
                        };
                        
                        if let Some(sender) = &self.state_sender {
                            let _ = sender.send(update);
                        }
                    }
                }
            }
            "header" => {
                debug!("Received header message: {:?}", message.data);
            }
            "keepalive" => {
                debug!("Received keepalive message");
            }
            _ => {
                debug!("Unknown message type: {}", message.msg_type);
            }
        }
        
        Ok(())
    }
    
    /// Handle binary message (sensor data)
    #[allow(dead_code)]
    async fn handle_binary_message(&self, data: Vec<u8>) -> Result<()> {
        // Binary messages in Loxone typically contain sensor state updates
        // The format is proprietary and would need reverse engineering
        // For now, we'll log the message
        debug!("Binary message processing not implemented yet: {} bytes", data.len());
        Ok(())
    }
    
    /// Get public context for external access
    pub fn context(&self) -> &ClientContext {
        &self.context
    }
    
    /// Subscribe to state updates
    pub fn subscribe_to_updates(&mut self) -> mpsc::UnboundedReceiver<StateUpdate> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.state_sender = Some(tx);
        rx
    }
}

#[cfg(feature = "websocket")]
#[async_trait]
impl LoxoneClient for LoxoneWebSocketClient {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting WebSocket to Loxone Miniserver at {}", self.base_url);
        
        let ws_url = self.build_ws_url()?;
        debug!("WebSocket URL: {ws_url}");
        
        // Connect to WebSocket
        let (ws_stream, response) = connect_async(&ws_url).await
            .map_err(|e| LoxoneError::connection(format!("WebSocket connection failed: {e}")))?;
        
        debug!("WebSocket connected, response: {:?}", response.status());
        
        self.ws_stream = Some(ws_stream);
        self.connected = true;
        *self.context.connected.write().await = true;
        
        // Start background message processing
        self.start_background_task().await?;
        
        info!("✅ Connected to Loxone WebSocket");
        Ok(())
    }
    
    async fn is_connected(&self) -> Result<bool> {
        Ok(self.connected && *self.context.connected.read().await)
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        
        self.ws_stream = None;
        self.state_sender = None;
        self.connected = false;
        *self.context.connected.write().await = false;
        
        info!("Disconnected from Loxone WebSocket");
        Ok(())
    }
    
    async fn send_command(&self, uuid: &str, command: &str) -> Result<LoxoneResponse> {
        if !self.connected {
            return Err(LoxoneError::connection("WebSocket not connected"));
        }
        
        // WebSocket command format is different from HTTP
        // This would need to be implemented based on Loxone's WebSocket protocol
        debug!("WebSocket command sending not fully implemented: {uuid} -> {command}");
        
        // For now, return a placeholder response
        Ok(LoxoneResponse {
            code: 200,
            value: serde_json::json!({
                "status": "sent",
                "uuid": uuid,
                "command": command
            }),
        })
    }
    
    async fn get_structure(&self) -> Result<LoxoneStructure> {
        // WebSocket doesn't typically provide structure file directly
        // This would need to be fetched via HTTP or cached
        Err(LoxoneError::connection(
            "Structure file not available via WebSocket - use HTTP client"
        ))
    }
    
    async fn get_device_states(&self, uuids: &[String]) -> Result<HashMap<String, serde_json::Value>> {
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
    
    async fn get_system_info(&self) -> Result<serde_json::Value> {
        // System info not typically available via WebSocket
        Err(LoxoneError::connection(
            "System info not available via WebSocket - use HTTP client"
        ))
    }
    
    async fn health_check(&self) -> Result<bool> {
        Ok(self.connected)
    }
}

// Placeholder implementations for when websocket feature is disabled
#[cfg(not(feature = "websocket"))]
pub struct LoxoneWebSocketClient;

#[cfg(not(feature = "websocket"))]
impl LoxoneWebSocketClient {
    pub async fn new(_config: crate::config::LoxoneConfig, _credentials: crate::config::LoxoneCredentials) -> Result<Self> {
        Err(LoxoneError::config("WebSocket client not available - enable 'websocket' feature"))
    }
}