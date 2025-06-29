//! Tests for the McpBackend derive macro

use pulseengine_mcp_cli_derive::McpBackend;
use pulseengine_mcp_protocol::{
    CallToolRequestParam, CallToolResult, Content, GetPromptRequestParam, GetPromptResult,
    ListPromptsResult, ListResourcesResult, ListToolsResult, PaginatedRequestParam, PromptMessage,
    PromptMessageContent, PromptMessageRole, ReadResourceRequestParam, ReadResourceResult,
    ServerInfo,
};
use pulseengine_mcp_server::backend::{BackendError, McpBackend as McpBackendTrait, SimpleBackend};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Test configuration for backends
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestConfig {
    name: String,
    version: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
        }
    }
}

#[cfg(test)]
mod simple_backend_tests {
    use super::*;

    /// Test basic SimpleBackend derive
    #[test]
    fn test_simple_backend_derive() {
        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple)]
        struct SimpleTestBackend {
            config: TestConfig,
        }

        impl SimpleTestBackend {
            fn new(config: TestConfig) -> Self {
                Self { config }
            }
        }

        // This should compile and generate the SimpleBackend implementation
        let backend = SimpleTestBackend::new(TestConfig::default());

        // Test that the generated methods work
        let server_info = <SimpleTestBackend as SimpleBackend>::get_server_info(&backend);
        assert_eq!(server_info.server_info.name, env!("CARGO_PKG_NAME"));
        assert_eq!(server_info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    /// Test SimpleBackend with custom error type
    #[test]
    fn test_simple_backend_custom_error() {
        #[derive(Debug, thiserror::Error)]
        enum CustomError {
            #[error("Custom error: {0}")]
            Custom(String),
        }

        impl From<BackendError> for CustomError {
            fn from(err: BackendError) -> Self {
                Self::Custom(err.to_string())
            }
        }

        impl From<CustomError> for pulseengine_mcp_protocol::Error {
            fn from(err: CustomError) -> Self {
                Self::internal_error(err.to_string())
            }
        }

        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple, error = "CustomError")]
        struct CustomErrorBackend {
            config: TestConfig,
        }

        let backend = CustomErrorBackend {
            config: TestConfig::default(),
        };

        // Should compile with custom error type
        let _server_info = backend.get_server_info();
    }

    /// Test SimpleBackend with custom config type
    #[test]
    fn test_simple_backend_custom_config() {
        #[derive(Debug, Clone)]
        struct CustomConfig {
            custom_field: String,
        }

        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple, config = "CustomConfig")]
        struct CustomConfigBackend {
            config: CustomConfig,
        }

        let backend = CustomConfigBackend {
            config: CustomConfig {
                custom_field: "test".to_string(),
            },
        };

        let _server_info = backend.get_server_info();
    }
}

#[cfg(test)]
mod full_backend_tests {
    use super::*;

    /// Test full McpBackend derive
    #[test]
    fn test_full_backend_derive() {
        #[derive(Clone, McpBackend)]
        struct FullTestBackend {
            config: TestConfig,
        }

        impl FullTestBackend {
            fn new(config: TestConfig) -> Self {
                Self { config }
            }
        }

        // This should compile and generate the full McpBackend implementation
        let backend = FullTestBackend::new(TestConfig::default());

        // Test that the generated methods work
        let server_info = backend.get_server_info();
        assert_eq!(server_info.server_info.name, env!("CARGO_PKG_NAME"));
        assert_eq!(server_info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    /// Test backend with delegate field
    #[test]
    fn test_backend_with_delegate() {
        // Create a mock inner backend
        #[derive(Clone)]
        struct InnerBackend {
            data: String,
        }

        #[async_trait::async_trait]
        impl SimpleBackend for InnerBackend {
            type Error = BackendError;
            type Config = TestConfig;

            async fn initialize(_config: Self::Config) -> Result<Self, Self::Error> {
                Ok(Self {
                    data: "inner".to_string(),
                })
            }

            fn get_server_info(&self) -> ServerInfo {
                ServerInfo {
                    protocol_version: Default::default(),
                    capabilities: Default::default(),
                    server_info: pulseengine_mcp_protocol::Implementation {
                        name: "inner-backend".to_string(),
                        version: "2.0.0".to_string(),
                    },
                    instructions: None,
                }
            }

            async fn health_check(&self) -> Result<(), Self::Error> {
                Ok(())
            }

            async fn list_tools(
                &self,
                _request: PaginatedRequestParam,
            ) -> Result<ListToolsResult, Self::Error> {
                Ok(ListToolsResult {
                    tools: vec![],
                    next_cursor: None,
                })
            }

            async fn call_tool(
                &self,
                _request: CallToolRequestParam,
            ) -> Result<CallToolResult, Self::Error> {
                Ok(CallToolResult::text("delegated"))
            }
        }

        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple)]
        struct DelegatingBackend {
            #[mcp_backend(delegate)]
            inner: InnerBackend,
            config: TestConfig,
        }

        let backend = DelegatingBackend {
            inner: InnerBackend {
                data: "test".to_string(),
            },
            config: TestConfig::default(),
        };

        // Test that delegation works
        let server_info = backend.get_server_info();
        assert_eq!(server_info.server_info.name, "inner-backend");
        assert_eq!(server_info.server_info.version, "2.0.0");
    }
}

