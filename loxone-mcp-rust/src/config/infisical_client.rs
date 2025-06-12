//! Infisical API client for credential management
//!
//! This module provides a native Rust implementation of the Infisical API
//! for secure credential storage and retrieval, compatible with WASM environments.

use crate::error::{LoxoneError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Infisical API client
pub struct InfisicalClient {
    client: Client,
    base_url: Url,
    access_token: Option<String>,
    project_id: String,
    environment: String,
}

/// Authentication request for universal auth
#[derive(Debug, Serialize)]
struct UniversalAuthRequest {
    client_id: String,
    client_secret: String,
}

/// Authentication response
#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
    #[allow(dead_code)]
    token_type: String,
}

/// Secret value response
#[derive(Debug, Deserialize)]
struct SecretResponse {
    secret: SecretData,
}

#[derive(Debug, Deserialize)]
struct SecretData {
    #[serde(rename = "secretValue")]
    secret_value: String,
    #[serde(rename = "secretKey")]
    secret_key: String,
    #[allow(dead_code)]
    version: u32,
}

/// List secrets response
#[derive(Debug, Deserialize)]
struct ListSecretsResponse {
    secrets: Vec<SecretData>,
}

/// Create/Update secret request
#[derive(Debug, Serialize)]
struct SecretRequest {
    #[serde(rename = "secretName")]
    secret_name: String,
    #[serde(rename = "secretValue")]
    secret_value: String,
    #[serde(rename = "secretPath")]
    secret_path: String,
    #[serde(rename = "type")]
    secret_type: String,
}

/// Error response from Infisical API
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    message: String,
    #[serde(rename = "statusCode")]
    #[allow(dead_code)]
    status_code: Option<u16>,
}

impl InfisicalClient {
    /// Create a new Infisical client
    pub fn new(
        host: Option<String>,
        project_id: String,
        environment: String,
    ) -> Result<Self> {
        let base_url = host
            .unwrap_or_else(|| "https://app.infisical.com".to_string())
            .parse()
            .map_err(|e| LoxoneError::credentials(format!("Invalid Infisical host: {}", e)))?;

        let client = Client::new();

        Ok(Self {
            client,
            base_url,
            access_token: None,
            project_id,
            environment,
        })
    }

    /// Authenticate using universal auth
    pub async fn authenticate(
        &mut self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<()> {
        let auth_url = self.base_url.join("/api/v1/auth/universal-auth/login")
            .map_err(|e| LoxoneError::credentials(format!("Failed to build auth URL: {}", e)))?;

        let request = UniversalAuthRequest {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
        };

        let response = self
            .client
            .post(auth_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Authentication request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&error_text) {
                return Err(LoxoneError::credentials(format!(
                    "Authentication failed: {}",
                    api_error.message
                )));
            }
            return Err(LoxoneError::credentials(format!(
                "Authentication failed with status {}: {}",
                status,
                error_text
            )));
        }

        let auth_response: AuthResponse = response
            .json()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse auth response: {}", e)))?;

        self.access_token = Some(auth_response.access_token);
        tracing::debug!("Successfully authenticated with Infisical");

        Ok(())
    }

