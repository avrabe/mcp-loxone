//! loxone-cli — Thin MCP client for controlling Loxone via a running loxone-mcp-server
//!
//! Sends JSON-RPC requests to the MCP server's HTTP endpoint.
//! Auto-starts the server if it's not running.

use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::process::Stdio;
use std::time::Duration;

const DEFAULT_URL: &str = "http://localhost:3001";
const MCP_ENDPOINT: &str = "/mcp";
/// Streamable HTTP requires clients to accept both JSON and SSE responses.
const ACCEPT: &str = "application/json, text/event-stream";

#[derive(Parser)]
#[command(name = "loxone-cli")]
#[command(about = "Control Loxone smart home via MCP server")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// MCP server URL
    #[arg(long, default_value = DEFAULT_URL, env = "LOXONE_MCP_URL")]
    url: String,

    /// Output raw JSON
    #[arg(long)]
    json: bool,

    /// Credential ID for auto-starting the server
    #[arg(long, env = "LOXONE_CREDENTIAL_ID")]
    credential_id: Option<String>,

    /// Server port (for auto-start)
    #[arg(long, default_value = "3001")]
    port: u16,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List all rooms
    Rooms,
    /// List devices (optionally filtered by room)
    Devices {
        /// Filter by room name
        #[arg(long)]
        room: Option<String>,
    },
    /// Get device details
    Device {
        /// Device ID or name
        id: String,
    },
    /// Show server status
    Status,

    // --- Lighting ---
    /// Show light status or control lights
    Lights {
        /// Action: on, off, dim
        action: Option<String>,
        /// Target room or device
        #[arg(long)]
        target: Option<String>,
        /// Brightness 0-100 (for dim)
        #[arg(long)]
        brightness: Option<u8>,
    },

    // --- Climate ---
    /// Show climate status or set temperature
    Climate {
        /// Temperature to set
        #[arg(long)]
        set: Option<f64>,
        /// Room name
        #[arg(long)]
        room: Option<String>,
        /// Mode: heat, cool, auto, off
        #[arg(long)]
        mode: Option<String>,
    },

    // --- Blinds ---
    /// Show blinds status or control blinds
    Blinds {
        /// Action: up, down, stop, shade
        action: Option<String>,
        /// Target blind or room
        #[arg(long)]
        target: Option<String>,
        /// Position 0-100
        #[arg(long)]
        position: Option<u8>,
    },

    // --- Audio ---
    /// Show audio status or control audio
    Audio {
        /// Action: play, pause, stop, next, prev
        action: Option<String>,
        /// Zone name
        #[arg(long)]
        zone: Option<String>,
        /// Volume 0-100
        #[arg(long)]
        volume: Option<u8>,
    },

    // --- Sensors ---
    /// Show sensor readings
    Sensors,
    /// Show door/window status
    Doors,
    /// Show motion sensor status
    Motion,

    // --- Weather & Energy ---
    /// Show weather data
    Weather,
    /// Show energy consumption
    Energy,

    // --- Security ---
    /// Show security status or change mode
    Security {
        /// Mode: arm, disarm, arm-home, arm-away
        mode: Option<String>,
        /// Security code
        #[arg(long)]
        code: Option<String>,
    },

    /// Lock/unlock a door
    Lock {
        /// Lock name or ID
        name: String,
        /// Action: lock, unlock, open
        action: String,
    },

    // --- Intercom ---
    /// Show camera/intercom status
    Cameras,
    /// Control intercom
    Intercom {
        /// Intercom name or ID
        name: String,
        /// Action: answer, decline, open
        action: String,
    },

    // --- Scenes ---
    /// List available scenes
    Scenes,
    /// Activate a scene
    Scene {
        /// Scene name
        name: String,
        /// Room (optional)
        #[arg(long)]
        room: Option<String>,
    },

    // --- Low-level ---
    /// List all MCP tools
    Tools,
    /// Call any MCP tool directly
    Call {
        /// Tool name
        tool: String,
        /// Arguments as key=value pairs
        args: Vec<String>,
    },
}

struct McpClient {
    http: reqwest::Client,
    base_url: String,
    session_id: Option<String>,
    request_id: u64,
}

