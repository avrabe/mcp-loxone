//! Discovery Cache Demo
//!
//! This example demonstrates the discovery result caching system that helps
//! reduce network overhead and improve performance during repeated discovery
//! operations by caching previously discovered devices.

use loxone_mcp_rust::discovery::{
    DiscoveredDevice, DiscoveryCache, DiscoveryCacheConfig, NetworkContext,
};
use std::net::IpAddr;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔍 Discovery Cache Demo");
    println!("======================\n");

    // Demo 1: Discovery Cache Configuration
    println!("1️⃣  Discovery Cache Configuration Options");

    // Default configuration
    let default_config = DiscoveryCacheConfig::default();
    println!("   🎯 Default Configuration:");
    println!("      Default TTL: {:?}", default_config.default_ttl);
    println!("      Max entries: {}", default_config.max_entries);
    println!(
        "      Cleanup interval: {:?}",
        default_config.cleanup_interval
    );
    println!(
        "      Min scan interval: {:?}",
        default_config.min_scan_interval
    );
    println!(
        "      Enable persistence: {}",
        default_config.enable_persistence
    );

    // Method-specific TTLs
    println!("\n   ⏰ Method-specific TTLs:");
    for (method, ttl) in &default_config.method_ttls {
        println!("      {method}: {ttl:?}");
    }

    // High-performance configuration
    let high_perf_config = DiscoveryCacheConfig {
        default_ttl: Duration::from_secs(300), // 5 minutes
        max_entries: 5000,
        cleanup_interval: Duration::from_secs(30),
        min_scan_interval: Duration::from_secs(10),
        enable_persistence: true,
        ..Default::default()
    };

    println!("\n   🚀 High-Performance Configuration:");
    println!("      Default TTL: {:?}", high_perf_config.default_ttl);
    println!("      Max entries: {}", high_perf_config.max_entries);
    println!(
        "      Cleanup interval: {:?}",
        high_perf_config.cleanup_interval
    );

    // Demo 2: Device Discovery and Caching
    println!("\n2️⃣  Device Discovery and Caching");

    let cache = DiscoveryCache::with_config(default_config);
    cache.start().await?;

    println!("   ✅ Discovery cache started");

    // Create sample discovered devices
    let devices = vec![
        create_sample_device(
            "192.168.1.100",
            "1001AB234",
            "Living Room Miniserver",
            "mdns",
        ),
        create_sample_device("192.168.1.101", "1001AB235", "Kitchen Extension", "upnp"),
        create_sample_device(
            "192.168.1.102",
            "1001AB236",
            "Garage Controller",
            "network_scan",
        ),
        create_sample_device("192.168.1.103", "1001AB237", "Garden Sensors", "manual"),
    ];

    // Add devices to cache
    for device in &devices {
        cache.add_device(device.clone()).await?;
        println!(
            "   📥 Cached device: {} at {} (via {})",
            device.name, device.ip_address, device.discovery_method
        );
    }

    // Show cache statistics
    let stats = cache.get_statistics().await;
    println!("\n   📊 Cache Statistics:");
    println!("      Total entries: {}", stats.total_entries);
    println!("      Total discoveries: {}", stats.total_discoveries);
    println!("      Cache hits: {}", stats.cache_hits);
    println!("      Cache misses: {}", stats.cache_misses);
    println!("      Hit ratio: {:.1}%", stats.hit_ratio * 100.0);
    println!("      Memory usage: {} bytes", stats.memory_usage_bytes);

    // Demo 3: Cache Retrieval Operations
    println!("\n3️⃣  Cache Retrieval Operations");

    // Get device by IP address
    let ip = IpAddr::V4("192.168.1.100".parse()?);
    if let Some(device) = cache.get_device(&ip).await {
        println!("   🎯 Found device by IP {}: {}", ip, device.name);
    }

    // Get device by serial number
    if let Some(device) = cache.get_device_by_serial("1001AB235").await {
        println!(
            "   🎯 Found device by serial: {} at {}",
            device.name, device.ip_address
        );
    }

    // Get all cached devices
    let all_devices = cache.get_all_devices().await;
    println!("   📋 All cached devices ({} total):", all_devices.len());
    for device in &all_devices {
        println!(
            "      • {} ({}) - Age: {:?}",
            device.name,
            device.ip_address,
            device.age()
        );
    }

    // Get devices by discovery method
    let mdns_devices = cache.get_devices_by_method("mdns").await;
    println!("   🔍 mDNS discovered devices: {}", mdns_devices.len());

    let upnp_devices = cache.get_devices_by_method("upnp").await;
    println!("   🔍 UPnP discovered devices: {}", upnp_devices.len());

    // Demo 4: Cache Hit/Miss Behavior
    println!("\n4️⃣  Cache Hit/Miss Behavior");

    // Cache hits
    let ip1 = IpAddr::V4("192.168.1.100".parse()?);
    let ip2 = IpAddr::V4("192.168.1.101".parse()?);
    cache.get_device(&ip1).await; // Hit
    cache.get_device(&ip2).await; // Hit

    // Cache misses
    let missing_ip1 = IpAddr::V4("192.168.1.200".parse()?);
    let missing_ip2 = IpAddr::V4("192.168.1.201".parse()?);
    cache.get_device(&missing_ip1).await; // Miss
    cache.get_device(&missing_ip2).await; // Miss

    let updated_stats = cache.get_statistics().await;
    println!("   📊 Updated Statistics:");
    println!("      Cache hits: {}", updated_stats.cache_hits);
    println!("      Cache misses: {}", updated_stats.cache_misses);
    println!("      Hit ratio: {:.1}%", updated_stats.hit_ratio * 100.0);

    // Demo 5: Device Discovery Methods
    println!("\n5️⃣  Discovery Method Statistics");

    println!("   📈 Devices by discovery method:");
    for (method, count) in &updated_stats.devices_by_method {
        println!("      {method}: {count} devices");
    }

    // Demo 6: Scan Interval Management
    println!("\n6️⃣  Scan Interval Management");

    println!("   ⏱️  Full scan needed: {}", cache.needs_full_scan().await);

    // Mark scan as completed
    cache.mark_full_scan_completed().await;
    println!("   ✅ Marked full scan as completed");

    println!(
        "   ⏱️  Full scan needed now: {}",
        cache.needs_full_scan().await
    );

    // Wait a bit and check again
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!(
        "   ⏱️  Full scan needed after 1s: {}",
        cache.needs_full_scan().await
    );

    // Demo 7: Cache Expiration and Cleanup
    println!("\n7️⃣  Cache Expiration and Cleanup");

    // Create cache with short TTL for demonstration
    let short_ttl_config = DiscoveryCacheConfig {
        default_ttl: Duration::from_millis(100),
        cleanup_interval: Duration::from_secs(1),
        ..Default::default()
    };

    let expiry_cache = DiscoveryCache::with_config(short_ttl_config);
    expiry_cache.start().await?;

    // Add a device
    let temp_device = create_sample_device("192.168.1.199", "TEMP001", "Temporary Device", "test");
    expiry_cache.add_device(temp_device.clone()).await?;
    println!("   ⏰ Added temporary device with short TTL");

    // Check if device exists
    let temp_ip = IpAddr::V4("192.168.1.199".parse()?);
    if expiry_cache.get_device(&temp_ip).await.is_some() {
        println!("   ✅ Device found immediately after adding");
    }

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Manual cleanup
    let expired_count = expiry_cache.cleanup_expired().await?;
    println!("   🧹 Cleaned up {} expired entries", expired_count);

    // Check if device still exists
    if expiry_cache.get_device(&temp_ip).await.is_none() {
        println!("   ❌ Device no longer found after expiration");
    }

    // Demo 8: Stale Device Detection
    println!("\n8️⃣  Stale Device Detection");

    let stale_cache = DiscoveryCache::with_config(DiscoveryCacheConfig {
        max_device_age: Duration::from_millis(50),
        ..Default::default()
    });
    stale_cache.start().await?;

    // Add device and make it stale
    let mut stale_device =
        create_sample_device("192.168.1.250", "STALE001", "Stale Device", "test");
    stale_device.last_seen = std::time::SystemTime::now() - Duration::from_millis(100); // Make it old
    stale_cache.add_device(stale_device).await?;

    println!("   👴 Added device that appears stale");

    // Cleanup stale devices
    let stale_count = stale_cache.cleanup_stale_devices().await?;
    println!("   🧹 Cleaned up {} stale devices", stale_count);

    // Demo 9: Persistence Simulation
    println!("\n9️⃣  Cache Persistence");

    let persistent_config = DiscoveryCacheConfig {
        enable_persistence: true,
        cache_file_path: Some("/tmp/loxone_discovery_cache.json".into()),
        ..Default::default()
    };

    let persistent_cache = DiscoveryCache::with_config(persistent_config);
    persistent_cache.start().await?;

    // Add some devices
    for i in 1..=3 {
        let device = create_sample_device(
            &format!("192.168.1.{}", 150 + i),
            &format!("PERSIST{i:03}"),
            &format!("Persistent Device {i}"),
            "manual",
        );
        persistent_cache.add_device(device).await?;
    }

    println!("   💾 Added devices to persistent cache");

    // Stop cache (triggers save)
    persistent_cache.stop().await?;
    println!("   💾 Cache stopped and saved to disk");

    // Demo 10: Network Context Awareness
    println!("\n🔟 Network Context Awareness");

    let network_context = NetworkContext::current();
    println!("   🌐 Current Network Context:");
    println!("      Local IPs: {:?}", network_context.local_ips);
    println!("      Interfaces: {:?}", network_context.interfaces);
    println!("      Captured at: {:?}", network_context.captured_at);

    // Demo 11: Performance and Memory Usage
    println!("\n1️⃣1️⃣ Performance and Memory Usage");

    let performance_cache = DiscoveryCache::with_config(DiscoveryCacheConfig {
        max_entries: 1000,
        ..Default::default()
    });
    performance_cache.start().await?;

    // Add many devices to test performance
    let start_time = std::time::Instant::now();
    for i in 1..=100 {
        let device = create_sample_device(
            &format!("10.0.0.{i}"),
            &format!("PERF{i:03}"),
            &format!("Performance Test Device {i}"),
            "benchmark",
        );
        performance_cache.add_device(device).await?;
    }
    let insert_duration = start_time.elapsed();

    println!("   ⚡ Added 100 devices in {:?}", insert_duration);

    // Test retrieval performance
    let start_time = std::time::Instant::now();
    for i in 1..=100 {
        let ip = IpAddr::V4(format!("10.0.0.{i}").parse()?);
        performance_cache.get_device(&ip).await;
    }
    let retrieval_duration = start_time.elapsed();

    println!("   ⚡ Retrieved 100 devices in {:?}", retrieval_duration);

    let perf_stats = performance_cache.get_statistics().await;
    println!("   📊 Performance Statistics:");
    println!("      Total entries: {}", perf_stats.total_entries);
    println!(
        "      Memory usage: {} bytes",
        perf_stats.memory_usage_bytes
    );
    println!(
        "      Avg per device: {} bytes",
        perf_stats.memory_usage_bytes / perf_stats.total_entries
    );

    // Summary
    println!("\n✨ Discovery Cache Benefits Summary:");
    println!("   • Reduces network overhead by caching discovery results");
    println!("   • Method-specific TTLs optimize cache effectiveness");
    println!("   • Automatic cleanup prevents memory leaks");
    println!("   • Statistics provide visibility into cache performance");
    println!("   • Persistent storage enables cache survival across restarts");
    println!("   • Network context awareness for intelligent invalidation");
    println!("   • Stale device detection for maintaining data freshness");

    println!("\n🔧 Integration Examples:");
    println!("   // Create and start discovery cache");
    println!("   let cache = DiscoveryCache::new();");
    println!("   cache.start().await?;");
    println!("   ");
    println!("   // Add discovered devices");
    println!("   cache.add_device(discovered_device).await?;");
    println!("   ");
    println!("   // Check cache before performing expensive discovery");
    println!("   if let Some(device) = cache.get_device(&ip).await {{");
    println!("       // Use cached device");
    println!("   }} else {{");
    println!("       // Perform discovery and cache result");
    println!("   }}");

    Ok(())
}

