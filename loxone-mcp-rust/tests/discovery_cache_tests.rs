//! Tests for discovery cache functionality

use loxone_mcp_rust::discovery::{
    DiscoveryCache, DiscoveryCacheConfig, DiscoveredDevice, NetworkContext
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

fn create_test_device(ip: &str, serial: &str, name: &str, method: &str) -> DiscoveredDevice {
    DiscoveredDevice::new(
        IpAddr::V4(ip.parse().unwrap()),
        80,
        serial.to_string(),
        name.to_string(),
        "Miniserver".to_string(),
        method.to_string(),
    )
}

fn create_test_config() -> DiscoveryCacheConfig {
    DiscoveryCacheConfig {
        default_ttl: Duration::from_secs(60),
        max_entries: 100,
        cleanup_interval: Duration::from_secs(10),
        min_scan_interval: Duration::from_secs(5),
        enable_persistence: false,
        ..Default::default()
    }
}

#[tokio::test]
async fn test_discovery_cache_creation() {
    let cache = DiscoveryCache::new();
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
}

#[tokio::test]
async fn test_discovery_cache_with_custom_config() {
    let config = create_test_config();
    let cache = DiscoveryCache::with_config(config.clone());
    
    // Verify the cache is initialized properly
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 0);
}

#[tokio::test]
async fn test_discovered_device_creation() {
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    
    assert_eq!(device.ip_address, IpAddr::V4("192.168.1.100".parse().unwrap()));
    assert_eq!(device.port, 80);
    assert_eq!(device.serial, "12345");
    assert_eq!(device.name, "Test Device");
    assert_eq!(device.discovery_method, "mdns");
    assert_eq!(device.device_type, "Miniserver");
}

#[tokio::test]
async fn test_device_capabilities_and_metadata() {
    let mut device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    
    // Add capabilities
    device.add_capability("http".to_string());
    device.add_capability("websocket".to_string());
    device.add_capability("http".to_string()); // Duplicate should be ignored
    
    assert_eq!(device.capabilities.len(), 2);
    assert!(device.has_capability("http"));
    assert!(device.has_capability("websocket"));
    assert!(!device.has_capability("ftp"));
    
    // Add metadata
    device.add_metadata("version".to_string(), "12.3.4.5".to_string());
    device.add_metadata("location".to_string(), "Living Room".to_string());
    
    assert_eq!(device.metadata.len(), 2);
    assert_eq!(device.metadata.get("version"), Some(&"12.3.4.5".to_string()));
    assert_eq!(device.metadata.get("location"), Some(&"Living Room".to_string()));
}

#[tokio::test]
async fn test_device_age_and_staleness() {
    let mut device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    
    // Device should not be stale initially
    assert!(!device.is_stale(Duration::from_secs(60)));
    
    // Make device appear older
    device.last_seen = SystemTime::now() - Duration::from_secs(120);
    
    // Now it should be stale with 60 second max age
    assert!(device.is_stale(Duration::from_secs(60)));
    
    // Update last seen
    device.update_last_seen();
    
    // Should no longer be stale
    assert!(!device.is_stale(Duration::from_secs(60)));
}

#[tokio::test]
async fn test_device_socket_address() {
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let socket_addr = device.socket_addr();
    
    assert_eq!(socket_addr.ip(), IpAddr::V4("192.168.1.100".parse().unwrap()));
    assert_eq!(socket_addr.port(), 80);
}

#[tokio::test]
async fn test_cache_add_and_get_device() {
    let cache = DiscoveryCache::with_config(create_test_config());
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let ip = device.ip_address;
    
    // Add device to cache
    cache.add_device(device.clone()).await.unwrap();
    
    // Retrieve device by IP
    let retrieved = cache.get_device(&ip).await;
    assert!(retrieved.is_some());
    let retrieved_device = retrieved.unwrap();
    assert_eq!(retrieved_device.serial, "12345");
    assert_eq!(retrieved_device.name, "Test Device");
    
    // Check statistics
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.cache_hits, 1);
    assert_eq!(stats.cache_misses, 0);
}

#[tokio::test]
async fn test_cache_get_device_by_serial() {
    let cache = DiscoveryCache::with_config(create_test_config());
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    
    cache.add_device(device.clone()).await.unwrap();
    
    // Retrieve device by serial
    let retrieved = cache.get_device_by_serial("12345").await;
    assert!(retrieved.is_some());
    let retrieved_device = retrieved.unwrap();
    assert_eq!(retrieved_device.ip_address.to_string(), "192.168.1.100");
    assert_eq!(retrieved_device.name, "Test Device");
}

#[tokio::test]
async fn test_cache_miss() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Try to get non-existent device
    let missing_ip = IpAddr::V4("192.168.1.200".parse().unwrap());
    let result = cache.get_device(&missing_ip).await;
    assert!(result.is_none());
    
    // Check miss statistics
    let stats = cache.get_statistics().await;
    assert_eq!(stats.cache_misses, 1);
    assert_eq!(stats.cache_hits, 0);
}

