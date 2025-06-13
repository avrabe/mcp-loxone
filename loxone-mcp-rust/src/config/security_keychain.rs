//! Alternative keychain access using macOS security command
//!
//! This module provides keychain access via the `security` command-line tool
//! which may avoid password prompts in some cases.

use crate::error::{LoxoneError, Result};
use std::process::Command;

/// Keychain access using macOS security command
pub struct SecurityKeychain;

impl SecurityKeychain {
    /// Get password using security command-line tool
    pub fn get_password(service: &str, account: &str) -> Result<String> {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", service, "-a", account, "-w"])
            .output()
            .map_err(|e| {
                LoxoneError::credentials(format!("Failed to run security command: {}", e))
            })?;

        if !output.status.success() {
            return Err(LoxoneError::credentials(format!(
                "Security command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let password = String::from_utf8(output.stdout)
            .map_err(|e| LoxoneError::credentials(format!("Invalid UTF-8 in password: {}", e)))?
            .trim()
            .to_string();

        Ok(password)
    }

    /// Get all credentials using security command
    pub fn get_all_credentials() -> Result<(String, String, Option<String>, Option<String>)> {
        let username = Self::get_password("LoxoneMCP", "LOXONE_USER")?;
        let password = Self::get_password("LoxoneMCP", "LOXONE_PASS")?;
        let host = Self::get_password("LoxoneMCP", "LOXONE_HOST").ok();
        // Try new name first, then old name for backward compatibility
        let api_key = Self::get_password("LoxoneMCP", "LOXONE_API_KEY")
            .or_else(|_| Self::get_password("LoxoneMCP", "LOXONE_SSE_API_KEY"))
            .ok();

        Ok((username, password, host, api_key))
    }

    /// Check if security command is available
    pub fn is_available() -> bool {
        Command::new("security")
            .arg("--help")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
