//! Common test utilities and fixtures
//!
//! This module provides shared test utilities, mock implementations,
//! and test fixtures for the Loxone MCP server test suite.

use loxone_mcp_rust::{
    client::{LoxoneClient, LoxoneDevice, LoxoneResponse, LoxoneStructure, ClientContext},
    config::{
        ServerConfig, LoxoneConfig, McpConfig, TransportConfig, ToolConfig, 
        LoggingConfig, FeatureConfig, CredentialStore, WebSocketConfig,
        credentials::LoxoneCredentials
    },
    error::{LoxoneError, Result},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
// use url::Url; // Unused import

/// Mock Loxone client for testing
pub struct MockLoxoneClient {
    /// Whether the client is connected
    pub connected: Arc<RwLock<bool>>,
    
    /// Mock devices to return
    pub devices: Arc<RwLock<HashMap<String, LoxoneDevice>>>,
    
    /// Mock structure data
    pub structure: Arc<RwLock<Option<LoxoneStructure>>>,
    
    /// Command history for verification
    pub command_history: Arc<RwLock<Vec<(String, String)>>>,
    
    /// Response overrides for specific commands
    pub response_overrides: Arc<RwLock<HashMap<String, LoxoneResponse>>>,
    
    /// Simulate connection failures
    pub simulate_failures: Arc<RwLock<bool>>,
}

impl MockLoxoneClient {
    /// Create new mock client
    pub fn new() -> Self {
        Self {
            connected: Arc::new(RwLock::new(false)),
            devices: Arc::new(RwLock::new(HashMap::new())),
            structure: Arc::new(RwLock::new(None)),
            command_history: Arc::new(RwLock::new(Vec::new())),
            response_overrides: Arc::new(RwLock::new(HashMap::new())),
            simulate_failures: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Add mock device
    #[allow(dead_code)]
    pub async fn add_device(&self, device: LoxoneDevice) {
        self.devices.write().await.insert(device.uuid.clone(), device);
    }
    
    /// Set mock structure
    #[allow(dead_code)]
    pub async fn set_structure(&self, structure: LoxoneStructure) {
        *self.structure.write().await = Some(structure);
    }
    
    /// Set command response override
    pub async fn set_response_override(&self, command: String, response: LoxoneResponse) {
        self.response_overrides.write().await.insert(command, response);
    }
    
    /// Enable/disable failure simulation
    #[allow(dead_code)]
    pub async fn simulate_failures(&self, enabled: bool) {
        *self.simulate_failures.write().await = enabled;
    }
    
    /// Get command history
    pub async fn get_command_history(&self) -> Vec<(String, String)> {
        self.command_history.read().await.clone()
    }
    
    /// Clear command history
    #[allow(dead_code)]
    pub async fn clear_command_history(&self) {
        self.command_history.write().await.clear();
    }
}

#[async_trait]
impl LoxoneClient for MockLoxoneClient {
    async fn connect(&mut self) -> Result<()> {
        if *self.simulate_failures.read().await {
            return Err(LoxoneError::connection("Simulated connection failure"));
        }
        
        *self.connected.write().await = true;
        Ok(())
    }
    
    async fn is_connected(&self) -> Result<bool> {
        Ok(*self.connected.read().await)
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        Ok(())
    }
    
    async fn send_command(&self, uuid: &str, command: &str) -> Result<LoxoneResponse> {
        if !*self.connected.read().await {
            return Err(LoxoneError::connection("Not connected"));
        }
        
        if *self.simulate_failures.read().await {
            return Err(LoxoneError::device_control("Simulated command failure"));
        }
        
        // Record command
        self.command_history.write().await.push((uuid.to_string(), command.to_string()));
        
        // Check for response override
        let override_key = format!("{}:{}", uuid, command);
        if let Some(response) = self.response_overrides.read().await.get(&override_key) {
            return Ok(response.clone());
        }
        
        // Default successful response
        Ok(LoxoneResponse {
            code: 200,
            value: serde_json::json!({
                "uuid": uuid,
                "command": command,
                "status": "success"
            }),
        })
    }
    
    async fn get_structure(&self) -> Result<LoxoneStructure> {
        if !*self.connected.read().await {
            return Err(LoxoneError::connection("Not connected"));
        }
        
        match &*self.structure.read().await {
            Some(structure) => Ok(structure.clone()),
            None => Err(LoxoneError::not_found("No structure data available")),
        }
    }
    
    async fn get_device_states(&self, uuids: &[String]) -> Result<HashMap<String, serde_json::Value>> {
        if !*self.connected.read().await {
            return Err(LoxoneError::connection("Not connected"));
        }
        
        let devices = self.devices.read().await;
        let mut states = HashMap::new();
        
        for uuid in uuids {
            if let Some(device) = devices.get(uuid) {
                for (state_name, value) in &device.states {
                    states.insert(format!("{}:{}", uuid, state_name), value.clone());
                }
            }
        }
        
        Ok(states)
    }
    
    async fn get_system_info(&self) -> Result<serde_json::Value> {
        if !*self.connected.read().await {
            return Err(LoxoneError::connection("Not connected"));
        }
        
        Ok(serde_json::json!({
            "version": "12.3.4.5",
            "serial": "TEST-SERIAL-123",
            "type": "Test Miniserver"
        }))
    }
    
    async fn health_check(&self) -> Result<bool> {
        Ok(*self.connected.read().await && !*self.simulate_failures.read().await)
    }
}

/// Create test server configuration
#[allow(dead_code)]
pub fn create_test_config() -> ServerConfig {
    ServerConfig {
        loxone: LoxoneConfig {
            url: "http://test.loxone.local".parse().unwrap(),
            username: "test_user".to_string(),
            timeout: std::time::Duration::from_secs(5),
            max_retries: 3,
            verify_ssl: false,
            #[cfg(feature = "websocket")]
            websocket: WebSocketConfig {
                enable_monitoring: false,
                discovery_duration: std::time::Duration::from_secs(10),
                keepalive_interval: std::time::Duration::from_secs(30),
            },
        },
        mcp: McpConfig {
            name: "Test Loxone Server".to_string(),
            version: "0.1.0-test".to_string(),
            transport: TransportConfig {
                transport_type: "stdio".to_string(),
                port: None,
                host: None,
            },
            tools: ToolConfig {
                enable_rooms: true,
                enable_devices: true,
                enable_sensors: true,
                enable_climate: true,
                enable_weather: false,
                max_devices_per_query: 50,
            },
        },
        credentials: CredentialStore::Environment,
        logging: LoggingConfig {
            level: "debug".to_string(),
            json_format: false,
            file: None,
        },
        features: FeatureConfig {
            enable_crypto: false,
            enable_websocket: false,
            enable_caching: true,
            cache_ttl: std::time::Duration::from_secs(30),
        },
    }
}

/// Create test credentials
#[allow(dead_code)]
pub fn create_test_credentials() -> LoxoneCredentials {
    LoxoneCredentials {
        username: "test_user".to_string(),
        password: "test_password".to_string(),
        api_key: Some("test_api_key".to_string()),
        #[cfg(feature = "crypto")]
        public_key: None,
    }
}

/// Create sample test device
pub fn create_test_device(uuid: &str, name: &str, device_type: &str, room: Option<&str>) -> LoxoneDevice {
    let mut states = HashMap::new();
    states.insert("value".to_string(), serde_json::json!(0));
    states.insert("active".to_string(), serde_json::json!(false));
    
    LoxoneDevice {
        uuid: uuid.to_string(),
        name: name.to_string(),
        device_type: device_type.to_string(),
        room: room.map(|r| r.to_string()),
        states,
        category: categorize_device(device_type),
        sub_controls: HashMap::new(),
    }
}

/// Create sample test structure
pub fn create_test_structure() -> LoxoneStructure {
    let mut controls = HashMap::new();
    let mut rooms = HashMap::new();
    let mut cats = HashMap::new();
    
    // Add test rooms
    rooms.insert("room-1".to_string(), serde_json::json!({
        "name": "Living Room",
        "uuid": "room-1"
    }));
    rooms.insert("room-2".to_string(), serde_json::json!({
        "name": "Kitchen",
        "uuid": "room-2"
    }));
    
    // Add test devices
    controls.insert("light-1".to_string(), serde_json::json!({
        "name": "Living Room Light",
        "type": "LightController",
        "room": "room-1",
        "states": {
            "active": 0,
            "value": 0
        }
    }));
    
    controls.insert("blind-1".to_string(), serde_json::json!({
        "name": "Kitchen Blind",
        "type": "Jalousie",
        "room": "room-2",
        "states": {
            "position": 0,
            "shade": 0
        }
    }));
    
    controls.insert("temp-1".to_string(), serde_json::json!({
        "name": "Living Room Temperature",
        "type": "IRoomControllerV2",
        "room": "room-1",
        "states": {
            "tempActual": 21.5,
            "tempTarget": 22.0
        }
    }));
    
    // Add test categories
    cats.insert("lighting".to_string(), serde_json::json!({
        "name": "Lighting",
        "type": "lighting"
    }));
    
    LoxoneStructure {
        last_modified: "2025-01-06T12:00:00Z".to_string(),
        controls,
        rooms,
        cats,
        global_states: HashMap::new(),
    }
}

/// Helper function to categorize devices for testing
fn categorize_device(device_type: &str) -> String {
    match device_type.to_lowercase().as_str() {
        t if t.contains("light") || t.contains("dimmer") => "lighting".to_string(),
        t if t.contains("jalousie") || t.contains("blind") => "blinds".to_string(),
        t if t.contains("controller") || t.contains("climate") => "climate".to_string(),
        t if t.contains("sensor") => "sensors".to_string(),
        _ => "other".to_string(),
    }
}

/// Test assertion helpers
pub mod assertions {
    use super::*;
    
    /// Assert that a command was sent
    #[allow(dead_code)]
    pub async fn assert_command_sent(client: &MockLoxoneClient, uuid: &str, command: &str) {
        let history = client.get_command_history().await;
        assert!(
            history.iter().any(|(u, c)| u == uuid && c == command),
            "Expected command '{}' to device '{}' was not sent. History: {:?}",
            command, uuid, history
        );
    }
    
    /// Assert that no commands were sent
    #[allow(dead_code)]
    pub async fn assert_no_commands_sent(client: &MockLoxoneClient) {
        let history = client.get_command_history().await;
        assert!(
            history.is_empty(),
            "Expected no commands to be sent, but found: {:?}",
            history
        );
    }
    
    /// Assert command count
    #[allow(dead_code)]
    pub async fn assert_command_count(client: &MockLoxoneClient, expected: usize) {
        let history = client.get_command_history().await;
        assert_eq!(
            history.len(),
            expected,
            "Expected {} commands, but found {}. History: {:?}",
            expected, history.len(), history
        );
    }
}

/// Async test helper macro
#[macro_export]
macro_rules! async_test {
    ($test_name:ident, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            // Initialize test logging
            let _ = tracing_subscriber::fmt()
                .with_env_filter("debug")
                .with_test_writer()
                .try_init();
            
            $test_body
        }
    };
}

/// Integration test setup
#[allow(dead_code)]
pub async fn setup_integration_test() -> (MockLoxoneClient, ClientContext) {
    let mock_client = MockLoxoneClient::new();
    let context = ClientContext::new();
    
    // Setup test data
    let structure = create_test_structure();
    mock_client.set_structure(structure.clone()).await;
    context.update_structure(structure).await.unwrap();
    
    // Add test devices
    let devices = vec![
        create_test_device("light-1", "Living Room Light", "LightController", Some("Living Room")),
        create_test_device("blind-1", "Kitchen Blind", "Jalousie", Some("Kitchen")),
        create_test_device("temp-1", "Living Room Temperature", "IRoomControllerV2", Some("Living Room")),
    ];
    
    for device in devices {
        mock_client.add_device(device).await;
    }
    
    // Connect the mock client
    let mut client_mut = mock_client;
    client_mut.connect().await.unwrap();
    
    (client_mut, context)
}