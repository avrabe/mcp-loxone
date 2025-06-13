//! Proper mDNS/Zeroconf discovery implementation for Loxone Miniservers
//!
//! This module implements real mDNS discovery using the mdns crate
//! to find Loxone Miniservers on the local network.

use crate::error::{LoxoneError, Result};
use crate::network_discovery::DiscoveredServer;
use mdns::{Record, RecordKind};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{debug, info};

/// Known service types that Loxone Miniservers might advertise
const LOXONE_SERVICE_TYPES: &[&str] = &[
    "_loxone._tcp.local",
    "_loxone-miniserver._tcp.local",
    "_http._tcp.local",
    "_loxone-ws._tcp.local",
];

/// Perform mDNS discovery for Loxone Miniservers
pub async fn discover_via_mdns(timeout: Duration) -> Result<Vec<DiscoveredServer>> {
    info!("Starting mDNS/Zeroconf discovery...");

    let mut discovered_servers = Vec::new();
    let mut seen_ips = HashSet::new();

    // Try each service type
    for service_type in LOXONE_SERVICE_TYPES {
        debug!("Searching for service type: {}", service_type);

        match discover_service_type(service_type, timeout).await {
            Ok(servers) => {
                for server in servers {
                    // Avoid duplicates by IP
                    if seen_ips.insert(server.ip.clone()) {
                        info!("Found Loxone via mDNS: {} at {}", server.name, server.ip);
                        discovered_servers.push(server);
                    }
                }
            }
            Err(e) => {
                debug!("Failed to discover {}: {}", service_type, e);
            }
        }
    }

    Ok(discovered_servers)
}

/// Discover servers advertising a specific service type
async fn discover_service_type(
    service_type: &str,
    timeout: Duration,
) -> Result<Vec<DiscoveredServer>> {
    let mut servers = Vec::new();

    // Create mDNS discovery instance
    let discovery = mdns::discover::all(service_type, timeout)
        .map_err(|e| LoxoneError::discovery(format!("mDNS discovery failed: {}", e)))?;

    // Get the async stream
    let stream = discovery.listen();

    // Use tokio timeout and collect responses
    let results = tokio::time::timeout(timeout, collect_mdns_responses(stream, service_type))
        .await
        .unwrap_or_else(|_| Ok(Vec::new()))?;

    servers.extend(results);

    Ok(servers)
}

/// Collect mDNS responses from the stream
async fn collect_mdns_responses(
    stream: impl futures_util::Stream<Item = std::result::Result<mdns::Response, mdns::Error>>,
    service_type: &str,
) -> Result<Vec<DiscoveredServer>> {
    use futures_util::StreamExt;

    let mut servers = Vec::new();
    let mut seen_ips = HashSet::new();

    futures_util::pin_mut!(stream);

    while let Some(response_result) = stream.next().await {
        match response_result {
            Ok(response) => {
                debug!("Got mDNS response");

                // Process each record in the response
                for record in response.records() {
                    if let Some(server) = process_mdns_record(&response, record, service_type) {
                        // Avoid duplicates
                        if seen_ips.insert(server.ip.clone()) {
                            servers.push(server);
                        }
                    }
                }
            }
            Err(e) => {
                // Ignore non-ASCII label errors from other devices (e.g., devices with Unicode names)
                if !e.to_string().contains("LabelIsNotAscii") {
                    debug!("mDNS response error: {}", e);
                }
            }
        }
    }

    Ok(servers)
}

