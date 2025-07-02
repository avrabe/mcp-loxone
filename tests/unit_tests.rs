//! Unit tests for core components
//!
//! Tests individual modules and functions in isolation.
//!
//! NOTE: Some tests temporarily disabled due to API changes.

#[cfg(test)]
mod tests {
    use loxone_mcp_rust::{config::CredentialStore, error::LoxoneError};

    #[test]
    fn test_credential_store_types() {
        // Test different credential store types
        let env_store = CredentialStore::Environment;
        assert!(matches!(env_store, CredentialStore::Environment));

        let file_store = CredentialStore::FileSystem {
            path: "/tmp/test_creds.json".to_string(),
        };
        assert!(matches!(file_store, CredentialStore::FileSystem { .. }));
    }

    #[test]
    fn test_loxone_error_types() {
        // Test different error types
        let connection_error = LoxoneError::connection("Test connection error");
        assert!(connection_error
            .to_string()
            .contains("Test connection error"));

        let auth_error = LoxoneError::authentication("Test auth error");
        assert!(auth_error.to_string().contains("Test auth error"));

        let parse_error = LoxoneError::invalid_input("Test parse error");
        assert!(parse_error.to_string().contains("Test parse error"));
    }

    #[test]
    fn test_error_conversion() {
        // Test error conversion
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let loxone_error = LoxoneError::from(io_error);
        assert!(loxone_error.to_string().contains("Connection refused"));
    }
}
