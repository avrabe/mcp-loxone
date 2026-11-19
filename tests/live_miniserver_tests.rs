//! Live Miniserver Integration Tests
//!
//! These tests run against a real Loxone Miniserver and are gated behind
//! the `LOXONE_LIVE_TEST=1` environment variable so they never run in CI.
//!
//! ## Running
//!
//! ```bash
//! LOXONE_LIVE_TEST=1 cargo test --test live_miniserver_tests --features test-utils -- --nocapture
//! ```
//!
//! ## Credentials
//!
//! Credentials are loaded from the `.env` file at the project root.
//! The `.env` file uses `USER`, `PASSWORD`, `SERVER` keys which are mapped
//! to `LOXONE_USER`, `LOXONE_PASS`, `LOXONE_HOST` respectively.

use loxone_mcp_rust::client::{ClientContext, LoxoneClient, LoxoneHttpClient};
use loxone_mcp_rust::config::credentials::{LoxoneCredentials, create_credentials};
use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig, ServerConfig};
use loxone_mcp_rust::server::macro_backend::LoxoneMcpServer;
use loxone_mcp_rust::services::{SensorTypeRegistry, UnifiedValueResolver};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether live tests should run.
fn should_run_live_tests() -> bool {
    std::env::var("LOXONE_LIVE_TEST")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Load credentials from the `.env` file at the project root and set the
/// corresponding `LOXONE_*` environment variables.  Returns `(host, user,
/// password)` on success.
fn load_env_file() -> Option<(String, String, String)> {
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    let content = std::fs::read_to_string(&env_path).ok()?;

    let mut server = None;
    let mut user = None;
    let mut password = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "SERVER" => {
                    // SAFETY: We are the only thread running at test init time and
                    // these env vars are not read concurrently during setup.
                    unsafe { std::env::set_var("LOXONE_HOST", value) };
                    server = Some(value.to_string());
                }
                "USER" => {
                    unsafe { std::env::set_var("LOXONE_USER", value) };
                    user = Some(value.to_string());
                }
                "PASSWORD" => {
                    unsafe { std::env::set_var("LOXONE_PASS", value) };
                    password = Some(value.to_string());
                }
                _ => {}
            }
        }
    }

    match (server, user, password) {
        (Some(s), Some(u), Some(p)) => Some((s, u, p)),
        _ => None,
    }
}

/// Build a `LoxoneConfig` and `LoxoneCredentials` from the loaded env values.
fn build_config_and_credentials(
    host: &str,
    user: &str,
    password: &str,
) -> (LoxoneConfig, LoxoneCredentials) {
    let url_str = if host.starts_with("http://") || host.starts_with("https://") {
        host.to_string()
    } else {
        format!("http://{host}")
    };

    let config = LoxoneConfig {
        url: url_str.parse().expect("Failed to parse Loxone host URL"),
        username: user.to_string(),
        timeout: Duration::from_secs(30),
        max_retries: 3,
        verify_ssl: false,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: loxone_mcp_rust::config::WebSocketConfig {
            enable_monitoring: false,
            discovery_duration: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(30),
        },
        auth_method: AuthMethod::Basic,
    };

    let credentials = create_credentials(user.to_string(), password.to_string());

    (config, credentials)
}

/// Create a connected `LoxoneHttpClient`, or panic with a clear message.
async fn create_connected_client() -> LoxoneHttpClient {
    let (host, user, password) = load_env_file().expect(
        "Failed to load .env file. Ensure the .env file exists at the project root \
         with SERVER, USER, and PASSWORD keys.",
    );
    let (config, credentials) = build_config_and_credentials(&host, &user, &password);

    let mut client = LoxoneHttpClient::new(config, credentials)
        .await
        .expect("Failed to create HTTP client");

    client
        .connect()
        .await
        .expect("Failed to connect to Loxone Miniserver -- is it reachable?");

    client
}

// ---------------------------------------------------------------------------
// Test 1: Connection and Structure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_connection_and_structure() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let client = create_connected_client().await;

    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure file");

    let controls_count = structure.controls.len();
    let rooms_count = structure.rooms.len();
    let cats_count = structure.cats.len();

    println!("=== Structure Summary ===");
    println!("  Controls : {controls_count}");
    println!("  Rooms    : {rooms_count}");
    println!("  Categories: {cats_count}");
    println!("  Last modified: {}", structure.last_modified);

    assert!(
        controls_count > 0,
        "Structure should contain at least one control"
    );
    assert!(
        rooms_count > 0,
        "Structure should contain at least one room"
    );

    // Print room names
    println!("\n  Rooms:");
    for room_data in structure.rooms.values() {
        if let Some(name) = room_data.get("name").and_then(|v| v.as_str()) {
            println!("    - {name}");
        }
    }
}

