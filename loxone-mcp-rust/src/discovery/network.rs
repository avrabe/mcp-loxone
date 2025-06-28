//! Network discovery for Loxone Miniservers
//!
//! This module provides multiple discovery methods:
//! 1. mDNS/Zeroconf discovery (most accurate)
//! 2. UDP broadcast discovery (Loxone-specific protocol)
//! 3. HTTP endpoint scanning (network scan fallback)

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};
use tracing::{debug, info};

/// Discovered Loxone server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub ip: String,
    pub name: String,
    pub port: String,
    pub method: String,
    pub service_type: Option<String>,
    pub service_name: Option<String>,
}

/// Network discovery client for Loxone servers
pub struct NetworkDiscovery {
    timeout: Duration,
}

impl NetworkDiscovery {
    /// Create a new network discovery client
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Discover Loxone servers using multiple methods
    pub async fn discover_servers(&self) -> Result<Vec<DiscoveredServer>> {
        let mut servers = Vec::new();

        info!("🔍 Discovering Loxone Miniservers on your network...");

        // Method 1: mDNS/Zeroconf Discovery (most accurate)
        #[cfg(all(feature = "discovery", feature = "mdns"))]
        {
            info!("   • Trying mDNS/zeroconf discovery...");
            match self.mdns_discovery().await {
                Ok(mdns_servers) if !mdns_servers.is_empty() => {
                    info!("✅ Found {} server(s)", mdns_servers.len());
                    servers.extend(mdns_servers);
                }
                Ok(_) => info!("⏭️  No mDNS announcements"),
                Err(e) => {
                    debug!("mDNS discovery error: {}", e);
                    info!("⏭️  mDNS discovery failed");
                }
            }
        }

        // Method 2: UDP Discovery (Loxone specific protocol)
        info!("   • Trying UDP discovery...");
        match self.udp_discovery().await {
            Ok(udp_servers) if !udp_servers.is_empty() => {
                info!("✅ Found {} server(s)", udp_servers.len());
                // Merge results, avoiding duplicates
                for server in udp_servers {
                    if !servers.iter().any(|s: &DiscoveredServer| s.ip == server.ip) {
                        servers.push(server);
                    }
                }
            }
            Ok(_) => info!("⏭️  No UDP response"),
            Err(e) => {
                debug!("UDP discovery error: {}", e);
                info!("⏭️  UDP discovery failed");
            }
        }

        // Method 3: Network scan for HTTP endpoints
        info!("   • Scanning network for HTTP endpoints...");
        match self.http_discovery().await {
            Ok(http_servers) => {
                let new_servers: Vec<_> = http_servers
                    .into_iter()
                    .filter(|server| !servers.iter().any(|s| s.ip == server.ip))
                    .collect();

                if !new_servers.is_empty() {
                    info!("✅ Found {} additional server(s)", new_servers.len());
                } else if servers.is_empty() {
                    info!("❌ No servers found");
                } else {
                    info!("⏭️  No additional servers");
                }

                servers.extend(new_servers);
            }
            Err(e) => {
                debug!("HTTP discovery error: {}", e);
                info!("⏭️  HTTP scan failed");
            }
        }

        // Sort servers by IP for consistent ordering
        servers.sort_by(|a, b| {
            let ip_a: Vec<u8> = a.ip.split('.').map(|s| s.parse().unwrap_or(0)).collect();
            let ip_b: Vec<u8> = b.ip.split('.').map(|s| s.parse().unwrap_or(0)).collect();
            ip_a.cmp(&ip_b)
        });

        Ok(servers)
    }

    /// Discover Loxone servers using mDNS/Zeroconf
    #[cfg(all(feature = "discovery", feature = "mdns"))]
    async fn mdns_discovery(&self) -> Result<Vec<DiscoveredServer>> {
        super::mdns::discover_via_mdns(self.timeout).await
    }

    /// Fallback when mDNS feature is not available
    #[cfg(not(all(feature = "discovery", feature = "mdns")))]
    #[allow(dead_code)]
    async fn mdns_discovery(&self) -> Result<Vec<DiscoveredServer>> {
        debug!("mDNS discovery not available (feature disabled)");
        Ok(vec![])
    }

