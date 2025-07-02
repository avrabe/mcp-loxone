//! WebSocket Real-Time Integration Demo
//!
//! This example demonstrates the enhanced WebSocket client with real-time
//! state updates, event filtering, subscription management, and hybrid operation.

#[cfg(feature = "websocket")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use loxone_mcp_rust::client::websocket_client::{
        EventFilter, LoxoneEventType, ReconnectionConfig,
    };
    use loxone_mcp_rust::client::{create_hybrid_client, create_websocket_client};
    use loxone_mcp_rust::config::credentials::LoxoneCredentials;
    use loxone_mcp_rust::config::{AuthMethod, LoxoneConfig};
    use std::collections::HashSet;
    use std::time::Duration;
    use url::Url;

    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🌐 WebSocket Real-Time Integration Demo");
    println!("======================================\n");

    let config = LoxoneConfig {
        url: Url::parse("http://192.168.1.100")?,
        username: "demo_user".to_string(),
        verify_ssl: false,
        timeout: Duration::from_secs(30),
        max_retries: 3,
        max_connections: Some(10),
        #[cfg(feature = "websocket")]
        websocket: Default::default(),
        auth_method: AuthMethod::Basic, // For demo compatibility
    };

    let credentials = LoxoneCredentials {
        username: "demo_user".to_string(),
        password: "demo_password".to_string(),
        api_key: None,
        #[cfg(feature = "crypto-openssl")]
        public_key: None,
    };

    // Demo 1: Hybrid Client (WebSocket + HTTP)
    println!("1️⃣  Creating Hybrid Client (WebSocket + HTTP)");
    match create_hybrid_client(&config, &credentials).await {
        Ok(mut hybrid_client) => {
            println!("   ✅ Hybrid client created successfully");
            println!("   🔄 HTTP client handles: commands, structure fetching, system info");
            println!("   📡 WebSocket handles: real-time state updates, events, monitoring");

            // Configure reconnection behavior
            let reconnection_config = ReconnectionConfig {
                enabled: true,
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
                max_attempts: Some(5), // Limited for demo
                jitter_factor: 0.1,
            };
            hybrid_client.set_reconnection_config(reconnection_config);
            println!("   🔄 Configured automatic reconnection with exponential backoff");

            // Demo subscription filtering
            println!("\n   📊 Setting up event subscriptions:");

            // Subscribe to all lighting devices
            let mut lighting_filter = EventFilter::default();
            lighting_filter.event_types.insert(LoxoneEventType::State);
            let _lighting_updates = hybrid_client.subscribe_with_filter(lighting_filter).await;
            println!("      💡 Subscribed to lighting state changes");

            // Subscribe to specific rooms
            let mut rooms = HashSet::new();
            rooms.insert("Living Room".to_string());
            rooms.insert("Kitchen".to_string());
            let _room_updates = hybrid_client.subscribe_to_rooms(rooms).await;
            println!("      🏠 Subscribed to Living Room and Kitchen events");

            // Subscribe to weather updates only
            let mut weather_types = HashSet::new();
            weather_types.insert(LoxoneEventType::Weather);
            let _weather_updates = hybrid_client.subscribe_to_event_types(weather_types).await;
            println!("      🌤️  Subscribed to weather updates");

            // Get statistics
            let stats = hybrid_client.get_stats().await;
            println!("\n   📈 Current Statistics:");
            println!("      Messages received: {}", stats.messages_received);
            println!("      State updates: {}", stats.state_updates);
            println!(
                "      Reconnection attempts: {}",
                stats.reconnection_attempts
            );

            println!("   ⚠️  Note: Connection would be established in a real environment");
        }
        Err(e) => println!("   ❌ Error creating hybrid client: {e}"),
    }

    // Demo 2: Standalone WebSocket Client
    println!("\n2️⃣  Creating Standalone WebSocket Client");
    match create_websocket_client(&config, &credentials).await {
        Ok(_ws_client) => {
            println!("   ✅ Standalone WebSocket client created successfully");
            println!("   📡 Real-time monitoring only (no HTTP capabilities)");
            println!("   💡 Ideal for dedicated event processing applications");
        }
        Err(e) => println!("   ❌ Error creating WebSocket client: {e}"),
    }

    // Demo 3: Event Type System
    println!("\n3️⃣  Event Type System");
    let event_types = vec![
        LoxoneEventType::State,
        LoxoneEventType::Weather,
        LoxoneEventType::Text,
        LoxoneEventType::Alarm,
        LoxoneEventType::System,
        LoxoneEventType::Sensor,
        LoxoneEventType::Unknown("custom".to_string()),
    ];

    for event_type in event_types {
        let serialized = serde_json::to_string(&event_type)?;
        println!("   🏷️  Event type: {event_type:?} → JSON: {serialized}");
    }

    // Demo 4: Advanced Filtering
    println!("\n4️⃣  Advanced Event Filtering");
    let advanced_filter = EventFilter {
        device_uuids: {
            let mut set = HashSet::new();
            set.insert("device-uuid-1".to_string());
            set.insert("device-uuid-2".to_string());
            set
        },
        event_types: {
            let mut set = HashSet::new();
            set.insert(LoxoneEventType::State);
            set.insert(LoxoneEventType::Sensor);
            set
        },
        rooms: {
            let mut set = HashSet::new();
            set.insert("Living Room".to_string());
            set
        },
        states: {
            let mut set = HashSet::new();
            set.insert("temperature".to_string());
            set.insert("humidity".to_string());
            set
        },
        min_interval: Some(Duration::from_millis(500)), // Debouncing
    };

    println!("   🎯 Filter Configuration:");
    println!(
        "      Device UUIDs: {} devices",
        advanced_filter.device_uuids.len()
    );
    println!(
        "      Event types: {} types",
        advanced_filter.event_types.len()
    );
    println!("      Rooms: {} rooms", advanced_filter.rooms.len());
    println!("      States: {} state types", advanced_filter.states.len());
    println!(
        "      Debounce interval: {:?}",
        advanced_filter.min_interval
    );

    println!("\n✨ WebSocket Features Summary:");
    println!("   • Real-time device state updates");
    println!("   • Advanced event filtering and subscription management");
    println!("   • Automatic reconnection with exponential backoff and jitter");
    println!("   • Hybrid operation (WebSocket + HTTP) for optimal performance");
    println!("   • Comprehensive statistics and monitoring");
    println!("   • Support for both token and basic authentication");
    println!("   • Efficient binary message parsing for sensor data");
    println!("   • Event debouncing to prevent spam");

    println!("\n📚 Use Cases:");
    println!("   • Real-time dashboards and monitoring applications");
    println!("   • Event-driven automation and alerting systems");
    println!("   • IoT data processing and analytics");
    println!("   • Mobile apps requiring live state synchronization");
    println!("   • Integration with external systems (MQTT, databases, etc.)");

    Ok(())
}

#[cfg(not(feature = "websocket"))]
fn main() {
    println!("WebSocket demo requires the 'websocket' feature to be enabled.");
    println!("Run with: cargo run --features websocket --example websocket_demo");
}
