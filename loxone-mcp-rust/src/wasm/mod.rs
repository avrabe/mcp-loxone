//! WASM-specific functionality and optimizations
//!
//! This module provides WASM-specific implementations, optimizations,
//! and component model support for different WASM targets.

#[cfg(target_arch = "wasm32")]
pub mod wasip2;

#[cfg(target_arch = "wasm32")]
pub mod browser;

#[cfg(target_arch = "wasm32")]
pub mod component;

#[cfg(target_arch = "wasm32")]
pub mod optimizations;

// Re-export main WASM functionality
#[cfg(target_arch = "wasm32")]
pub use wasip2::*;

#[cfg(target_arch = "wasm32")]
pub use browser::*;

#[cfg(target_arch = "wasm32")]
pub use component::*;

// WASM target detection and configuration
pub fn get_wasm_target() -> WasmTarget {
    #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
    {
        // Check for WASIP2 features
        if cfg!(feature = "wasi-keyvalue") || has_wasi_keyvalue() {
            return WasmTarget::Wasip2;
        }
        return WasmTarget::Wasip1;
    }

    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    {
        return WasmTarget::Browser;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        WasmTarget::None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WasmTarget {
    None,
    Browser,
    Wasip1,
    Wasip2,
}

impl WasmTarget {
    pub fn supports_keyvalue(&self) -> bool {
        matches!(self, WasmTarget::Wasip2 | WasmTarget::Browser)
    }

    pub fn supports_http(&self) -> bool {
        matches!(self, WasmTarget::Wasip2 | WasmTarget::Browser)
    }

    pub fn supports_config(&self) -> bool {
        matches!(self, WasmTarget::Wasip2)
    }

    pub fn is_wasm(&self) -> bool {
        !matches!(self, WasmTarget::None)
    }
}

/// Check if WASI keyvalue interface is available
#[cfg(target_arch = "wasm32")]
fn has_wasi_keyvalue() -> bool {
    // This would check for WASI keyvalue capability
    // For now, we'll use feature detection
    cfg!(feature = "wasi-keyvalue")
}

#[cfg(not(target_arch = "wasm32"))]
fn has_wasi_keyvalue() -> bool {
    false
}

/// WASM-specific error types
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("WASM target not supported: {target:?}")]
    UnsupportedTarget { target: WasmTarget },

    #[error("WASI interface not available: {interface}")]
    WasiInterfaceUnavailable { interface: String },

    #[error("Browser API not available: {api}")]
    BrowserApiUnavailable { api: String },

    #[error("Component initialization failed: {reason}")]
    ComponentInitFailed { reason: String },

    #[error("Memory limit exceeded: {limit_mb}MB")]
    MemoryLimitExceeded { limit_mb: u32 },
}

/// WASM runtime configuration
#[derive(Debug, Clone)]
pub struct WasmConfig {
    pub target: WasmTarget,
    pub max_memory_mb: u32,
    pub enable_debug_logging: bool,
    pub keyvalue_namespace: String,
    pub http_timeout_ms: u32,
    pub component_features: Vec<String>,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            target: get_wasm_target(),
            max_memory_mb: 64, // 64MB default for WASM
            enable_debug_logging: false,
            keyvalue_namespace: "loxone-mcp".to_string(),
            http_timeout_ms: 30000, // 30 seconds
            component_features: vec![
                "credential-management".to_string(),
                "device-control".to_string(),
                "sensor-discovery".to_string(),
            ],
        }
    }
}

impl WasmConfig {
    /// Create optimized config for WASIP2
    pub fn for_wasip2() -> Self {
        Self {
            target: WasmTarget::Wasip2,
            max_memory_mb: 128, // More memory for WASIP2
            enable_debug_logging: true,
            keyvalue_namespace: "loxone-mcp-wasip2".to_string(),
            http_timeout_ms: 60000, // Longer timeout for component calls
            component_features: vec![
                "credential-management".to_string(),
                "device-control".to_string(),
                "sensor-discovery".to_string(),
                "infisical-integration".to_string(),
                "wasi-keyvalue".to_string(),
                "wasi-http".to_string(),
                "wasi-config".to_string(),
            ],
        }
    }

    /// Create config for browser environment
    pub fn for_browser() -> Self {
        Self {
            target: WasmTarget::Browser,
            max_memory_mb: 32, // Limited memory for browser
            enable_debug_logging: false,
            keyvalue_namespace: "loxone-mcp-browser".to_string(),
            http_timeout_ms: 15000, // Shorter timeout for browser
            component_features: vec![
                "credential-management".to_string(),
                "device-control".to_string(),
                "local-storage".to_string(),
            ],
        }
    }

    /// Validate configuration for target
    pub fn validate(&self) -> Result<(), WasmError> {
        match self.target {
            WasmTarget::None => {
                return Err(WasmError::UnsupportedTarget {
                    target: self.target.clone(),
                });
            }
            WasmTarget::Wasip2 => {
                if !self
                    .component_features
                    .contains(&"wasi-keyvalue".to_string())
                {
                    return Err(WasmError::ComponentInitFailed {
                        reason: "WASIP2 requires wasi-keyvalue feature".to_string(),
                    });
                }
            }
            WasmTarget::Browser => {
                if self.max_memory_mb > 64 {
                    return Err(WasmError::MemoryLimitExceeded { limit_mb: 64 });
                }
            }
            WasmTarget::Wasip1 => {
                // Basic validation for WASIP1
            }
        }

        Ok(())
    }
}

