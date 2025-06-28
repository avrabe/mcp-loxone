//! Modern mDNS/Zeroconf discovery implementation for Loxone Miniservers
//!
//! This module implements real mDNS discovery using the mdns-sd crate
//! to find Loxone Miniservers on the local network.

#![cfg(feature = "mdns")]

use super::network::DiscoveredServer;
use crate::error::{LoxoneError, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, TxtProperties};
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Perform mDNS discovery for Loxone Miniservers
pub async fn discover_via_mdns(timeout: Duration) -> Result<Vec<DiscoveredServer>> {
    debug!("Starting mDNS discovery for Loxone Miniservers...");

    // Create the mDNS daemon
    let mdns = ServiceDaemon::new()
        .map_err(|e| LoxoneError::discovery(format!("Failed to create mDNS daemon: {e}")))?;

    let mut servers = Vec::new();
    let mut discovered_hosts = HashMap::new();

    // Use a channel to collect results from multiple service type searches
    let (tx, rx) = mpsc::channel();

    // Loxone service types to search for
    let loxone_service_types = [
        "_loxone._tcp.local.",     // Primary Loxone service type
        "_http._tcp.local.",       // HTTP services (broader search)
        "_https._tcp.local.",      // HTTPS services
        "_miniserver._tcp.local.", // Loxone Miniserver specific
    ];

    for service_type in &loxone_service_types {
        debug!("Browsing for service type: {}", service_type);

        // Browse for services of this type
        let receiver = mdns.browse(service_type).map_err(|e| {
            LoxoneError::discovery(format!("Failed to browse for {service_type}: {e}"))
        })?;

        let tx_clone = tx.clone();
        let service_type_owned = service_type.to_string();

        // Spawn a task to handle events for this service type
        tokio::task::spawn_blocking(move || {
            let start_time = std::time::Instant::now();

            while start_time.elapsed() < timeout {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        let _ = tx_clone.send((service_type_owned.clone(), event));
                    }
                    Err(_) => {
                        // Timeout or error - continue to check overall timeout
                        continue;
                    }
                }
            }
        });
    }

    // Drop the original sender so the receiver will get disconnected when all tasks finish
    drop(tx);

    // Collect events with overall timeout
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok((service_type, event)) => {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        debug!(
                            "Discovered service: {} at {}:{}",
                            info.get_fullname(),
                            info.get_addresses()
                                .iter()
                                .next()
                                .unwrap_or(&std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
                            info.get_port()
                        );

                        // Check if this looks like a Loxone service
                        if is_likely_loxone_service(info.get_fullname(), info.get_properties()) {
                            let addr = if let Some(ip) = info.get_addresses().iter().next() {
                                ip.to_string()
                            } else {
                                continue;
                            };

                            let key = format!("{}:{}", addr, info.get_port());

                            // Avoid duplicates from multiple service types
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                discovered_hosts.entry(key)
                            {
                                let server = DiscoveredServer {
                                    ip: addr,
                                    name: extract_device_name(
                                        info.get_fullname(),
                                        info.get_properties(),
                                    ),
                                    port: info.get_port().to_string(),
                                    method: "mDNS".to_string(),
                                    service_type: Some(service_type.clone()),
                                    service_name: Some(info.get_fullname().to_string()),
                                };

                                e.insert(server.clone());
                                servers.push(server);

                                info!(
                                    "Found Loxone device via mDNS: {} at {}:{}",
                                    info.get_fullname(),
                                    info.get_addresses().iter().next().unwrap(),
                                    info.get_port()
                                );
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(name, service_type) => {
                        debug!("Service removed: {} ({})", name, service_type);
                    }
                    ServiceEvent::SearchStarted(service_type) => {
                        debug!("Search started for: {}", service_type);
                    }
                    ServiceEvent::SearchStopped(service_type) => {
                        debug!("Search stopped for: {}", service_type);
                    }
                    ServiceEvent::ServiceFound(name, service_type) => {
                        debug!("Service found: {} ({})", name, service_type);
                    }
                }
            }
            Err(_) => {
                // Timeout or channel disconnected
                continue;
            }
        }
    }

    // Stop browsing for all service types
    for service_type in &loxone_service_types {
        if let Err(e) = mdns.stop_browse(service_type) {
            warn!("Failed to stop browsing for {}: {}", service_type, e);
        }
    }

    // Shutdown the mDNS daemon
    if let Err(e) = mdns.shutdown() {
        warn!("Failed to shutdown mDNS daemon: {}", e);
    }

    info!(
        "mDNS discovery completed, found {} unique devices",
        servers.len()
    );
    Ok(servers)
}

/// Check if a discovered service looks like a Loxone device
fn is_likely_loxone_service(fullname: &str, properties: &TxtProperties) -> bool {
    let fullname_lower = fullname.to_lowercase();

    // Direct Loxone service type
    if fullname_lower.contains("loxone") || fullname_lower.contains("miniserver") {
        return true;
    }

    // Check properties for Loxone-specific indicators
    for property in properties.iter() {
        let key_lower = property.key().to_lowercase();
        if let Some(value_bytes) = property.val() {
            let value_lower = String::from_utf8_lossy(value_bytes).to_lowercase();

            if key_lower.contains("vendor") && value_lower.contains("loxone") {
                return true;
            }
            if key_lower.contains("model")
                && (value_lower.contains("miniserver") || value_lower.contains("loxone"))
            {
                return true;
            }
            if key_lower.contains("name")
                && (value_lower.contains("miniserver") || value_lower.contains("loxone"))
            {
                return true;
            }
        }
    }

    // For HTTP services, we'll let them through and filter later
    // since Loxone devices might advertise as generic HTTP services
    if fullname_lower.contains("_http._tcp") {
        return true;
    }

    false
}

/// Extract a meaningful device name from mDNS service info
fn extract_device_name(fullname: &str, properties: &TxtProperties) -> String {
    // Try to get name from properties first
    for property in properties.iter() {
        let key_lower = property.key().to_lowercase();
        if key_lower == "name" || key_lower == "device_name" || key_lower == "friendly_name" {
            if let Some(value_bytes) = property.val() {
                let value_str = String::from_utf8_lossy(value_bytes);
                if !value_str.is_empty() {
                    return value_str.to_string();
                }
            }
        }
    }

    // Extract from service name
    if let Some(service_name) = fullname.split('.').next() {
        if !service_name.is_empty() {
            return service_name.to_string();
        }
    }

    // Default fallback
    "Loxone Miniserver".to_string()
}