    /// Get a secret by name
    pub async fn get_secret(&self, secret_name: &str) -> Result<String> {
        self.ensure_authenticated()?;

        let secret_url = self.base_url
            .join(&format!(
                "/api/v3/secrets/raw/{}?environment={}&workspaceId={}&secretPath=/",
                secret_name, self.environment, self.project_id
            ))
            .map_err(|e| LoxoneError::credentials(format!("Failed to build secret URL: {}", e)))?;

        let response = self
            .client
            .get(secret_url)
            .bearer_auth(self.access_token.as_ref().unwrap())
            .send()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to get secret: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            if status == 404 {
                return Err(LoxoneError::credentials(format!(
                    "Secret '{}' not found in Infisical",
                    secret_name
                )));
            }
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LoxoneError::credentials(format!(
                "Failed to get secret '{}': {} - {}",
                secret_name,
                status,
                error_text
            )));
        }

        let secret_response: SecretResponse = response
            .json()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse secret response: {}", e)))?;

        Ok(secret_response.secret.secret_value)
    }

    /// Create or update a secret
    pub async fn set_secret(&self, secret_name: &str, secret_value: &str) -> Result<()> {
        self.ensure_authenticated()?;

        let secrets_url = self.base_url
            .join(&format!(
                "/api/v3/secrets/raw/{}?environment={}&workspaceId={}",
                secret_name, self.environment, self.project_id
            ))
            .map_err(|e| LoxoneError::credentials(format!("Failed to build secrets URL: {}", e)))?;

        let request = SecretRequest {
            secret_name: secret_name.to_string(),
            secret_value: secret_value.to_string(),
            secret_path: "/".to_string(),
            secret_type: "shared".to_string(),
        };

        // Try to update first (PATCH), then create if it doesn't exist (POST)
        let mut response = self
            .client
            .patch(secrets_url.clone())
            .bearer_auth(self.access_token.as_ref().unwrap())
            .json(&request)
            .send()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to update secret: {}", e)))?;

        if response.status() == 404 {
            // Secret doesn't exist, create it
            let create_url = self.base_url
                .join(&format!(
                    "/api/v3/secrets/raw?environment={}&workspaceId={}",
                    self.environment, self.project_id
                ))
                .map_err(|e| LoxoneError::credentials(format!("Failed to build create URL: {}", e)))?;

            response = self
                .client
                .post(create_url)
                .bearer_auth(self.access_token.as_ref().unwrap())
                .json(&request)
                .send()
                .await
                .map_err(|e| LoxoneError::credentials(format!("Failed to create secret: {}", e)))?;
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LoxoneError::credentials(format!(
                "Failed to set secret '{}': {} - {}",
                secret_name,
                status,
                error_text
            )));
        }

        tracing::debug!("Successfully set secret '{}'", secret_name);
        Ok(())
    }

    /// Delete a secret
    pub async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        self.ensure_authenticated()?;

        let secret_url = self.base_url
            .join(&format!(
                "/api/v3/secrets/raw/{}?environment={}&workspaceId={}&secretPath=/",
                secret_name, self.environment, self.project_id
            ))
            .map_err(|e| LoxoneError::credentials(format!("Failed to build secret URL: {}", e)))?;

        let response = self
            .client
            .delete(secret_url)
            .bearer_auth(self.access_token.as_ref().unwrap())
            .send()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to delete secret: {}", e)))?;

        if !response.status().is_success() && response.status() != 404 {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LoxoneError::credentials(format!(
                "Failed to delete secret '{}': {} - {}",
                secret_name,
                status,
                error_text
            )));
        }

        tracing::debug!("Successfully deleted secret '{}'", secret_name);
        Ok(())
    }

    /// List all secrets in the project/environment
    pub async fn list_secrets(&self) -> Result<Vec<String>> {
        self.ensure_authenticated()?;

        let secrets_url = self.base_url
            .join(&format!(
                "/api/v3/secrets/raw?environment={}&workspaceId={}&secretPath=/",
                self.environment, self.project_id
            ))
            .map_err(|e| LoxoneError::credentials(format!("Failed to build secrets URL: {}", e)))?;

        let response = self
            .client
            .get(secrets_url)
            .bearer_auth(self.access_token.as_ref().unwrap())
            .send()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to list secrets: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LoxoneError::credentials(format!(
                "Failed to list secrets: {} - {}",
                status,
                error_text
            )));
        }

        let secrets_response: ListSecretsResponse = response
            .json()
            .await
            .map_err(|e| LoxoneError::credentials(format!("Failed to parse secrets list: {}", e)))?;

        Ok(secrets_response
            .secrets
            .into_iter()
            .map(|s| s.secret_key)
            .collect())
    }

    /// Ensure the client is authenticated
    fn ensure_authenticated(&self) -> Result<()> {
        if self.access_token.is_none() {
            return Err(LoxoneError::credentials(
                "Not authenticated with Infisical. Call authenticate() first."
            ));
        }
        Ok(())
    }

    /// Check if the client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    /// Get multiple secrets in a single request (more efficient)
    pub async fn get_secrets(&self, secret_names: &[&str]) -> Result<HashMap<String, String>> {
        let mut secrets = HashMap::new();

        // For now, we'll use individual requests
        // TODO: Implement batch API when Infisical supports it
        for &secret_name in secret_names {
            match self.get_secret(secret_name).await {
                Ok(value) => {
                    secrets.insert(secret_name.to_string(), value);
                }
                Err(e) => {
                    tracing::warn!("Failed to get secret '{}': {}", secret_name, e);
                    // Continue with other secrets
                }
            }
        }

        Ok(secrets)
    }
}

/// Convenience function to create an authenticated Infisical client
pub async fn create_authenticated_client(
    project_id: String,
    environment: String,
    client_id: String,
    client_secret: String,
    host: Option<String>,
) -> Result<InfisicalClient> {
    let mut client = InfisicalClient::new(host, project_id, environment)?;
    client.authenticate(&client_id, &client_secret).await?;
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infisical_client_creation() {
        let client = InfisicalClient::new(
            None,
            "test-project".to_string(),
            "dev".to_string(),
        );
        assert!(client.is_ok());
        
        let client = client.unwrap();
        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_custom_host() {
        let client = InfisicalClient::new(
            Some("https://my-infisical.com".to_string()),
            "test-project".to_string(),
            "dev".to_string(),
        );
        assert!(client.is_ok());
    }
}