/// WASM capabilities detection
pub struct WasmCapabilities {
    pub has_keyvalue: bool,
    pub has_http: bool,
    pub has_config: bool,
    pub has_logging: bool,
    pub has_crypto: bool,
    pub memory_limit_mb: Option<u32>,
}

impl WasmCapabilities {
    /// Detect available WASM capabilities
    pub fn detect() -> Self {
        let target = get_wasm_target();

        Self {
            has_keyvalue: target.supports_keyvalue(),
            has_http: target.supports_http(),
            has_config: target.supports_config(),
            has_logging: true, // Always available
            has_crypto: detect_crypto_support(),
            memory_limit_mb: detect_memory_limit(),
        }
    }

    /// Check if all required capabilities are available
    pub fn check_requirements(&self, requirements: &[&str]) -> Result<(), WasmError> {
        for requirement in requirements {
            match *requirement {
                "keyvalue" if !self.has_keyvalue => {
                    return Err(WasmError::WasiInterfaceUnavailable {
                        interface: "keyvalue".to_string(),
                    });
                }
                "http" if !self.has_http => {
                    return Err(WasmError::WasiInterfaceUnavailable {
                        interface: "http".to_string(),
                    });
                }
                "config" if !self.has_config => {
                    return Err(WasmError::WasiInterfaceUnavailable {
                        interface: "config".to_string(),
                    });
                }
                "crypto" if !self.has_crypto => {
                    return Err(WasmError::BrowserApiUnavailable {
                        api: "crypto".to_string(),
                    });
                }
                _ => {} // Unknown requirement, ignore
            }
        }

        Ok(())
    }
}

/// Detect crypto support
fn detect_crypto_support() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // Check for WebCrypto API in browser
        if let Some(window) = web_sys::window() {
            if let Ok(crypto) = js_sys::Reflect::get(&window, &"crypto".into()) {
                return !crypto.is_undefined();
            }
        }

        // Check for WASI crypto interface
        // This would be implemented when WASI crypto is standardized
        false
    }

    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Detect memory limit
fn detect_memory_limit() -> Option<u32> {
    #[cfg(target_arch = "wasm32")]
    {
        // Get WASM memory information
        let memory = wasm_bindgen::memory();
        let buffer = memory.buffer();
        Some((buffer.byte_length() / (1024 * 1024)) as u32)
    }

    #[cfg(not(target_arch = "wasm32"))]
    None
}

/// WASM module initialization
#[cfg(target_arch = "wasm32")]
pub fn initialize_wasm_module() -> Result<WasmConfig, WasmError> {
    let target = get_wasm_target();
    let config = match target {
        WasmTarget::Wasip2 => WasmConfig::for_wasip2(),
        WasmTarget::Browser => WasmConfig::for_browser(),
        WasmTarget::Wasip1 => WasmConfig::default(),
        WasmTarget::None => {
            return Err(WasmError::UnsupportedTarget { target });
        }
    };

    // Validate configuration
    config.validate()?;

    // Log initialization
    log_wasm_info(&config);

    Ok(config)
}

#[cfg(target_arch = "wasm32")]
fn log_wasm_info(config: &WasmConfig) {
    let capabilities = WasmCapabilities::detect();

    // Use appropriate logging for target
    match config.target {
        WasmTarget::Wasip2 => {
            #[cfg(target_os = "wasi")]
            {
                wasi::logging::log(
                    wasi::logging::Level::Info,
                    "loxone-mcp",
                    &format!("WASM module initialized for {:?}", config.target),
                );

                wasi::logging::log(
                    wasi::logging::Level::Debug,
                    "loxone-mcp",
                    &format!(
                        "Capabilities: keyvalue={}, http={}, config={}",
                        capabilities.has_keyvalue, capabilities.has_http, capabilities.has_config
                    ),
                );
            }
        }
        WasmTarget::Browser => {
            web_sys::console::log_1(
                &format!(
                    "Loxone MCP: WASM module initialized for {:?}",
                    config.target
                )
                .into(),
            );
        }
        _ => {
            // Basic console output
            #[cfg(feature = "debug-logging")]
            web_sys::console::log_1(&"Loxone MCP: WASM module initialized".into());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn initialize_wasm_module() -> Result<WasmConfig, WasmError> {
    Err(WasmError::UnsupportedTarget {
        target: WasmTarget::None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_target_detection() {
        let target = get_wasm_target();

        #[cfg(target_arch = "wasm32")]
        assert!(target.is_wasm());

        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(target, WasmTarget::None);
    }

    #[test]
    fn test_wasm_config_validation() {
        let config = WasmConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = WasmConfig::for_browser();
        invalid_config.max_memory_mb = 128; // Too much for browser
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_capabilities_detection() {
        let capabilities = WasmCapabilities::detect();

        // Test requirement checking
        let result = capabilities.check_requirements(&["logging"]);
        assert!(result.is_ok());

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = capabilities.check_requirements(&["keyvalue"]);
            assert!(result.is_err());
        }
    }
}
