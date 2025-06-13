//! Input validation and sanitization for security
//!
//! This module provides comprehensive input validation to prevent
//! injection attacks and ensure data integrity.

use crate::error::{LoxoneError, Result};
use regex::Regex;
use std::sync::OnceLock;

/// Standard UUID validation regex
static UUID_REGEX: OnceLock<Regex> = OnceLock::new();

/// Loxone UUID validation regex (format like "0CD8C06B.855703.I2")
static LOXONE_UUID_REGEX: OnceLock<Regex> = OnceLock::new();

/// IP address validation regex
static IP_REGEX: OnceLock<Regex> = OnceLock::new();

/// IPv6 address validation regex
static IPV6_REGEX: OnceLock<Regex> = OnceLock::new();

/// MAC address validation regex
static MAC_REGEX: OnceLock<Regex> = OnceLock::new();

/// Port number validation regex
static PORT_REGEX: OnceLock<Regex> = OnceLock::new();

/// Alphanumeric with spaces regex (for names)
static NAME_REGEX: OnceLock<Regex> = OnceLock::new();

/// Action command validation regex
static ACTION_REGEX: OnceLock<Regex> = OnceLock::new();

/// Maximum length constraints
pub struct ValidationLimits;

impl ValidationLimits {
    /// Maximum device/room name length
    pub const MAX_NAME_LENGTH: usize = 100;

    /// Maximum action command length
    pub const MAX_ACTION_LENGTH: usize = 50;

    /// Maximum number of devices in batch operations
    pub const MAX_BATCH_SIZE: usize = 100;

    /// Maximum JSON payload size (1MB)
    pub const MAX_PAYLOAD_SIZE: usize = 1_048_576;
}

/// Input validator for MCP tool parameters
pub struct InputValidator;

