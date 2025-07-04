//! Common test utilities
//!
//! NOTE: Temporarily simplified due to API changes - needs full update for rmcp 0.1.2

#[cfg(test)]
pub mod test_helpers {
    use loxone_mcp_rust::config::ServerConfig;

    pub fn create_test_config() -> ServerConfig {
        // Return a minimal valid config for testing
        ServerConfig::default()
    }
}