#[tokio::test]
async fn test_cache_get_all_devices() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Add multiple devices
    let devices = vec![
        create_test_device("192.168.1.100", "12345", "Device 1", "mdns"),
        create_test_device("192.168.1.101", "12346", "Device 2", "upnp"),
        create_test_device("192.168.1.102", "12347", "Device 3", "network_scan"),
    ];
    
    for device in &devices {
        cache.add_device(device.clone()).await.unwrap();
    }
    
    // Get all devices
    let all_devices = cache.get_all_devices().await;
    assert_eq!(all_devices.len(), 3);
    
    // Verify device details
    let serials: Vec<String> = all_devices.iter().map(|d| d.serial.clone()).collect();
    assert!(serials.contains(&"12345".to_string()));
    assert!(serials.contains(&"12346".to_string()));
    assert!(serials.contains(&"12347".to_string()));
}

#[tokio::test]
async fn test_cache_get_devices_by_method() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Add devices with different discovery methods
    let devices = vec![
        create_test_device("192.168.1.100", "12345", "Device 1", "mdns"),
        create_test_device("192.168.1.101", "12346", "Device 2", "mdns"),
        create_test_device("192.168.1.102", "12347", "Device 3", "upnp"),
        create_test_device("192.168.1.103", "12348", "Device 4", "network_scan"),
    ];
    
    for device in &devices {
        cache.add_device(device.clone()).await.unwrap();
    }
    
    // Get devices by method
    let mdns_devices = cache.get_devices_by_method("mdns").await;
    assert_eq!(mdns_devices.len(), 2);
    
    let upnp_devices = cache.get_devices_by_method("upnp").await;
    assert_eq!(upnp_devices.len(), 1);
    
    let network_scan_devices = cache.get_devices_by_method("network_scan").await;
    assert_eq!(network_scan_devices.len(), 1);
    
    let nonexistent_devices = cache.get_devices_by_method("bluetooth").await;
    assert_eq!(nonexistent_devices.len(), 0);
}

#[tokio::test]
async fn test_cache_expiration() {
    let config = DiscoveryCacheConfig {
        default_ttl: Duration::from_millis(50),
        ..create_test_config()
    };
    let cache = DiscoveryCache::with_config(config);
    
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let ip = device.ip_address;
    
    // Add device
    cache.add_device(device).await.unwrap();
    
    // Should be available immediately
    assert!(cache.get_device(&ip).await.is_some());
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Should be expired now
    assert!(cache.get_device(&ip).await.is_none());
}

#[tokio::test]
async fn test_cache_cleanup_expired() {
    let config = DiscoveryCacheConfig {
        default_ttl: Duration::from_millis(10),
        ..create_test_config()
    };
    let cache = DiscoveryCache::with_config(config);
    
    // Add multiple devices
    for i in 0..5 {
        let device = create_test_device(
            &format!("192.168.1.{}", 100 + i),
            &format!("serial{}", i),
            &format!("Device {}", i),
            "test"
        );
        cache.add_device(device).await.unwrap();
    }
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Cleanup should remove all expired entries
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 5);
    
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 0);
}

#[tokio::test]
async fn test_cache_cleanup_stale_devices() {
    let config = DiscoveryCacheConfig {
        max_device_age: Duration::from_millis(50),
        ..create_test_config()
    };
    let cache = DiscoveryCache::with_config(config);
    
    // Add devices with different ages
    let mut old_device = create_test_device("192.168.1.100", "old123", "Old Device", "test");
    old_device.last_seen = SystemTime::now() - Duration::from_millis(100); // Make it stale
    
    let fresh_device = create_test_device("192.168.1.101", "fresh123", "Fresh Device", "test");
    
    cache.add_device(old_device).await.unwrap();
    cache.add_device(fresh_device).await.unwrap();
    
    // Cleanup stale devices
    let removed = cache.cleanup_stale_devices().await.unwrap();
    assert_eq!(removed, 1); // Only the old device should be removed
    
    let remaining_devices = cache.get_all_devices().await;
    assert_eq!(remaining_devices.len(), 1);
    assert_eq!(remaining_devices[0].serial, "fresh123");
}

#[tokio::test]
async fn test_full_scan_tracking() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Initially should need full scan
    assert!(cache.needs_full_scan().await);
    
    // Mark scan as completed
    cache.mark_full_scan_completed().await;
    
    // Should not need scan immediately after completion
    assert!(!cache.needs_full_scan().await);
}

#[tokio::test]
async fn test_cache_clear() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Add several devices
    for i in 1..=5 {
        let device = create_test_device(
            &format!("192.168.1.{}", 100 + i),
            &format!("serial{}", i),
            &format!("Device {}", i),
            "test"
        );
        cache.add_device(device).await.unwrap();
    }
    
    // Verify devices were added
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 5);
    
    // Clear cache
    let cleared_count = cache.clear().await.unwrap();
    assert_eq!(cleared_count, 5);
    
    // Verify cache is empty
    let final_stats = cache.get_statistics().await;
    assert_eq!(final_stats.total_entries, 0);
    
    let all_devices = cache.get_all_devices().await;
    assert_eq!(all_devices.len(), 0);
}