    /// Discover Loxone servers using UDP broadcast
    async fn udp_discovery(&self) -> Result<Vec<DiscoveredServer>> {
        let mut servers = Vec::new();

        // Create UDP socket for discovery
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(self.timeout))?;

        // Loxone discovery messages (varies by version, try common ones)
        let discovery_messages = [
            b"LoxLIVE".as_slice(),          // Common discovery message
            b"eWeLink".as_slice(),          // Alternative discovery
            b"\x00\x00\x00\x00".as_slice(), // Simple broadcast
        ];

        // Send discovery packets to configurable ports
        let ports_env = std::env::var("DISCOVERY_UDP_PORTS")
            .ok()
            .and_then(|p| serde_json::from_str::<Vec<u16>>(&p).ok())
            .unwrap_or_else(|| vec![7777, 7700, 80, 8080]);
        let broadcast_base = std::env::var("DISCOVERY_BROADCAST_ADDRESS")
            .unwrap_or_else(|_| "255.255.255.255".to_string());

        for port in &ports_env {
            for message in &discovery_messages {
                let broadcast_addr = format!("{broadcast_base}:{port}");
                if let Ok(addr) = broadcast_addr.parse::<SocketAddr>() {
                    let _ = socket.send_to(message, addr);
                }
            }
        }

        // Listen for responses
        let start = std::time::Instant::now();
        let mut buffer = [0u8; 1024];
        let mut responded_ips = std::collections::HashSet::new();

        while start.elapsed() < self.timeout {
            match socket.recv_from(&mut buffer) {
                Ok((len, addr)) => {
                    let ip = addr.ip().to_string();
                    if responded_ips.insert(ip.clone()) {
                        // New response from this IP
                        let data = &buffer[..len];
                        let server_name =
                            parse_udp_response(data).unwrap_or("Loxone Miniserver".to_string());

                        servers.push(DiscoveredServer {
                            ip,
                            name: server_name,
                            port: "80".to_string(),
                            method: "UDP Discovery".to_string(),
                            service_type: None,
                            service_name: None,
                        });
                    }
                }
                Err(_) => {
                    // Timeout or no more responses
                    break;
                }
            }
        }