fn create_sample_device(ip: &str, serial: &str, name: &str, method: &str) -> DiscoveredDevice {
    let mut device = DiscoveredDevice::new(
        IpAddr::V4(ip.parse().unwrap()),
        80,
        serial.to_string(),
        name.to_string(),
        "Miniserver".to_string(),
        method.to_string(),
    );

    // Add some sample capabilities and metadata
    device.add_capability("http".to_string());
    device.add_capability("loxone-api".to_string());
    device.add_metadata("version".to_string(), "12.3.4.5".to_string());
    device.add_metadata("location".to_string(), "Home Network".to_string());

    if method == "mdns" {
        device.add_capability("mdns".to_string());
        device.add_metadata("mdns_type".to_string(), "_loxone._tcp".to_string());
    } else if method == "upnp" {
        device.add_capability("upnp".to_string());
        device.add_metadata("upnp_type".to_string(), "urn:loxone:device:1".to_string());
    }

    device.response_time = Duration::from_millis(50 + rand::random::<u64>() % 200);

    device
}

// Simple random number generation for demo purposes
mod rand {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    pub fn random<T>() -> T
    where
        T: From<u64>,
    {
        let val = COUNTER.fetch_add(1, Ordering::Relaxed);
        T::from(val.wrapping_mul(1103515245).wrapping_add(12345))
    }
}
