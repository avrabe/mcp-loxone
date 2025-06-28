//! Request validation utilities

use pulseengine_mcp_protocol::{Error, Request};

/// Request validator
pub struct RequestValidator;

impl RequestValidator {
    /// Validate an MCP request
    pub fn validate_request(request: &Request) -> Result<(), Error> {
        // Basic validation
        if request.jsonrpc != "2.0" {
            return Err(Error::invalid_request("Invalid JSON-RPC version"));
        }

        if request.method.is_empty() {
            return Err(Error::invalid_request("Method cannot be empty"));
        }

        Ok(())
    }
}