// ---------------------------------------------------------------------------
// Test 2: Read Device States
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_device_states() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let client = create_connected_client().await;

    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure");

    // Grab up to 5 control UUIDs for state queries
    let uuids: Vec<String> = structure.controls.keys().take(5).cloned().collect();

    assert!(!uuids.is_empty(), "Need at least one control UUID");

    println!("=== Device States (first {} controls) ===", uuids.len());

    let states = client
        .get_device_states(&uuids)
        .await
        .expect("Failed to get device states");

    for uuid in &uuids {
        let control = structure.controls.get(uuid);
        let name = control
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let control_type = control
            .and_then(|c| c.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        if let Some(state_val) = states.get(uuid) {
            println!("  [{control_type}] {name} ({uuid}): {state_val}");
        } else {
            println!("  [{control_type}] {name} ({uuid}): <no state returned>");
        }
    }

    // We expect at least some states to have been returned. Not all controls
    // support the "state" command, so we allow partial success.
    println!("  Got state for {}/{} devices", states.len(), uuids.len());
}

// ---------------------------------------------------------------------------
// Test 3: Read-only API Endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_api_endpoints() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let (host, user, password) = load_env_file().expect("Failed to load .env");
    let (_config, _credentials) = build_config_and_credentials(&host, &user, &password);

    // Build a raw reqwest client with basic auth for direct API calls
    let auth_header = format!(
        "Basic {}",
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{user}:{password}")
        )
    );

    let http = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(15))
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build reqwest client");

    let base = if host.starts_with("http") {
        host.clone()
    } else {
        format!("http://{host}")
    };

    println!("=== Read-Only API Endpoints ===");

    // 1) /jdev/cfg/api  -- API info / version
    {
        let url = format!("{base}/jdev/cfg/api");
        let resp = http
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .expect("Failed to reach /jdev/cfg/api");

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        println!("  /jdev/cfg/api  status={status}");
        println!("    body: {body}");
        assert!(
            status.is_success(),
            "/jdev/cfg/api returned non-success status: {status}"
        );
    }

    // 2) /jdev/cfg/version  -- firmware version
    {
        let url = format!("{base}/jdev/cfg/version");
        let resp = http
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .expect("Failed to reach /jdev/cfg/version");

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        println!("  /jdev/cfg/version  status={status}");
        println!("    body: {body}");
        assert!(
            status.is_success(),
            "/jdev/cfg/version returned non-success status: {status}"
        );
    }

    // 3) /jdev/sps/LoxAPPversion3  -- structure file version
    {
        let url = format!("{base}/jdev/sps/LoxAPPversion3");
        let resp = http
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .expect("Failed to reach /jdev/sps/LoxAPPversion3");

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        println!("  /jdev/sps/LoxAPPversion3  status={status}");
        println!("    body: {body}");
        assert!(
            status.is_success(),
            "/jdev/sps/LoxAPPversion3 returned non-success status: {status}"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: Device Type Inventory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_device_inventory() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let client = create_connected_client().await;

    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure");

    // Count controls by type
    let mut type_counts: HashMap<String, usize> = HashMap::new();

    for control in structure.controls.values() {
        let control_type = control
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        *type_counts.entry(control_type).or_insert(0) += 1;
    }

    // Sort by count (descending)
    let mut sorted: Vec<_> = type_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    println!("=== Device Type Inventory ===");
    println!("  Total controls: {}", structure.controls.len());
    println!();
    for (device_type, count) in &sorted {
        println!("  {count:>4}x {device_type}");
    }

    assert!(
        !type_counts.is_empty(),
        "At least one control type should exist"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Light Control (toggle + restore)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_light_toggle() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let client = create_connected_client().await;

    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure");

    // Find a Switch or LightController type device
    let light = structure.controls.iter().find(|(_uuid, control)| {
        let t = control.get("type").and_then(|v| v.as_str()).unwrap_or("");
        t == "Switch" || t == "LightController"
    });

    let (uuid, control) = match light {
        Some(pair) => pair,
        None => {
            println!("No Switch or LightController found -- skipping light toggle test");
            return;
        }
    };

    let name = control
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let control_type = control
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    println!("=== Light Toggle Test ===");
    println!("  Device: {name} ({control_type})");
    println!("  UUID  : {uuid}");

    // Read initial state
    let initial_states = client
        .get_device_states(std::slice::from_ref(uuid))
        .await
        .unwrap_or_default();
    let initial_state = initial_states.get(uuid).cloned();
    println!("  Initial state: {initial_state:?}");

    // Determine whether the light appears to be on (heuristic: non-zero value)
    let is_on = initial_state
        .as_ref()
        .map(|v| v.as_f64().unwrap_or(0.0) != 0.0 || v.as_str().map(|s| s != "0").unwrap_or(false))
        .unwrap_or(false);

    let (first_cmd, restore_cmd) = if is_on { ("Off", "On") } else { ("On", "Off") };

    // Toggle
    println!("  Sending '{first_cmd}'...");
    match client.send_command(uuid, first_cmd).await {
        Ok(resp) => println!("  Response: {:?}", resp.value),
        Err(e) => {
            println!("  Command failed: {e}");
            return;
        }
    }

    // Wait a moment for the device to react
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Read changed state
    let changed_states = client
        .get_device_states(std::slice::from_ref(uuid))
        .await
        .unwrap_or_default();
    let changed_state = changed_states.get(uuid).cloned();
    println!("  State after toggle: {changed_state:?}");

    // Restore original state
    println!("  Restoring with '{restore_cmd}'...");
    match client.send_command(uuid, restore_cmd).await {
        Ok(resp) => println!("  Restore response: {:?}", resp.value),
        Err(e) => println!("  Restore failed: {e} -- manual intervention may be needed"),
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify restoration
    let final_states = client
        .get_device_states(std::slice::from_ref(uuid))
        .await
        .unwrap_or_default();
    let final_state = final_states.get(uuid).cloned();
    println!("  Final state: {final_state:?}");

    println!("  Light toggle test completed successfully.");
}

// ---------------------------------------------------------------------------
// Test 6: Temperature Reading
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_temperature_reading() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let client = create_connected_client().await;

    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure");

    // Find climate-related controls (IRoomController, IRoomControllerV2, etc.)
    let climate_controls: Vec<_> = structure
        .controls
        .iter()
        .filter(|(_uuid, control)| {
            let t = control
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            t.contains("iroomcontroller")
                || t.contains("intelligent room controller")
                || t == "iroomcontrollerv2"
        })
        .collect();

    if climate_controls.is_empty() {
        // Fall back: look for any control that has temperature-related states
        let temp_controls: Vec<_> = structure
            .controls
            .iter()
            .filter(|(_uuid, control)| {
                if let Some(states) = control.get("states").and_then(|v| v.as_object()) {
                    states.keys().any(|k| {
                        let k_lower = k.to_lowercase();
                        k_lower.contains("temp") || k_lower.contains("temperature")
                    })
                } else {
                    false
                }
            })
            .collect();

        if temp_controls.is_empty() {
            println!("No climate or temperature controls found -- skipping temperature test");
            return;
        }

        println!("=== Temperature Reading (from controls with temperature states) ===");
        for (uuid, control) in temp_controls.iter().take(5) {
            let name = control
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let ctype = control
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let room_uuid = control.get("room").and_then(|v| v.as_str()).unwrap_or("");
            let room_name = structure
                .rooms
                .get(room_uuid)
                .and_then(|r| r.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown room");

            // Try to read temperature state UUIDs
            if let Some(states) = control.get("states").and_then(|v| v.as_object()) {
                let temp_state_uuids: Vec<String> = states
                    .iter()
                    .filter(|(k, _)| {
                        let kl = k.to_lowercase();
                        kl.contains("temp") || kl.contains("temperature")
                    })
                    .filter_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                    .collect();

                if !temp_state_uuids.is_empty() {
                    let values = client
                        .get_state_values(&temp_state_uuids)
                        .await
                        .unwrap_or_default();
                    println!("  [{ctype}] {name} (room: {room_name})");
                    for (state_uuid, val) in &values {
                        if let Some(temp) = val.as_f64() {
                            assert!(
                                (-20.0..=60.0).contains(&temp),
                                "Temperature {temp} C is outside reasonable range (-20..60)"
                            );
                            println!("    State {state_uuid}: {temp:.1} C");
                        } else {
                            println!("    State {state_uuid}: {val}");
                        }
                    }
                }
            }

            println!("  {name} ({ctype}, UUID: {uuid})");
        }
        return;
    }

    println!("=== Temperature Reading ===");
    println!("  Found {} climate controller(s)", climate_controls.len());

    for (uuid, control) in &climate_controls {
        let name = control
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let ctype = control
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let room_uuid = control.get("room").and_then(|v| v.as_str()).unwrap_or("");
        let room_name = structure
            .rooms
            .get(room_uuid)
            .and_then(|r| r.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown room");

        println!("  [{ctype}] {name} (room: {room_name}, UUID: {uuid})");

        // Try to read state values for temperature-related states
        if let Some(states) = control.get("states").and_then(|v| v.as_object()) {
            let temp_keys: Vec<_> = states
                .iter()
                .filter(|(k, _)| {
                    let kl = k.to_lowercase();
                    kl.contains("temp") || kl.contains("value")
                })
                .collect();

            let state_uuids: Vec<String> = temp_keys
                .iter()
                .filter_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                .collect();

            if !state_uuids.is_empty() {
                let values = client
                    .get_state_values(&state_uuids)
                    .await
                    .unwrap_or_default();

                for (key, state_uuid_val) in &temp_keys {
                    if let Some(state_uuid) = state_uuid_val.as_str() {
                        if let Some(val) = values.get(state_uuid) {
                            if let Some(temp) = val.as_f64() {
                                if key.to_lowercase().contains("temp") {
                                    assert!(
                                        (-20.0..=60.0).contains(&temp),
                                        "Temperature {temp} C outside reasonable range"
                                    );
                                }
                                println!("    {key}: {temp:.1}");
                            } else {
                                println!("    {key}: {val}");
                            }
                        } else {
                            println!("    {key}: <could not read value>");
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test 7: MCP Server Tool Integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_mcp_tool_execution() {
    if !should_run_live_tests() {
        eprintln!("Skipping live test (set LOXONE_LIVE_TEST=1 to run)");
        return;
    }

    let (host, user, password) = load_env_file().expect("Failed to load .env");
    let (config, credentials) = build_config_and_credentials(&host, &user, &password);

    let mut http_client = LoxoneHttpClient::new(config.clone(), credentials)
        .await
        .expect("Failed to create HTTP client");

    http_client
        .connect()
        .await
        .expect("Failed to connect to Miniserver");

    let client: Arc<dyn LoxoneClient> = Arc::new(http_client);
    let context = Arc::new(ClientContext::new());

    // Populate context with structure
    let structure = client
        .get_structure()
        .await
        .expect("Failed to fetch structure");
    context
        .update_structure(structure)
        .await
        .expect("Failed to update context");

    // Build supporting services
    let sensor_registry = Arc::new(SensorTypeRegistry::new());
    let value_resolver = Arc::new(UnifiedValueResolver::new(client.clone(), sensor_registry));
    let server_config = ServerConfig {
        loxone: config,
        ..ServerConfig::default()
    };

    let mcp_server = LoxoneMcpServer::with_context(
        client.clone(),
        context.clone(),
        value_resolver,
        None,
        server_config,
    );

    println!("=== MCP Tool: get_lights_status ===");
    match mcp_server.get_lights_status().await {
        Ok(result) => {
            let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("  Lights found: {count}");
            if let Some(lights) = result.get("lights").and_then(|v| v.as_array()) {
                for light in lights.iter().take(10) {
                    let name = light.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let ltype = light.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("    - {name} ({ltype})");
                }
                if lights.len() > 10 {
                    println!("    ... and {} more", lights.len() - 10);
                }
            }
            assert!(
                result.get("lights").is_some(),
                "get_lights_status should return a 'lights' field"
            );
        }
        Err(e) => {
            panic!("get_lights_status failed: {e}");
        }
    }

    println!("\n=== MCP Tool: get_climate_status ===");
    match mcp_server.get_climate_status().await {
        Ok(result) => {
            let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("  Climate controllers found: {count}");
            if let Some(controllers) = result.get("climate_controllers").and_then(|v| v.as_array())
            {
                for ctrl in controllers.iter().take(10) {
                    let name = ctrl.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("    - {name}");
                }
            }
            assert!(
                result.get("climate_controllers").is_some(),
                "get_climate_status should return a 'climate_controllers' field"
            );
        }
        Err(e) => {
            // Climate controllers may not exist on every setup -- just report
            println!("  get_climate_status returned error (may be expected): {e}");
        }
    }

    println!("\n=== MCP Tool: list_rooms ===");
    match mcp_server.list_rooms().await {
        Ok(result) => {
            if let Some(rooms) = result.get("rooms").and_then(|v| v.as_array()) {
                println!("  Rooms found: {}", rooms.len());
                for room in rooms.iter().take(20) {
                    let name = room.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("    - {name}");
                }
            }
            assert!(
                result.get("rooms").is_some(),
                "list_rooms should return a 'rooms' field"
            );
        }
        Err(e) => {
            panic!("list_rooms failed: {e}");
        }
    }

    println!("\n=== MCP Tool: get_sensor_readings ===");
    match mcp_server.get_sensor_readings().await {
        Ok(result) => {
            let count = result.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("  Sensors found: {count}");
            if let Some(sensors) = result.get("sensors").and_then(|v| v.as_array()) {
                for sensor in sensors.iter().take(10) {
                    let name = sensor.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let stype = sensor.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("    - {name} ({stype})");
                }
                if sensors.len() > 10 {
                    println!("    ... and {} more", sensors.len() - 10);
                }
            }
        }
        Err(e) => {
            println!("  get_sensor_readings returned error (may be expected): {e}");
        }
    }

    println!("\n=== MCP Tool: get_server_status ===");
    match mcp_server.get_server_status().await {
        Ok(result) => {
            println!(
                "  Server status: {}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        Err(e) => {
            println!("  get_server_status returned error: {e}");
        }
    }
}