impl McpClient {
    fn new(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            session_id: None,
            request_id: 0,
        }
    }

    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    async fn send_jsonrpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id();
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id
        });

        let url = format!("{}{}", self.base_url, MCP_ENDPOINT);
        let mut req = self.http.post(&url).header("Accept", ACCEPT).json(&body);

        if let Some(sid) = &self.session_id {
            req = req.header("Mcp-Session-Id", sid);
        }

        let resp = req.send().await.map_err(|e| format!("HTTP error: {e}"))?;

        // Capture session ID from response
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            self.session_id = sid.to_str().ok().map(String::from);
        }

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Server returned {status}: {text}"));
        }

        // The streamable HTTP transport may return SSE-formatted responses
        // Try to parse as JSON first, then try SSE format
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            return Self::extract_result(json);
        }

        // Parse SSE: look for "data: {...}" lines
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ")
                && let Ok(json) = serde_json::from_str::<Value>(data)
            {
                return Self::extract_result(json);
            }
        }

        Err(format!("Unexpected response format: {text}"))
    }

    fn extract_result(json: Value) -> Result<Value, String> {
        if let Some(error) = json.get("error") {
            return Err(format!(
                "MCP error: {}",
                error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown")
            ));
        }
        Ok(json.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.send_jsonrpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "loxone-cli",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await?;

        // Send initialized notification (a notification carries no id)
        let body = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let url = format!("{}{}", self.base_url, MCP_ENDPOINT);
        let mut req = self.http.post(&url).header("Accept", ACCEPT).json(&body);
        if let Some(sid) = &self.session_id {
            req = req.header("Mcp-Session-Id", sid);
        }
        // Fire and forget — notification
        let _ = req.send().await;

        Ok(())
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let result = self
            .send_jsonrpc(
                "tools/call",
                json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await?;

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Extract text content from MCP tool result
        if let Some(content) = result.get("content").and_then(|c| c.as_array())
            && let Some(first) = content.first()
            && let Some(text) = first.get("text").and_then(|t| t.as_str())
        {
            if is_error {
                return Err(text.to_string());
            }
            // Try to parse the inner JSON
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                return Ok(parsed);
            }
            return Ok(Value::String(text.to_string()));
        }

        Ok(result)
    }

    async fn list_tools(&mut self) -> Result<Value, String> {
        self.send_jsonrpc("tools/list", json!({})).await
    }
}

async fn ensure_server_running(cli: &Cli) -> Result<(), String> {
    // Try to connect
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}{}", cli.url.trim_end_matches('/'), MCP_ENDPOINT);
    let probe = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "probe", "version": "0" }
        },
        "id": 0
    });

    if client.post(&url).json(&probe).send().await.is_ok() {
        return Ok(());
    }

    // Server not running — auto-start
    eprintln!("MCP server not running, starting...");

    let server_bin = which_server()?;
    let mut cmd = tokio::process::Command::new(&server_bin);
    cmd.arg("streamable-http")
        .arg("--port")
        .arg(cli.port.to_string());

    if let Some(cid) = &cli.credential_id {
        cmd.arg("--credential-id").arg(cid);
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    cmd.spawn()
        .map_err(|e| format!("Failed to start server: {e}"))?;

    // Wait for server to be ready (up to 15 seconds)
    for i in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if client.post(&url).json(&probe).send().await.is_ok() {
            eprintln!("MCP server ready (took {}ms)", (i + 1) * 500);
            return Ok(());
        }
    }

    Err("Server failed to start within 15 seconds".to_string())
}

fn which_server() -> Result<String, String> {
    // Check if loxone-mcp-server is on PATH
    if let Ok(output) = std::process::Command::new("which")
        .arg("loxone-mcp-server")
        .output()
        && output.status.success()
    {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    // Fallback: check common locations
    let candidates = [
        "./target/release/loxone-mcp-server",
        "./target/debug/loxone-mcp-server",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    Err("loxone-mcp-server not found. Install it or set PATH.".to_string())
}

fn format_output(value: &Value, raw_json: bool) {
    if raw_json {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        );
        return;
    }

    // Format based on content structure
    match value {
        Value::Array(items) => {
            for item in items {
                format_item(item);
            }
        }
        Value::Object(map) => {
            // Check if it's a status response with nested data
            if let Some(status) = map.get("status")
                && let Some(s) = status.as_str()
                && s == "error"
                && let Some(msg) = map.get("message").and_then(|m| m.as_str())
            {
                eprintln!("Error: {msg}");
                return;
            }

            // Check for common list patterns
            if let Some(items) = map
                .get("rooms")
                .or(map.get("devices"))
                .or(map.get("lights"))
                .or(map.get("blinds"))
                .or(map.get("scenes"))
                .or(map.get("zones"))
                .or(map.get("sensors"))
                && let Some(arr) = items.as_array()
            {
                for item in arr {
                    format_item(item);
                }
                return;
            }

            // Generic object display
            for (key, val) in map {
                match val {
                    Value::String(s) => println!("  {key}: {s}"),
                    Value::Number(n) => println!("  {key}: {n}"),
                    Value::Bool(b) => println!("  {key}: {b}"),
                    Value::Null => println!("  {key}: -"),
                    _ => println!(
                        "  {key}: {}",
                        serde_json::to_string(val).unwrap_or_default()
                    ),
                }
            }
        }
        Value::String(s) => println!("{s}"),
        _ => println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        ),
    }
}

fn format_item(item: &Value) {
    if let Some(obj) = item.as_object() {
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let uuid = obj.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        let state = obj.get("state").or(obj.get("status")).or(obj.get("value"));
        let type_name = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let mut line = format!("  {name}");
        if !type_name.is_empty() {
            line.push_str(&format!(" ({type_name})"));
        }
        if let Some(s) = state {
            match s {
                Value::String(v) => line.push_str(&format!(" = {v}")),
                Value::Number(v) => line.push_str(&format!(" = {v}")),
                Value::Bool(v) => line.push_str(&format!(" = {v}")),
                _ => {}
            }
        }
        if !uuid.is_empty() {
            line.push_str(&format!("  [{uuid}]"));
        }
        println!("{line}");
    } else {
        println!(
            "  {}",
            serde_json::to_string(item).unwrap_or_else(|_| item.to_string())
        );
    }
}

