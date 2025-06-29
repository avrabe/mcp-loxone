//! Tests for the McpConfig derive macro

use clap::Parser;
use pulseengine_mcp_cli::{DefaultLoggingConfig, McpConfiguration};
use pulseengine_mcp_cli_derive::McpConfig;
use pulseengine_mcp_protocol::ServerInfo;

#[cfg(test)]
mod basic_tests {
    use super::*;

    /// Test basic McpConfig derive
    #[test]
    fn test_basic_derive() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct BasicConfig {
            #[arg(short, long)]
            port: u16,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for BasicConfig {
            fn default() -> Self {
                Self {
                    port: 8080,
                    server_info: Some(pulseengine_mcp_cli::config::create_server_info(
                        Some("test".to_string()),
                        Some("1.0.0".to_string()),
                    )),
                    logging: Some(DefaultLoggingConfig::default()),
                }
            }
        }

        // Test that the generated trait implementation works
        let config = BasicConfig::default();

        // Test McpConfiguration trait methods
        assert!(config.get_server_info().server_info.name == "test");
        assert!(config.get_logging_config().level == "info");
        assert!(config.validate().is_ok());
    }

    /// Test auto-populate attribute
    #[test]
    fn test_auto_populate() {
        #[derive(McpConfig, Parser, Clone, Default)]
        #[command(name = "test")]
        struct AutoPopulateConfig {
            #[arg(short, long, default_value = "3000")]
            port: u16,

            #[mcp(auto_populate)]
            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        // Create config with auto-populate
        let config = AutoPopulateConfig::with_auto_populate();

        // Server info should be populated from Cargo.toml
        assert!(config.server_info.is_some());
        let server_info = config.server_info.as_ref().unwrap();
        assert_eq!(server_info.server_info.name, env!("CARGO_PKG_NAME"));
        assert_eq!(server_info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    /// Test logging configuration attribute
    #[test]
    fn test_logging_config() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct LoggingConfig {
            #[arg(short, long)]
            verbose: bool,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[mcp(logging)]
            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for LoggingConfig {
            fn default() -> Self {
                Self {
                    verbose: false,
                    server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
                    logging: Some(DefaultLoggingConfig::default()),
                }
            }
        }

        let config = LoggingConfig::default();

        // Test that logging configuration is accessible
        let logging_config = config.get_logging_config();
        assert_eq!(logging_config.level, "info");
        assert!(matches!(
            logging_config.format,
            pulseengine_mcp_cli::LogFormat::Pretty
        ));
    }
}

#[cfg(test)]
mod field_attribute_tests {
    use super::*;

    /// Test multiple mcp attributes on fields
    #[test]
    fn test_multiple_attributes() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct MultiAttributeConfig {
            #[arg(short, long)]
            name: String,

            #[mcp(auto_populate)]
            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[mcp(logging)]
            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,

            #[clap(skip)]
            internal_field: Option<String>,
        }

        impl Default for MultiAttributeConfig {
            fn default() -> Self {
                Self {
                    name: "test".to_string(),
                    server_info: None,
                    logging: Some(DefaultLoggingConfig::default()),
                    internal_field: Some("internal".to_string()),
                }
            }
        }

        let mut config = MultiAttributeConfig::default();
        config.auto_populate();

        // Server info should be populated
        assert!(config.server_info.is_some());

        // Logging should be configured
        assert!(config.logging.is_some());

        // Internal field should be ignored by MCP processing
        assert_eq!(config.internal_field, Some("internal".to_string()));
    }

    /// Test custom types with McpConfig
    #[test]
    fn test_custom_types() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct CustomServerInfo {
            name: String,
            version: String,
            custom_field: String,
        }

        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct CustomTypeConfig {
            #[arg(short, long)]
            port: u16,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,

            #[clap(skip)]
            custom_info: Option<CustomServerInfo>,
        }

        impl Default for CustomTypeConfig {
            fn default() -> Self {
                Self {
                    port: 8080,
                    server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
                    logging: Some(DefaultLoggingConfig::default()),
                    custom_info: Some(CustomServerInfo {
                        name: "custom".to_string(),
                        version: "1.0.0".to_string(),
                        custom_field: "test".to_string(),
                    }),
                }
            }
        }

        let config = CustomTypeConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.custom_info.is_some());
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    /// Test validation logic
    #[test]
    fn test_validation() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct ValidatedConfig {
            #[arg(short, long)]
            port: u16,

            #[arg(short, long)]
            max_connections: usize,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for ValidatedConfig {
            fn default() -> Self {
                Self {
                    port: 8080,
                    max_connections: 1000,
                    server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
                    logging: Some(DefaultLoggingConfig::default()),
                }
            }
        }

        // Test valid configuration
        let valid_config = ValidatedConfig::default();
        assert!(valid_config.validate().is_ok());

        // Test with different values
        let mut config = ValidatedConfig::default();
        config.port = 0; // Port 0 is valid (OS assigns)
        assert!(config.validate().is_ok());

        config.port = 65535; // Max port
        assert!(config.validate().is_ok());
    }

    /// Test error handling in generated code
    #[test]
    fn test_error_cases() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct ErrorTestConfig {
            #[arg(short, long)]
            required_field: String,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for ErrorTestConfig {
            fn default() -> Self {
                Self {
                    required_field: String::new(),
                    server_info: None,
                    logging: None,
                }
            }
        }

        let config = ErrorTestConfig::default();

        // Test with missing server info
        assert!(config.get_server_info().server_info.name == env!("CARGO_PKG_NAME"));

        // Test with missing logging config
        assert!(config.get_logging_config().level == "info");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test full integration with clap parsing
    #[test]
    fn test_clap_integration() {
        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test-app", about = "Test application")]
        struct CliConfig {
            /// Server port
            #[arg(short, long, default_value = "8080")]
            port: u16,

            /// Enable debug mode
            #[arg(short, long)]
            debug: bool,

            /// Configuration file
            #[arg(short, long)]
            config_file: Option<String>,

            #[mcp(auto_populate)]
            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[mcp(logging)]
            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for CliConfig {
            fn default() -> Self {
                Self {
                    port: 8080,
                    debug: false,
                    config_file: None,
                    server_info: None,
                    logging: Some(DefaultLoggingConfig::default()),
                }
            }
        }

        // Test parsing with args
        let config = CliConfig::try_parse_from(&["test", "--port", "3000", "--debug"])
            .expect("Failed to parse args");

        assert_eq!(config.port, 3000);
        assert!(config.debug);
        assert!(config.config_file.is_none());

        // Test that MCP fields are populated correctly
        assert!(config.validate().is_ok());
    }

    /// Test environment variable support
    #[test]
    fn test_env_var_support() {
        use std::env;

        #[derive(McpConfig, Parser, Clone)]
        #[command(name = "test")]
        struct EnvConfig {
            /// Port from environment
            #[arg(short, long, env = "TEST_PORT", default_value = "8080")]
            port: u16,

            /// API key from environment
            #[arg(long, env = "TEST_API_KEY")]
            api_key: Option<String>,

            #[clap(skip)]
            server_info: Option<ServerInfo>,

            #[clap(skip)]
            logging: Option<DefaultLoggingConfig>,
        }

        impl Default for EnvConfig {
            fn default() -> Self {
                Self {
                    port: 8080,
                    api_key: None,
                    server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
                    logging: Some(DefaultLoggingConfig::default()),
                }
            }
        }

        // Set environment variables
        env::set_var("TEST_PORT", "9000");
        env::set_var("TEST_API_KEY", "secret-key");

        // Parse without command line args
        let config = EnvConfig::try_parse_from(&["test"]).expect("Failed to parse from env");

        assert_eq!(config.port, 9000);
        assert_eq!(config.api_key, Some("secret-key".to_string()));

        // Clean up
        env::remove_var("TEST_PORT");
        env::remove_var("TEST_API_KEY");
    }
}

/// Test logging initialization
#[test]
fn test_logging_initialization() {
    #[derive(McpConfig, Parser, Clone)]
    #[command(name = "test")]
    struct LogInitConfig {
        #[arg(short, long)]
        quiet: bool,

        #[clap(skip)]
        server_info: Option<ServerInfo>,

        #[mcp(logging)]
        #[clap(skip)]
        logging: Option<DefaultLoggingConfig>,
    }

    impl Default for LogInitConfig {
        fn default() -> Self {
            Self {
                quiet: false,
                server_info: Some(pulseengine_mcp_cli::config::create_server_info(None, None)),
                logging: Some(DefaultLoggingConfig {
                    level: "debug".to_string(),
                    format: pulseengine_mcp_cli::LogFormat::Json,
                    output: pulseengine_mcp_cli::LogOutput::Stdout,
                    structured: true,
                }),
            }
        }
    }

    let config = LogInitConfig::default();

    // Test that we can get logging config
    let log_config = config.get_logging_config();
    assert_eq!(log_config.level, "debug");
    assert!(matches!(
        log_config.format,
        pulseengine_mcp_cli::LogFormat::Json
    ));

    // Note: We can't actually test initialize_logging() here because
    // it would conflict with other tests' logging initialization
}
