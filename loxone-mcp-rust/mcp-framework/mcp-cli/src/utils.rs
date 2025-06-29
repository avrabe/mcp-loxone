//! Utility functions for CLI operations

use crate::CliError;
use std::path::Path;

/// Parse Cargo.toml to extract package information
#[cfg(feature = "cli")]
pub fn parse_cargo_toml<P: AsRef<Path>>(path: P) -> Result<CargoToml, CliError> {
    use std::fs;

    let content = fs::read_to_string(path)
        .map_err(|e| CliError::configuration(format!("Failed to read Cargo.toml: {e}")))?;

    let cargo_toml: CargoToml = toml::from_str(&content)
        .map_err(|e| CliError::configuration(format!("Failed to parse Cargo.toml: {e}")))?;

    Ok(cargo_toml)
}

/// Find Cargo.toml in current directory or parent directories
pub fn find_cargo_toml() -> Result<std::path::PathBuf, CliError> {
    let mut current_dir = std::env::current_dir()
        .map_err(|e| CliError::configuration(format!("Failed to get current directory: {e}")))?;

    loop {
        let cargo_toml = current_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            return Ok(cargo_toml);
        }

        if !current_dir.pop() {
            break;
        }
    }

    Err(CliError::configuration(
        "Cargo.toml not found in current directory or parents",
    ))
}

/// Parsed Cargo.toml structure
#[cfg(feature = "cli")]
#[derive(Debug, serde::Deserialize)]
pub struct CargoToml {
    pub package: Option<Package>,
}

#[cfg(feature = "cli")]
#[derive(Debug, serde::Deserialize)]
pub struct Package {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Option<Vec<String>>,
}

#[cfg(feature = "cli")]
impl CargoToml {
    pub fn get_name(&self) -> Option<&str> {
        self.package.as_ref()?.name.as_deref()
    }

    pub fn get_version(&self) -> Option<&str> {
        self.package.as_ref()?.version.as_deref()
    }

    pub fn get_description(&self) -> Option<&str> {
        self.package.as_ref()?.description.as_deref()
    }
}

/// Validate configuration values
pub mod validation {
    use crate::CliError;

    /// Validate port number
    pub fn validate_port(port: u16) -> Result<(), CliError> {
        if port == 0 {
            return Err(CliError::configuration("Port cannot be 0"));
        }
        if port < 1024 {
            tracing::warn!(
                "Using privileged port {}, this may require elevated permissions",
                port
            );
        }
        Ok(())
    }

    /// Validate URL format
    pub fn validate_url(url: &str) -> Result<(), CliError> {
        url::Url::parse(url)
            .map_err(|e| CliError::configuration(format!("Invalid URL '{url}': {e}")))?;
        Ok(())
    }

    /// Validate file path exists
    pub fn validate_file_exists(path: &str) -> Result<(), CliError> {
        if !std::path::Path::new(path).exists() {
            return Err(CliError::configuration(format!(
                "File does not exist: {path}"
            )));
        }
        Ok(())
    }

    /// Validate directory exists
    pub fn validate_dir_exists(path: &str) -> Result<(), CliError> {
        let path = std::path::Path::new(path);
        if !path.exists() {
            return Err(CliError::configuration(format!(
                "Directory does not exist: {}",
                path.display()
            )));
        }
        if !path.is_dir() {
            return Err(CliError::configuration(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_port() {
        use validation::*;

        assert!(validate_port(8080).is_ok());
        assert!(validate_port(80).is_ok()); // Should warn but not error
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validate_url() {
        use validation::*;

        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://localhost:8080").is_ok());
        assert!(validate_url("invalid-url").is_err());
    }
}