#[tokio::test]
async fn test_cache_statistics() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Initial statistics
    let initial_stats = cache.get_statistics().await;
    assert_eq!(initial_stats.total_entries, 0);
    assert_eq!(initial_stats.cache_hits, 0);
    assert_eq!(initial_stats.cache_misses, 0);
    assert_eq!(initial_stats.total_discoveries, 0);
    
    // Add a device
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let ip = device.ip_address;
    cache.add_device(device).await.unwrap();
    
    // Generate some hits and misses
    cache.get_device(&ip).await; // Hit
    cache.get_device(&IpAddr::V4("192.168.1.200".parse().unwrap())).await; // Miss
    cache.get_device(&ip).await; // Hit
    
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.cache_hits, 2);
    assert_eq!(stats.cache_misses, 1);
    assert_eq!(stats.total_discoveries, 1);
    assert_eq!(stats.hit_ratio, 2.0 / 3.0);
    
    // Check devices by method
    assert!(stats.devices_by_method.contains_key("mdns"));
    assert_eq!(stats.devices_by_method.get("mdns"), Some(&1));
}

#[tokio::test]
async fn test_cache_start_stop() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    // Start the cache
    let start_result = cache.start().await;
    assert!(start_result.is_ok());
    
    // Stop the cache
    let stop_result = cache.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_network_context() {
    let context = NetworkContext::current();
    
    // Verify context has timestamp
    assert!(context.captured_at <= SystemTime::now());
    
    // Should have empty vectors in our simple implementation
    assert_eq!(context.local_ips.len(), 0);
    assert_eq!(context.interfaces.len(), 0);
}

#[tokio::test]
async fn test_cache_entry_ttl() {
    use loxone_mcp_rust::discovery::CacheEntry;
    
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let ttl = Duration::from_millis(100);
    let entry = CacheEntry::new(device, ttl);
    
    // Should not be expired immediately
    assert!(!entry.is_expired());
    assert!(entry.remaining_ttl() > Duration::from_millis(50));
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Should be expired now
    assert!(entry.is_expired());
    assert_eq!(entry.remaining_ttl(), Duration::from_secs(0));
}

#[tokio::test]
async fn test_cache_access_count() {
    use loxone_mcp_rust::discovery::CacheEntry;
    
    let device = create_test_device("192.168.1.100", "12345", "Test Device", "mdns");
    let mut entry = CacheEntry::new(device, Duration::from_secs(60));
    
    // Initial access count should be 0
    assert_eq!(entry.access_count, 0);
    
    // Increment access count
    entry.increment_access();
    assert_eq!(entry.access_count, 1);
    
    entry.increment_access();
    assert_eq!(entry.access_count, 2);
}

#[tokio::test]
async fn test_cache_method_specific_ttls() {
    let mut method_ttls = HashMap::new();
    method_ttls.insert("mdns".to_string(), Duration::from_secs(300));
    method_ttls.insert("upnp".to_string(), Duration::from_secs(600));
    
    let config = DiscoveryCacheConfig {
        method_ttls,
        default_ttl: Duration::from_secs(900),
        ..create_test_config()
    };
    
    let cache = DiscoveryCache::with_config(config);
    
    // Add devices with different methods
    let mdns_device = create_test_device("192.168.1.100", "12345", "mDNS Device", "mdns");
    let upnp_device = create_test_device("192.168.1.101", "12346", "UPnP Device", "upnp");
    let unknown_device = create_test_device("192.168.1.102", "12347", "Unknown Device", "bluetooth");
    
    cache.add_device(mdns_device).await.unwrap();
    cache.add_device(upnp_device).await.unwrap();
    cache.add_device(unknown_device).await.unwrap();
    
    // All devices should be cached
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 3);
}

#[tokio::test]
async fn test_cache_with_duplicate_devices() {
    let cache = DiscoveryCache::with_config(create_test_config());
    
    let device1 = create_test_device("192.168.1.100", "12345", "Device Original", "mdns");
    let mut device2 = create_test_device("192.168.1.100", "12345", "Device Updated", "mdns");
    device2.firmware_version = "new_version".to_string();
    
    // Add first device
    cache.add_device(device1).await.unwrap();
    
    // Add second device with same IP and serial (should update existing)
    cache.add_device(device2).await.unwrap();
    
    // Should still have only one entry
    let stats = cache.get_statistics().await;
    assert_eq!(stats.total_entries, 1);
    
    // Should be the updated device
    let ip = IpAddr::V4("192.168.1.100".parse().unwrap());
    let retrieved = cache.get_device(&ip).await.unwrap();
    assert_eq!(retrieved.name, "Device Updated");
    assert_eq!(retrieved.firmware_version, "new_version");
}