impl InputValidator {
    /// Initialize regex patterns
    fn init_patterns() {
        UUID_REGEX.get_or_init(|| {
            Regex::new(
                r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
            )
            .expect("Invalid UUID regex")
        });

        LOXONE_UUID_REGEX.get_or_init(|| {
            // Loxone UUID format: 8 hex chars, dot, 6 hex chars, dot, 1-3 alphanumeric chars
            Regex::new(r"^[0-9a-fA-F]{8}\.[0-9a-fA-F]{6}\.[a-zA-Z0-9]{1,3}$")
                .expect("Invalid Loxone UUID regex")
        });

        IP_REGEX.get_or_init(|| {
            Regex::new(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$").expect("Invalid IP regex")
        });

        IPV6_REGEX.get_or_init(|| {
            // Simplified IPv6 regex - covers most common formats
            Regex::new(r"^([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$|^::1$|^::$|^([0-9a-fA-F]{1,4}:){1,7}:$|^([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}$|^([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}$|^([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}$|^([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}$|^([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}$|^[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})$|^:((:[0-9a-fA-F]{1,4}){1,7}|:)$")
                .expect("Invalid IPv6 regex")
        });

        MAC_REGEX.get_or_init(|| {
            // MAC address formats: XX:XX:XX:XX:XX:XX or XX-XX-XX-XX-XX-XX
            Regex::new(r"^([0-9a-fA-F]{2}[:-]){5}[0-9a-fA-F]{2}$").expect("Invalid MAC regex")
        });

        PORT_REGEX.get_or_init(|| {
            // Port number: 1-65535
            Regex::new(r"^([1-9][0-9]{0,3}|[1-5][0-9]{4}|6[0-4][0-9]{3}|65[0-4][0-9]{2}|655[0-2][0-9]|6553[0-5])$")
                .expect("Invalid port regex")
        });

        NAME_REGEX.get_or_init(|| {
            Regex::new(r"^[a-zA-Z0-9\s\-_äöüÄÖÜß]{1,100}$").expect("Invalid name regex")
        });

        ACTION_REGEX
            .get_or_init(|| Regex::new(r"^[a-zA-Z0-9\-_]{1,50}$").expect("Invalid action regex"));
    }

    /// Validate UUID format (supports both standard UUID and Loxone format)
    pub fn validate_uuid(uuid: &str) -> Result<&str> {
        Self::init_patterns();

        if uuid.is_empty() {
            return Err(LoxoneError::invalid_input("UUID cannot be empty"));
        }

        if uuid.len() > 50 {
            return Err(LoxoneError::invalid_input("UUID too long"));
        }

        // Check standard UUID format first
        if UUID_REGEX.get().unwrap().is_match(uuid) {
            return Ok(uuid);
        }

        // Check Loxone UUID format
        if LOXONE_UUID_REGEX.get().unwrap().is_match(uuid) {
            return Ok(uuid);
        }

        Err(LoxoneError::invalid_input(format!(
            "Invalid UUID format: {}. Supported formats: standard UUID (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx) or Loxone UUID (XXXXXXXX.XXXXXX.XX)",
            uuid
        )))
    }

    /// Validate device/room name
    pub fn validate_name(name: &str) -> Result<&str> {
        Self::init_patterns();

        if name.is_empty() {
            return Err(LoxoneError::invalid_input("Name cannot be empty"));
        }

        if name.len() > ValidationLimits::MAX_NAME_LENGTH {
            return Err(LoxoneError::invalid_input(format!(
                "Name too long (max {} characters)",
                ValidationLimits::MAX_NAME_LENGTH
            )));
        }

        // Check for common injection patterns
        if name.contains("../") || name.contains("..\\") || name.contains('\0') {
            return Err(LoxoneError::invalid_input(
                "Name contains invalid characters",
            ));
        }

        if NAME_REGEX.get().unwrap().is_match(name) {
            Ok(name)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid name format: {}. Only alphanumeric characters, spaces, hyphens, and underscores allowed",
                name
            )))
        }
    }

    /// Validate action command
    pub fn validate_action(action: &str) -> Result<&str> {
        Self::init_patterns();

        if action.is_empty() {
            return Err(LoxoneError::invalid_input("Action cannot be empty"));
        }

        if action.len() > ValidationLimits::MAX_ACTION_LENGTH {
            return Err(LoxoneError::invalid_input(format!(
                "Action too long (max {} characters)",
                ValidationLimits::MAX_ACTION_LENGTH
            )));
        }

        // Check for command injection attempts
        let dangerous_chars = ['&', '|', ';', '$', '`', '\n', '\r', '\0'];
        if action.chars().any(|c| dangerous_chars.contains(&c)) {
            return Err(LoxoneError::invalid_input(
                "Action contains dangerous characters",
            ));
        }

        if ACTION_REGEX.get().unwrap().is_match(action) {
            Ok(action)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid action format: {}. Only alphanumeric characters, hyphens, and underscores allowed",
                action
            )))
        }
    }

    /// Validate IP address or hostname
    pub fn validate_host(host: &str) -> Result<&str> {
        Self::init_patterns();

        if host.is_empty() {
            return Err(LoxoneError::invalid_input("Host cannot be empty"));
        }

        if host.len() > 253 {
            return Err(LoxoneError::invalid_input("Host too long"));
        }

        // Check if it's an IP address (with optional port)
        // First, separate host and port
        let host_part = if let Some(colon_pos) = host.rfind(':') {
            // Check if it's IPv4:port pattern
            let potential_host = &host[..colon_pos];
            let potential_port = &host[colon_pos + 1..];

            // Validate port
            if let Ok(port) = potential_port.parse::<u16>() {
                if port > 0 {
                    potential_host
                } else {
                    host // Invalid port (0), treat whole thing as hostname
                }
            } else {
                host // Not a valid port, treat whole thing as hostname
            }
        } else {
            host
        };

        // Check if it looks like an IP address pattern
        let parts: Vec<&str> = host_part.split('.').collect();
        if parts.len() == 4 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            // It looks like an IP address, validate each octet
            let mut all_valid = true;
            for part in parts {
                if part.parse::<u8>().is_err() {
                    // If parsing as u8 fails, it's not a valid IP octet (e.g., > 255)
                    all_valid = false;
                    break;
                }
            }
            if all_valid {
                return Ok(host);
            } else {
                return Err(LoxoneError::invalid_input(format!(
                    "Invalid IP address: {}",
                    host
                )));
            }
        }

        // Validate as hostname
        let hostname_regex = Regex::new(
            r"^(?:[a-zA-Z0-9](?:[a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\.)*[a-zA-Z0-9](?:[a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(?::\d{1,5})?$"
        ).unwrap();

        if hostname_regex.is_match(host) {
            Ok(host)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid host format: {}",
                host
            )))
        }
    }

    /// Validate numeric value within range
    pub fn validate_numeric_range<T: PartialOrd + std::fmt::Display>(
        value: T,
        min: T,
        max: T,
        field_name: &str,
    ) -> Result<T> {
        if value < min || value > max {
            return Err(LoxoneError::invalid_input(format!(
                "{} must be between {} and {}",
                field_name, min, max
            )));
        }
        Ok(value)
    }

    /// Validate batch size
    pub fn validate_batch_size(size: usize) -> Result<usize> {
        if size == 0 {
            return Err(LoxoneError::invalid_input("Batch size cannot be zero"));
        }

        if size > ValidationLimits::MAX_BATCH_SIZE {
            return Err(LoxoneError::invalid_input(format!(
                "Batch size too large (max {})",
                ValidationLimits::MAX_BATCH_SIZE
            )));
        }

        Ok(size)
    }

    /// Sanitize string for safe display
    pub fn sanitize_for_display(input: &str) -> String {
        input
            .chars()
            .filter(|c| !c.is_control() || c.is_whitespace())
            .take(1000) // Limit display length
            .collect()
    }

    /// Validate JSON payload size
    pub fn validate_payload_size(payload: &str) -> Result<()> {
        if payload.len() > ValidationLimits::MAX_PAYLOAD_SIZE {
            return Err(LoxoneError::invalid_input(format!(
                "Payload too large (max {} bytes)",
                ValidationLimits::MAX_PAYLOAD_SIZE
            )));
        }
        Ok(())
    }

    /// Validate IPv4 address
    pub fn validate_ipv4(ip: &str) -> Result<&str> {
        Self::init_patterns();

        if ip.is_empty() {
            return Err(LoxoneError::invalid_input("IPv4 address cannot be empty"));
        }

        if ip.len() > 15 {
            return Err(LoxoneError::invalid_input("IPv4 address too long"));
        }

        // Basic format check
        if !IP_REGEX.get().unwrap().is_match(ip) {
            return Err(LoxoneError::invalid_input(format!(
                "Invalid IPv4 format: {}",
                ip
            )));
        }

        // Validate each octet is 0-255
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() != 4 {
            return Err(LoxoneError::invalid_input("IPv4 must have 4 octets"));
        }

        for (i, part) in parts.iter().enumerate() {
            match part.parse::<u8>() {
                Ok(octet) => {
                    // Check for special reserved ranges
                    if i == 0 && (octet == 0 || octet == 127 || octet >= 224) {
                        return Err(LoxoneError::invalid_input(format!(
                            "Invalid first octet: {} (reserved range)",
                            octet
                        )));
                    }
                }
                Err(_) => {
                    return Err(LoxoneError::invalid_input(format!(
                        "Invalid octet: {} (must be 0-255)",
                        part
                    )));
                }
            }
        }

        Ok(ip)
    }

    /// Validate IPv6 address
    pub fn validate_ipv6(ip: &str) -> Result<&str> {
        Self::init_patterns();

        if ip.is_empty() {
            return Err(LoxoneError::invalid_input("IPv6 address cannot be empty"));
        }

        if ip.len() > 39 {
            return Err(LoxoneError::invalid_input("IPv6 address too long"));
        }

        if IPV6_REGEX.get().unwrap().is_match(ip) {
            Ok(ip)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid IPv6 format: {}",
                ip
            )))
        }
    }

    /// Validate MAC address
    pub fn validate_mac_address(mac: &str) -> Result<&str> {
        Self::init_patterns();

        if mac.is_empty() {
            return Err(LoxoneError::invalid_input("MAC address cannot be empty"));
        }

        if mac.len() > 17 {
            return Err(LoxoneError::invalid_input("MAC address too long"));
        }

        if MAC_REGEX.get().unwrap().is_match(mac) {
            Ok(mac)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid MAC address format: {}. Use XX:XX:XX:XX:XX:XX or XX-XX-XX-XX-XX-XX",
                mac
            )))
        }
    }

    /// Validate port number
    pub fn validate_port(port: u16) -> Result<u16> {
        if port == 0 {
            return Err(LoxoneError::invalid_input("Port cannot be 0"));
        }

        // Check for well-known system ports that should be restricted
        if port < 1024 && ![22, 53, 80, 443, 993].contains(&port) {
            return Err(LoxoneError::invalid_input(format!(
                "Port {} is in system range (1-1023) and not explicitly allowed",
                port
            )));
        }

        Ok(port)
    }

    /// Validate port from string
    pub fn validate_port_string(port_str: &str) -> Result<u16> {
        Self::init_patterns();

        if port_str.is_empty() {
            return Err(LoxoneError::invalid_input("Port cannot be empty"));
        }

        // First check regex format
        if !PORT_REGEX.get().unwrap().is_match(port_str) {
            return Err(LoxoneError::invalid_input(format!(
                "Invalid port format: {}",
                port_str
            )));
        }

        // Parse as u16
        let port = port_str.parse::<u16>().map_err(|_| {
            LoxoneError::invalid_input(format!("Port out of range: {} (must be 1-65535)", port_str))
        })?;

        Self::validate_port(port)
    }

    /// Validate network CIDR notation
    pub fn validate_cidr(cidr: &str) -> Result<&str> {
        if cidr.is_empty() {
            return Err(LoxoneError::invalid_input("CIDR cannot be empty"));
        }

        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(LoxoneError::invalid_input(
                "CIDR must be in format IP/prefix",
            ));
        }

        // Validate IP part
        Self::validate_ipv4(parts[0])?;

        // Validate prefix
        let prefix = parts[1].parse::<u8>().map_err(|_| {
            LoxoneError::invalid_input(format!("Invalid CIDR prefix: {} (must be 0-32)", parts[1]))
        })?;

        if prefix > 32 {
            return Err(LoxoneError::invalid_input(format!(
                "CIDR prefix too large: {} (must be 0-32)",
                prefix
            )));
        }

        Ok(cidr)
    }

    /// Validate URL format
    pub fn validate_url(url: &str) -> Result<&str> {
        if url.is_empty() {
            return Err(LoxoneError::invalid_input("URL cannot be empty"));
        }

        if url.len() > 2048 {
            return Err(LoxoneError::invalid_input("URL too long"));
        }

        // Basic URL validation
        let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").expect("Invalid URL regex");

        if url_regex.is_match(url) {
            Ok(url)
        } else {
            Err(LoxoneError::invalid_input(format!(
                "Invalid URL format: {}",
                url
            )))
        }
    }

    /// Validate network address (IPv4, IPv6, hostname, or hostname:port)
    pub fn validate_network_address(address: &str) -> Result<&str> {
        if address.is_empty() {
            return Err(LoxoneError::invalid_input(
                "Network address cannot be empty",
            ));
        }

        // Try parsing as hostname:port first
        if let Some(colon_pos) = address.rfind(':') {
            let host_part = &address[..colon_pos];
            let port_part = &address[colon_pos + 1..];

            // Validate port part
            Self::validate_port_string(port_part)?;

            // Validate host part
            return Self::validate_network_host(host_part);
        }

        // No port, validate as host only
        Self::validate_network_host(address)
    }

    /// Validate network host (IPv4, IPv6, or hostname)
    pub fn validate_network_host(host: &str) -> Result<&str> {
        if host.is_empty() {
            return Err(LoxoneError::invalid_input("Host cannot be empty"));
        }

        // Try IPv4 first
        if Self::validate_ipv4(host).is_ok() {
            return Ok(host);
        }

        // Try IPv6 (may be in brackets)
        let ipv6_host = if host.starts_with('[') && host.ends_with(']') {
            &host[1..host.len() - 1]
        } else {
            host
        };

        if Self::validate_ipv6(ipv6_host).is_ok() {
            return Ok(host);
        }

        // Finally try hostname validation (same as existing validate_host logic)
        Self::validate_host(host)
    }
}