fn parse_kv_args(args: &[String]) -> Value {
    let mut map = serde_json::Map::new();
    for arg in args {
        if let Some((key, val)) = arg.split_once('=') {
            // Try to parse as number or bool, fallback to string
            if let Ok(n) = val.parse::<f64>() {
                map.insert(key.to_string(), json!(n));
            } else if val == "true" {
                map.insert(key.to_string(), json!(true));
            } else if val == "false" {
                map.insert(key.to_string(), json!(false));
            } else {
                map.insert(key.to_string(), json!(val));
            }
        }
    }
    Value::Object(map)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    ensure_server_running(&cli).await?;

    let mut client = McpClient::new(&cli.url);
    client.initialize().await?;

    let result = match &cli.command {
        Command::Rooms => client.call_tool("list_rooms", json!({})).await?,
        Command::Devices { room } => {
            client
                .call_tool("list_devices", json!({ "room": room }))
                .await?
        }
        Command::Device { id } => {
            client
                .call_tool("get_device_info", json!({ "device_id": id }))
                .await?
        }
        Command::Status => client.call_tool("get_server_status", json!({})).await?,

        Command::Lights {
            action,
            target,
            brightness,
        } => {
            if let Some(action) = action {
                client
                    .call_tool(
                        "control_lights",
                        json!({
                            "scope": if target.is_some() { "room" } else { "system" },
                            "target": target,
                            "action": action,
                            "brightness": brightness
                        }),
                    )
                    .await?
            } else {
                client.call_tool("get_lights_status", json!({})).await?
            }
        }

        Command::Climate { set, room, mode } => {
            if let Some(temp) = set {
                let room = room
                    .as_deref()
                    .ok_or("--room is required when setting temperature")?;
                client
                    .call_tool(
                        "set_temperature",
                        json!({
                            "room": room,
                            "temperature": temp,
                            "mode": mode
                        }),
                    )
                    .await?
            } else {
                client.call_tool("get_climate_status", json!({})).await?
            }
        }

        Command::Blinds {
            action,
            target,
            position,
        } => {
            if let Some(action) = action {
                let target = target
                    .as_deref()
                    .ok_or("--target is required for blind control")?;
                client
                    .call_tool(
                        "control_blinds",
                        json!({
                            "target": target,
                            "action": action,
                            "position": position
                        }),
                    )
                    .await?
            } else {
                client.call_tool("get_blinds_status", json!({})).await?
            }
        }

        Command::Audio {
            action,
            zone,
            volume,
        } => {
            if let Some(vol) = volume {
                let zone = zone.as_deref().ok_or("--zone is required for volume")?;
                client
                    .call_tool("set_audio_volume", json!({ "zone": zone, "volume": vol }))
                    .await?
            } else if let Some(action) = action {
                let zone = zone
                    .as_deref()
                    .ok_or("--zone is required for audio control")?;
                client
                    .call_tool(
                        "control_audio_zone",
                        json!({ "zone": zone, "action": action }),
                    )
                    .await?
            } else {
                client.call_tool("get_audio_status", json!({})).await?
            }
        }

        Command::Sensors => client.call_tool("get_sensor_readings", json!({})).await?,
        Command::Doors => {
            client
                .call_tool("get_door_window_status", json!({}))
                .await?
        }
        Command::Motion => client.call_tool("get_motion_status", json!({})).await?,
        Command::Weather => client.call_tool("get_weather", json!({})).await?,
        Command::Energy => client.call_tool("get_energy_status", json!({})).await?,

        Command::Security { mode, code } => {
            if let Some(mode) = mode {
                client
                    .call_tool("set_security_mode", json!({ "mode": mode, "code": code }))
                    .await?
            } else {
                client.call_tool("get_security_status", json!({})).await?
            }
        }

        Command::Lock { name, action } => {
            client
                .call_tool(
                    "control_door_lock",
                    json!({ "lock": name, "action": action }),
                )
                .await?
        }

        Command::Cameras => client.call_tool("get_camera_status", json!({})).await?,
        Command::Intercom { name, action } => {
            client
                .call_tool(
                    "control_intercom",
                    json!({ "intercom": name, "action": action }),
                )
                .await?
        }

        Command::Scenes => client.call_tool("list_scenes", json!({})).await?,
        Command::Scene { name, room } => {
            client
                .call_tool("activate_scene", json!({ "scene": name, "room": room }))
                .await?
        }

        Command::Tools => {
            let result = client.list_tools().await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                println!("Available tools ({}):", tools.len());
                for tool in tools {
                    let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                    let desc = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    println!("  {name:30} {desc}");
                }
            }
            return Ok(());
        }

        Command::Call { tool, args } => {
            let arguments = parse_kv_args(args);
            client.call_tool(tool, arguments).await?
        }
    };

    format_output(&result, cli.json);
    Ok(())
}
