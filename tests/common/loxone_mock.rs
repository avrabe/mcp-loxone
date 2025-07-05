//! WireMock-based Loxone API mocking infrastructure
//!
//! Provides mock HTTP servers that simulate Loxone Miniserver API responses
//! for testing without requiring actual hardware.

use serde_json::{json, Value};
use std::collections::HashMap;
use wiremock::{
    matchers::{method, path, path_regex, query_param},
    Mock, MockServer, ResponseTemplate,
};

/// Mock Loxone Miniserver for testing
pub struct MockLoxoneServer {
    pub server: MockServer,
    pub base_url: String,
}

impl MockLoxoneServer {
    /// Create a new mock Loxone server with default endpoints
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let base_url = server.uri();

        let mock_server = Self { server, base_url };
        mock_server.setup_default_mocks().await;
        mock_server
    }

    /// Setup default mock endpoints that most tests will need
    async fn setup_default_mocks(&self) {
        // Mock structure file endpoint
        self.mock_structure_file().await;
        
        // Mock authentication endpoints
        self.mock_auth_endpoints().await;
        
        // Mock device state endpoints
        self.mock_device_states().await;
        
        // Mock device control endpoints
        self.mock_device_controls().await;
    }

    /// Mock the structure file endpoint
    async fn mock_structure_file(&self) {
        let structure_response = json!({
            "lastModified": "2024-01-01 12:00:00",
            "msInfo": {
                "serialNr": "TEST-12345",
                "msName": "Test Miniserver",
                "projectName": "Test Project",
                "location": "Test Location",
                "localUrl": self.base_url,
                "remoteUrl": "",
                "tempUnit": 1,
                "currency": "€",
                "squareUnit": "m²",
                "version": "14.4.10.23",
                "modified": "2024-01-01 12:00:00"
            },
            "rooms": {
                "0cd8c06b-855703-ffff-ffff000000000000": {
                    "name": "Living Room",
                    "type": 0,
                    "defaultRating": 0,
                    "isFavorite": false
                },
                "0cd8c06b-855703-ffff-ffff000000000001": {
                    "name": "Kitchen", 
                    "type": 0,
                    "defaultRating": 0,
                    "isFavorite": false
                }
            },
            "controls": {
                "0cd8c06b-855703-ffff-ffff000000000010": {
                    "name": "Living Room Light",
                    "type": "LightController",
                    "room": "0cd8c06b-855703-ffff-ffff000000000000",
                    "states": {
                        "value": "0cd8c06b-855703-ffff-ffff000000000010"
                    }
                },
                "0cd8c06b-855703-ffff-ffff000000000011": {
                    "name": "Kitchen Light",
                    "type": "LightController", 
                    "room": "0cd8c06b-855703-ffff-ffff000000000001",
                    "states": {
                        "value": "0cd8c06b-855703-ffff-ffff000000000011"
                    }
                },
                "0cd8c06b-855703-ffff-ffff000000000020": {
                    "name": "Living Room Blinds",
                    "type": "Jalousie",
                    "room": "0cd8c06b-855703-ffff-ffff000000000000", 
                    "states": {
                        "position": "0cd8c06b-855703-ffff-ffff000000000020",
                        "shadePosition": "0cd8c06b-855703-ffff-ffff000000000021"
                    }
                }
            }
        });

        Mock::given(method("GET"))
            .and(path("/data/LoxAPP3.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(structure_response))
            .mount(&self.server)
            .await;
    }

    /// Mock authentication endpoints
    async fn mock_auth_endpoints(&self) {
        // Mock token authentication
        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sys/getkey2/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sys/getkey2/test",
                    "value": "test-key-response",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;

        // Mock basic auth validation
        Mock::given(method("GET"))
            .and(path("/jdev/cfg/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/cfg/api",
                    "value": "authenticated",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock device state endpoints
    async fn mock_device_states(&self) {
        // Mock individual device state queries
        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "value": 1.0,
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;

        // Mock batch state queries
        Mock::given(method("GET"))
            .and(path("/jdev/sps/enablebinstatusupdate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sps/enablebinstatusupdate", 
                    "value": "enabled",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock device control endpoints
    async fn mock_device_controls(&self) {
        // Mock light control
        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*/On"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sps/io/Light/On",
                    "value": "1",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;

        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*/Off"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sps/io/Light/Off",
                    "value": "0", 
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;

        // Mock blind/jalousie control
        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*/FullUp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sps/io/Jalousie/FullUp",
                    "value": "1",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;

        Mock::given(method("GET"))
            .and(path_regex(r"/jdev/sps/io/.*/FullDown"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "LL": {
                    "control": "jdev/sps/io/Jalousie/FullDown", 
                    "value": "1",
                    "Code": "200"
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Add a custom mock endpoint
    pub async fn add_mock(&self, mock: Mock) {
        mock.mount(&self.server).await;
    }

    /// Get the mock server's base URL
    pub fn url(&self) -> &str {
        &self.base_url
    }

    /// Setup a mock for sensor data
    pub async fn mock_sensor_data(&self, device_uuid: &str, sensor_type: &str, value: f64) {
        let response = json!({
            "LL": {
                "value": value,
                "Code": "200"
            }
        });

        Mock::given(method("GET"))
            .and(path(format!("/jdev/sps/io/{}", device_uuid)))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&self.server)
            .await;
    }

    /// Setup a mock for error responses
    pub async fn mock_error_response(&self, path: &str, error_code: u16, message: &str) {
        let response = json!({
            "LL": {
                "control": path,
                "value": message,
                "Code": error_code.to_string()
            }
        });

        Mock::given(method("GET"))
            .and(path(path))
            .respond_with(ResponseTemplate::new(error_code).set_body_json(response))
            .mount(&self.server)
            .await;
    }
}

/// Helper function to create a mock server with common test data
pub async fn create_test_loxone_server() -> MockLoxoneServer {
    MockLoxoneServer::start().await
}

/// Helper function to create a mock server with specific device configurations
pub async fn create_mock_server_with_devices(devices: Vec<(&str, &str, &str)>) -> MockLoxoneServer {
    let server = MockLoxoneServer::start().await;
    
    for (uuid, name, device_type) in devices {
        server.mock_sensor_data(uuid, device_type, 1.0).await;
    }
    
    server
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_creation() {
        let mock_server = MockLoxoneServer::start().await;
        assert!(!mock_server.url().is_empty());
    }

    #[tokio::test] 
    async fn test_mock_structure_endpoint() {
        let mock_server = MockLoxoneServer::start().await;
        
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/data/LoxAPP3.json", mock_server.url()))
            .send()
            .await
            .unwrap();
            
        assert_eq!(response.status(), 200);
        
        let json: Value = response.json().await.unwrap();
        assert!(json["msInfo"]["serialNr"] == "TEST-12345");
    }
}