/// Validation middleware for tool parameters
pub struct ToolParameterValidator;

impl ToolParameterValidator {
    /// Validate device control parameters
    pub fn validate_device_control(device: &str, action: &str) -> Result<()> {
        // Try as UUID first, then as name
        if device.contains('-') {
            InputValidator::validate_uuid(device)?;
        } else {
            InputValidator::validate_name(device)?;
        }

        InputValidator::validate_action(action)?;
        Ok(())
    }

    /// Validate room control parameters
    pub fn validate_room_control(room_name: &str, action: &str) -> Result<()> {
        InputValidator::validate_name(room_name)?;
        InputValidator::validate_action(action)?;
        Ok(())
    }

    /// Validate temperature parameters
    pub fn validate_temperature(room_name: &str, temperature: f64) -> Result<()> {
        InputValidator::validate_name(room_name)?;
        InputValidator::validate_numeric_range(temperature, 5.0, 30.0, "Temperature")?;
        Ok(())
    }

    /// Validate discovery parameters
    pub fn validate_discovery_params(
        category: Option<&String>,
        device_type: Option<&String>,
        limit: Option<usize>,
    ) -> Result<()> {
        if let Some(cat) = category {
            InputValidator::validate_name(cat)?;
        }

        if let Some(dt) = device_type {
            InputValidator::validate_name(dt)?;
        }

        if let Some(l) = limit {
            InputValidator::validate_batch_size(l)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_validation() {
        // Valid standard UUID
        assert!(InputValidator::validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());

        // Valid Loxone UUID
        assert!(InputValidator::validate_uuid("0CD8C06B.855703.I2").is_ok());
        assert!(InputValidator::validate_uuid("1234ABCD.123456.O1").is_ok());

        // Invalid UUIDs
        assert!(InputValidator::validate_uuid("not-a-uuid").is_err());
        assert!(InputValidator::validate_uuid("").is_err());
        assert!(InputValidator::validate_uuid("550e8400-e29b-41d4-a716-44665544000g").is_err());

        // Invalid Loxone UUID formats
        assert!(InputValidator::validate_uuid("0CD8C06B.855703").is_err()); // Missing last part
        assert!(InputValidator::validate_uuid("0CD8C06B.855703.I2345").is_err()); // Last part too long
        assert!(InputValidator::validate_uuid("0CD8C06B.85570.I2").is_err()); // Middle part too short
    }

    #[test]
    fn test_name_validation() {
        // Valid names
        assert!(InputValidator::validate_name("Living Room").is_ok());
        assert!(InputValidator::validate_name("Küche").is_ok());
        assert!(InputValidator::validate_name("Room_123").is_ok());

        // Invalid names
        assert!(InputValidator::validate_name("").is_err());
        assert!(InputValidator::validate_name("../etc/passwd").is_err());
        assert!(InputValidator::validate_name("Room;DROP TABLE").is_err());
    }

    #[test]
    fn test_action_validation() {
        // Valid actions
        assert!(InputValidator::validate_action("on").is_ok());
        assert!(InputValidator::validate_action("off").is_ok());
        assert!(InputValidator::validate_action("dim-50").is_ok());

        // Invalid actions
        assert!(InputValidator::validate_action("").is_err());
        assert!(InputValidator::validate_action("on;ls").is_err());
        assert!(InputValidator::validate_action("off|cat").is_err());
    }

    #[test]
    fn test_host_validation() {
        // Valid hosts
        assert!(InputValidator::validate_host("192.168.1.100").is_ok());
        assert!(InputValidator::validate_host("192.168.1.100:8080").is_ok());
        assert!(InputValidator::validate_host("loxone.local").is_ok());
        assert!(InputValidator::validate_host("miniserver.home.lan").is_ok());

        // Invalid hosts
        assert!(InputValidator::validate_host("").is_err());

        // Test the problematic IP
        let result = InputValidator::validate_host("192.168.1.256");
        println!("Result for 192.168.1.256: {:?}", result);
        assert!(result.is_err());

        assert!(InputValidator::validate_host("host with spaces").is_err());
    }

    #[test]
    fn test_ipv4_validation() {
        // Valid IPv4 addresses
        assert!(InputValidator::validate_ipv4("192.168.1.100").is_ok());
        assert!(InputValidator::validate_ipv4("10.0.0.1").is_ok());
        assert!(InputValidator::validate_ipv4("172.16.0.1").is_ok());

        // Invalid IPv4 addresses
        assert!(InputValidator::validate_ipv4("").is_err());
        assert!(InputValidator::validate_ipv4("192.168.1.256").is_err()); // > 255
        assert!(InputValidator::validate_ipv4("192.168.1").is_err()); // Missing octet
        assert!(InputValidator::validate_ipv4("192.168.1.1.1").is_err()); // Too many octets
        assert!(InputValidator::validate_ipv4("0.168.1.100").is_err()); // Reserved first octet
        assert!(InputValidator::validate_ipv4("127.0.0.1").is_err()); // Loopback reserved
        assert!(InputValidator::validate_ipv4("224.0.0.1").is_err()); // Multicast reserved
    }

    #[test]
    fn test_ipv6_validation() {
        // Valid IPv6 addresses
        assert!(InputValidator::validate_ipv6("2001:0db8:85a3:0000:0000:8a2e:0370:7334").is_ok());
        assert!(InputValidator::validate_ipv6("::1").is_ok());
        assert!(InputValidator::validate_ipv6("::").is_ok());

        // Invalid IPv6 addresses
        assert!(InputValidator::validate_ipv6("").is_err());
        assert!(InputValidator::validate_ipv6("not:valid:ipv6").is_err());
        assert!(InputValidator::validate_ipv6("2001:0db8:85a3::8a2e::7334").is_err());
        // Double ::
    }

    #[test]
    fn test_mac_address_validation() {
        // Valid MAC addresses
        assert!(InputValidator::validate_mac_address("AA:BB:CC:DD:EE:FF").is_ok());
        assert!(InputValidator::validate_mac_address("aa:bb:cc:dd:ee:ff").is_ok());
        assert!(InputValidator::validate_mac_address("AA-BB-CC-DD-EE-FF").is_ok());
        assert!(InputValidator::validate_mac_address("00:11:22:33:44:55").is_ok());

        // Invalid MAC addresses
        assert!(InputValidator::validate_mac_address("").is_err());
        assert!(InputValidator::validate_mac_address("AA:BB:CC:DD:EE").is_err()); // Too short
        assert!(InputValidator::validate_mac_address("AA:BB:CC:DD:EE:FF:GG").is_err()); // Too long
        assert!(InputValidator::validate_mac_address("GG:BB:CC:DD:EE:FF").is_err()); // Invalid hex
        assert!(InputValidator::validate_mac_address("AA.BB.CC.DD.EE.FF").is_err());
        // Wrong separator
    }

    #[test]
    fn test_port_validation() {
        // Valid ports
        assert!(InputValidator::validate_port(80).is_ok());
        assert!(InputValidator::validate_port(443).is_ok());
        assert!(InputValidator::validate_port(8080).is_ok());
        assert!(InputValidator::validate_port(65535).is_ok());

        // Invalid ports
        assert!(InputValidator::validate_port(0).is_err());
        assert!(InputValidator::validate_port(21).is_err()); // System port not in allowed list
        assert!(InputValidator::validate_port(23).is_err()); // System port not in allowed list
    }

    #[test]
    fn test_port_string_validation() {
        // Valid port strings
        assert!(InputValidator::validate_port_string("80").is_ok());
        assert!(InputValidator::validate_port_string("443").is_ok());
        assert!(InputValidator::validate_port_string("8080").is_ok());

        // Invalid port strings
        assert!(InputValidator::validate_port_string("").is_err());
        assert!(InputValidator::validate_port_string("0").is_err());
        assert!(InputValidator::validate_port_string("65536").is_err()); // > max port
        assert!(InputValidator::validate_port_string("abc").is_err());
        assert!(InputValidator::validate_port_string("-80").is_err());
    }

    #[test]
    fn test_cidr_validation() {
        // Valid CIDR notations
        assert!(InputValidator::validate_cidr("192.168.1.0/24").is_ok());
        assert!(InputValidator::validate_cidr("10.0.0.0/8").is_ok());
        assert!(InputValidator::validate_cidr("172.16.0.0/16").is_ok());
        assert!(InputValidator::validate_cidr("192.168.1.100/32").is_ok());

        // Invalid CIDR notations
        assert!(InputValidator::validate_cidr("").is_err());
        assert!(InputValidator::validate_cidr("192.168.1.0").is_err()); // Missing prefix
        assert!(InputValidator::validate_cidr("192.168.1.0/33").is_err()); // Invalid prefix
        assert!(InputValidator::validate_cidr("192.168.1.256/24").is_err()); // Invalid IP
        assert!(InputValidator::validate_cidr("192.168.1.0/abc").is_err()); // Invalid prefix format
    }

    #[test]
    fn test_url_validation() {
        // Valid URLs
        assert!(InputValidator::validate_url("http://example.com").is_ok());
        assert!(InputValidator::validate_url("https://example.com").is_ok());
        assert!(InputValidator::validate_url("http://192.168.1.100").is_ok());
        assert!(InputValidator::validate_url("https://example.com:8080/path").is_ok());

        // Invalid URLs
        assert!(InputValidator::validate_url("").is_err());
        assert!(InputValidator::validate_url("ftp://example.com").is_err()); // Wrong protocol
        assert!(InputValidator::validate_url("example.com").is_err()); // Missing protocol
        assert!(InputValidator::validate_url("http://").is_err()); // Incomplete
    }

    #[test]
    fn test_network_address_validation() {
        // Valid network addresses
        assert!(InputValidator::validate_network_address("192.168.1.100").is_ok());
        assert!(InputValidator::validate_network_address("192.168.1.100:8080").is_ok());
        assert!(InputValidator::validate_network_address("example.com").is_ok());
        assert!(InputValidator::validate_network_address("example.com:443").is_ok());
        assert!(InputValidator::validate_network_address("[::1]:8080").is_ok());

        // Invalid network addresses
        assert!(InputValidator::validate_network_address("").is_err());
        assert!(InputValidator::validate_network_address("192.168.1.256:8080").is_err()); // Invalid IP
        assert!(InputValidator::validate_network_address("example.com:99999").is_err()); // Invalid port
        assert!(InputValidator::validate_network_address("example.com:0").is_err());
        // Invalid port
    }

    #[test]
    fn test_network_host_validation() {
        // Valid network hosts
        assert!(InputValidator::validate_network_host("192.168.1.100").is_ok());
        assert!(InputValidator::validate_network_host("example.com").is_ok());
        assert!(InputValidator::validate_network_host("sub.example.com").is_ok());
        assert!(InputValidator::validate_network_host("[::1]").is_ok());

        // Invalid network hosts
        assert!(InputValidator::validate_network_host("").is_err());
        assert!(InputValidator::validate_network_host("192.168.1.256").is_err()); // Invalid IP
        assert!(InputValidator::validate_network_host("host with spaces").is_err());
    }
}