        Ok(servers)
    }

    /// Discover Loxone servers by scanning network for HTTP endpoints
    async fn http_discovery(&self) -> Result<Vec<DiscoveredServer>> {
        let mut servers = Vec::new();

        // Get local network range
        let local_ip = get_local_ip()?;
        let network_prefix = extract_network_prefix(&local_ip);

        debug!("Scanning network: {}.x", network_prefix);

        // Create HTTP client with short timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;

        // Scan common IP ranges (prioritize common router/device IPs)
        let mut tasks = Vec::new();
        let priority_ips = [1, 2, 10, 100, 101, 102, 200, 201, 202];

        // Check priority IPs first
        for &ip in &priority_ips {
            let ip_addr = format!("{network_prefix}.{ip}");
            let task = check_loxone_http(&client, ip_addr);
            tasks.push(task);
        }

        // Then scan broader range
        for ip in 3..255 {
            if !priority_ips.contains(&ip) {
                let ip_addr = format!("{network_prefix}.{ip}");
                let task = check_loxone_http(&client, ip_addr);
                tasks.push(task);
            }
        }

        // Execute all requests concurrently with timeout
        match tokio::time::timeout(self.timeout, futures::future::join_all(tasks)).await {
            Ok(results) => {
                for server in results.into_iter().flatten().flatten() {
                    servers.push(server);
                }
            }
            Err(_) => {
                debug!("HTTP discovery timed out");
            }
        }

        Ok(servers)
    }

    /// Test connection to a discovered server
    pub async fn test_connection(
        &self,
        host: &str,
        username: &str,
        password: &str,
    ) -> Result<HashMap<String, String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let url = format!("http://{host}/data/LoxAPP3.json");
        let response = client
            .get(&url)
            .basic_auth(username, Some(password))
            .send()
            .await?;

        if response.status().is_success() {
            let data: serde_json::Value = response.json().await?;
            let mut info = HashMap::new();

            if let Some(ms_info) = data.get("msInfo") {
                info.insert(
                    "name".to_string(),
                    ms_info
                        .get("projectName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                );
                info.insert(
                    "version".to_string(),
                    ms_info
                        .get("swVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                );
            }

            Ok(info)
        } else if response.status() == 401 {
            Err(LoxoneError::credentials(
                "Invalid username or password".to_string(),
            ))
        } else {
            Err(LoxoneError::credentials(format!(
                "HTTP {}",
                response.status()
            )))
        }
    }
}

/// Check if an IP address hosts a Loxone Miniserver via HTTP
async fn check_loxone_http(
    client: &reqwest::Client,
    ip: String,
) -> Result<Option<DiscoveredServer>> {
    let url = format!("http://{ip}/");

    match client.get(&url).send().await {
        Ok(response) if response.status() == 401 || response.status().is_success() => {
            // 401 indicates Loxone authentication required, which is good
            let mut name = "Loxone Miniserver".to_string();
            let mut version = "Unknown".to_string();

            // Try to get version info without auth
            if let Ok(version_response) = client
                .get(format!("http://{ip}/jdev/sys/getversion"))
                .send()
                .await
            {
                if version_response.status().is_success() {
                    if let Ok(data) = version_response.json::<serde_json::Value>().await {
                        if let Some(v) = data
                            .get("LL")
                            .and_then(|ll| ll.get("value"))
                            .and_then(|v| v.as_str())
                        {
                            version = v.to_string();
                        }
                    }
                }
            }

            // Try to get project name (might require auth)
            if let Ok(cfg_response) = client.get(format!("http://{ip}/jdev/cfg/api")).send().await {
                if cfg_response.status().is_success() {
                    if let Ok(data) = cfg_response.json::<serde_json::Value>().await {
                        if let Some(project_name) = data
                            .get("LL")
                            .and_then(|ll| ll.get("value"))
                            .and_then(|v| v.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            name = project_name.to_string();
                        }
                    }
                }
            }

            let display_name = if version != "Unknown" {
                format!("{name} (v{version})")
            } else {
                name
            };

            Ok(Some(DiscoveredServer {
                ip,
                name: display_name,
                port: "80".to_string(),
                method: "HTTP Scan".to_string(),
                service_type: None,
                service_name: None,
            }))
        }
        _ => Ok(None),
    }
}

/// Get local IP address for network scanning
fn get_local_ip() -> Result<String> {
    use socket2::{Domain, Socket, Type};
    use std::net::SocketAddr;

    // Use configurable DNS server for connectivity check
    let dns_server =
        std::env::var("DISCOVERY_DNS_SERVER").unwrap_or_else(|_| "8.8.8.8".to_string());
    let dns_port = std::env::var("DISCOVERY_DNS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80);
    let connect_addr = format!("{dns_server}:{dns_port}");

    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
    socket.connect(&connect_addr.parse::<SocketAddr>().unwrap().into())?;
    let local_addr = socket.local_addr()?;

    if let Some(addr) = local_addr.as_socket_ipv4() {
        Ok(addr.ip().to_string())
    } else {
        Err(LoxoneError::discovery(
            "Failed to get local IP address".to_string(),
        ))
    }
}

/// Extract network prefix from IP (e.g., "192.168.1.100" -> "192.168.1")
fn extract_network_prefix(ip: &str) -> String {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() >= 3 {
        format!("{}.{}.{}", parts[0], parts[1], parts[2])
    } else {
        "192.168.1".to_string() // fallback
    }
}

/// Parse UDP discovery response
fn parse_udp_response(data: &[u8]) -> Option<String> {
    // Try to parse as JSON first
    if data.starts_with(b"{") {
        if let Ok(text) = std::str::from_utf8(data) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }

    // Default name if parsing fails
    None
}