/// Process a single mDNS record
fn process_mdns_record(
    response: &mdns::Response,
    record: &Record,
    service_type: &str,
) -> Option<DiscoveredServer> {
    match &record.kind {
        RecordKind::A(ip) => {
            if is_likely_loxone(&record.name, service_type) {
                Some(DiscoveredServer {
                    ip: ip.to_string(),
                    name: extract_name_from_mdns(response, record),
                    port: extract_port_from_mdns(response).unwrap_or_else(|| "80".to_string()),
                    method: "mDNS/Zeroconf".to_string(),
                    service_type: Some(service_type.to_string()),
                    service_name: Some(record.name.clone()),
                })
            } else {
                None
            }
        }
        RecordKind::AAAA(ipv6) => {
            if is_likely_loxone(&record.name, service_type) {
                Some(DiscoveredServer {
                    ip: ipv6.to_string(),
                    name: extract_name_from_mdns(response, record),
                    port: extract_port_from_mdns(response).unwrap_or_else(|| "80".to_string()),
                    method: "mDNS/Zeroconf".to_string(),
                    service_type: Some(service_type.to_string()),
                    service_name: Some(record.name.clone()),
                })
            } else {
                None
            }
        }
        RecordKind::PTR(ptr_name) => {
            // PTR records point to service instances
            debug!("Found PTR record pointing to: {}", ptr_name);

            // Look for corresponding A/AAAA records
            for other_record in response.records() {
                if other_record.name == *ptr_name {
                    match &other_record.kind {
                        RecordKind::A(ip) => {
                            return Some(DiscoveredServer {
                                ip: ip.to_string(),
                                name: extract_name_from_mdns(response, record),
                                port: extract_port_from_mdns(response)
                                    .unwrap_or_else(|| "80".to_string()),
                                method: "mDNS/Zeroconf".to_string(),
                                service_type: Some(service_type.to_string()),
                                service_name: Some(ptr_name.clone()),
                            });
                        }
                        RecordKind::AAAA(ipv6) => {
                            return Some(DiscoveredServer {
                                ip: ipv6.to_string(),
                                name: extract_name_from_mdns(response, record),
                                port: extract_port_from_mdns(response)
                                    .unwrap_or_else(|| "80".to_string()),
                                method: "mDNS/Zeroconf".to_string(),
                                service_type: Some(service_type.to_string()),
                                service_name: Some(ptr_name.clone()),
                            });
                        }
                        _ => {}
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if a service name is likely a Loxone device
fn is_likely_loxone(name: &str, service_type: &str) -> bool {
    let name_lower = name.to_lowercase();

    // Direct service type match
    if service_type.contains("loxone") {
        return true;
    }

    // Name-based detection
    let loxone_indicators = [
        "loxone",
        "miniserver",
        "lox",
        // Common Loxone hostname patterns
        "/a",
        "/b",
        "/c",
        "/d",
        "/e",
        "/f", // Single letter IDs
    ];

    loxone_indicators
        .iter()
        .any(|indicator| name_lower.contains(indicator))
}

/// Extract a friendly name from mDNS response
fn extract_name_from_mdns(response: &mdns::Response, record: &Record) -> String {
    // Try to find TXT records with name information
    for r in response.records() {
        if let RecordKind::TXT(txt_records) = &r.kind {
            for txt in txt_records {
                if let Some(stripped) = txt.strip_prefix("name=") {
                    return stripped.to_string();
                }
                if let Some(stripped) = txt.strip_prefix("friendlyName=") {
                    return stripped.to_string();
                }
                if let Some(stripped) = txt.strip_prefix("fn=") {
                    return stripped.to_string();
                }
            }
        }
    }

    // Parse hostname for Loxone pattern (e.g., "Beier/A5")
    if let Some(hostname) = parse_loxone_hostname(&record.name) {
        return hostname;
    }

    // Default to record name or generic
    if record.name.contains("loxone") || record.name.contains("miniserver") {
        record.name.clone()
    } else {
        "Loxone Miniserver".to_string()
    }
}

/// Parse Loxone hostname pattern (Owner/ID)
fn parse_loxone_hostname(hostname: &str) -> Option<String> {
    // Remove .local suffix if present
    let name = hostname.trim_end_matches(".local");

    // Check for Owner/ID pattern
    if name.contains('/') {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(name.to_string());
        }
    }

    // Check for Owner-ID pattern (alternative format)
    if name.contains('-') && name.matches('-').count() == 1 {
        let parts: Vec<&str> = name.split('-').collect();
        if parts.len() == 2 && !parts[0].is_empty() && parts[1].len() <= 3 {
            // Convert back to slash format
            return Some(format!("{}/{}", parts[0], parts[1]));
        }
    }

    None
}

/// Extract port from mDNS response
fn extract_port_from_mdns(response: &mdns::Response) -> Option<String> {
    // Look for SRV records which contain port information
    for record in response.records() {
        if let RecordKind::SRV { port, .. } = &record.kind {
            return Some(port.to_string());
        }
    }

    // Look in TXT records
    for record in response.records() {
        if let RecordKind::TXT(txt_records) = &record.kind {
            for txt in txt_records {
                if let Some(stripped) = txt.strip_prefix("port=") {
                    return Some(stripped.to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_loxone_hostname() {
        assert_eq!(
            parse_loxone_hostname("Beier/A5.local"),
            Some("Beier/A5".to_string())
        );
        assert_eq!(
            parse_loxone_hostname("Beier-A5.local"),
            Some("Beier/A5".to_string())
        );
        assert_eq!(
            parse_loxone_hostname("Smith/B1"),
            Some("Smith/B1".to_string())
        );
        assert_eq!(parse_loxone_hostname("regular-hostname"), None);
    }

    #[test]
    fn test_is_likely_loxone() {
        assert!(is_likely_loxone("loxone-miniserver", "_http._tcp.local"));
        assert!(is_likely_loxone("Beier/A5", "_http._tcp.local"));
        assert!(is_likely_loxone("miniserver.local", "_http._tcp.local"));
        assert!(is_likely_loxone("anything", "_loxone._tcp.local"));
        assert!(!is_likely_loxone("router", "_http._tcp.local"));
    }
}