#[cfg(test)]
mod error_generation_tests {
    use super::*;

    /// Test automatic error type generation
    #[test]
    fn test_auto_error_generation() {
        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple)]
        struct AutoErrorBackend {
            config: TestConfig,
        }

        // The derive macro should generate AutoErrorBackendError type
        let backend = AutoErrorBackend {
            config: TestConfig::default(),
        };

        // Test that the generated error type works
        let _server_info = backend.get_server_info();

        // We can't directly test the error type here, but the fact that
        // this compiles proves the error type was generated correctly
    }

    /// Test error_from attribute
    #[test]
    fn test_error_from_fields() {
        #[derive(Debug, Clone, thiserror::Error)]
        #[error("IO error")]
        struct IoWrapper;

        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple)]
        struct ErrorFromBackend {
            #[mcp_backend(error_from)]
            io_errors: Option<IoWrapper>,
            config: TestConfig,
        }

        let backend = ErrorFromBackend {
            io_errors: None,
            config: TestConfig::default(),
        };

        let _server_info = backend.get_server_info();
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    /// Test that various attribute combinations compile
    #[test]
    fn test_attribute_combinations() {
        // All attributes together
        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple, error = "BackendError", config = "TestConfig")]
        struct AllAttributesBackend {
            config: TestConfig,
        }

        // Just error attribute
        #[derive(Clone, McpBackend)]
        #[mcp_backend(error = "BackendError")]
        struct JustErrorBackend {
            config: TestConfig,
        }

        // Just config attribute
        #[derive(Clone, McpBackend)]
        #[mcp_backend(config = "TestConfig")]
        struct JustConfigBackend {
            config: TestConfig,
        }

        // No attributes (full backend)
        #[derive(Clone, McpBackend)]
        struct NoAttributesBackend {
            config: TestConfig,
        }

        // All should compile successfully
        let _b1 = AllAttributesBackend {
            config: TestConfig::default(),
        };
        let _b2 = JustErrorBackend {
            config: TestConfig::default(),
        };
        let _b3 = JustConfigBackend {
            config: TestConfig::default(),
        };
        let _b4 = NoAttributesBackend {
            config: TestConfig::default(),
        };
    }

    /// Test that the generated implementations match the trait requirements
    #[test]
    fn test_trait_requirements() {
        #[derive(Clone, McpBackend)]
        #[mcp_backend(simple)]
        struct TraitTestBackend {
            config: TestConfig,
        }

        fn assert_simple_backend<T: SimpleBackend>(_backend: &T) {}

        let backend = TraitTestBackend {
            config: TestConfig::default(),
        };

        // This should compile, proving the trait is implemented correctly
        assert_simple_backend(&backend);
    }
}

/// Run async tests
#[tokio::test]
async fn test_async_methods() {
    #[derive(Clone, McpBackend)]
    #[mcp_backend(simple)]
    struct AsyncTestBackend {
        config: TestConfig,
    }

    let backend = AsyncTestBackend {
        config: TestConfig::default(),
    };

    // Test health check
    let health_result = backend.health_check().await;
    assert!(health_result.is_ok());

    // Test list tools
    let tools_result = backend
        .list_tools(PaginatedRequestParam { cursor: None })
        .await;
    assert!(tools_result.is_ok());
    assert_eq!(tools_result.unwrap().tools.len(), 0);

    // Test call tool
    let call_result = backend
        .call_tool(CallToolRequestParam {
            name: "test".to_string(),
            arguments: None,
        })
        .await;
    assert!(call_result.is_err()); // Should return "not supported" error
}

/// Test full backend async methods
#[tokio::test]
async fn test_full_backend_async() {
    #[derive(Clone, McpBackend)]
    struct FullAsyncBackend {
        config: TestConfig,
    }

    let backend = FullAsyncBackend {
        config: TestConfig::default(),
    };

    // Test all McpBackend methods
    let resources_result = backend
        .list_resources(PaginatedRequestParam { cursor: None })
        .await;
    assert!(resources_result.is_ok());
    assert_eq!(resources_result.unwrap().resources.len(), 0);

    let prompts_result = backend
        .list_prompts(PaginatedRequestParam { cursor: None })
        .await;
    assert!(prompts_result.is_ok());
    assert_eq!(prompts_result.unwrap().prompts.len(), 0);

    let read_result = backend
        .read_resource(ReadResourceRequestParam {
            uri: "test://resource".to_string(),
        })
        .await;
    assert!(read_result.is_err()); // Should return "not supported" error

    let prompt_result = backend
        .get_prompt(GetPromptRequestParam {
            name: "test".to_string(),
            arguments: None,
        })
        .await;
    assert!(prompt_result.is_err()); // Should return "not supported" error
}
