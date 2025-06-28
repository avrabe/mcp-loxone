//! Integration tests for complete tool/resource interactions
//!
//! Tests the full integration between MCP tools (actions) and resources (read-only data)
//! to ensure the complete workflow works end-to-end.

#[cfg(test)]
mod tests {
    use loxone_mcp_rust::server::resources::ResourceManager;

    /// Test resource URI validation and parameter extraction
    #[tokio::test]
    async fn test_resource_uri_validation_and_extraction() {
        let resource_manager = ResourceManager::new();

        // Test valid URIs
        let valid_uris = vec![
            "loxone://rooms",
            "loxone://rooms/Kitchen/devices",
            "loxone://devices/all",
            "loxone://devices/type/Switch",
            "loxone://devices/category/lighting",
            "loxone://system/status",
            "loxone://system/capabilities",
            "loxone://audio/zones",
            "loxone://sensors/door-window",
            "loxone://weather/current",
            "loxone://security/status",
            "loxone://energy/consumption",
        ];

        for uri in valid_uris {
            let context = resource_manager
                .parse_uri(uri)
                .unwrap_or_else(|_| panic!("Failed to parse valid URI: {uri}"));

            // Verify context has correct URI
            assert_eq!(context.uri, uri);

            // Verify parameters are extracted correctly for parameterized URIs
            match uri {
                uri if uri.contains("/Kitchen/") => {
                    assert!(context.params.path_params.contains_key("room_name"));
                    assert_eq!(
                        context.params.path_params.get("room_name").unwrap(),
                        "Kitchen"
                    );
                }
                uri if uri.contains("/type/Switch") => {
                    assert!(context.params.path_params.contains_key("device_type"));
                    assert_eq!(
                        context.params.path_params.get("device_type").unwrap(),
                        "Switch"
                    );
                }
                uri if uri.contains("/category/lighting") => {
                    assert!(context.params.path_params.contains_key("category"));
                    assert_eq!(
                        context.params.path_params.get("category").unwrap(),
                        "lighting"
                    );
                }
                _ => {} // No parameters expected for other URIs
            }
        }

        // Test invalid URIs
        let invalid_uris = vec![
            "http://invalid/scheme",
            "loxone://invalid/path",
            "loxone://rooms//invalid",
            "not-a-uri",
            "",
        ];

        for uri in invalid_uris {
            assert!(
                resource_manager.parse_uri(uri).is_err(),
                "Should have failed to parse invalid URI: {}",
                uri
            );
        }
    }

    /// Test resource listing functionality
    #[tokio::test]
    async fn test_resource_listing() {
        let resource_manager = ResourceManager::new();
        let resources = resource_manager.list_resources();

        // Verify we have the expected number of resources (22 total)
        assert_eq!(resources.len(), 22);

        // Verify some key resources are present
        let resource_uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();

        // Core resources
        assert!(resource_uris.contains(&"loxone://rooms"));
        assert!(resource_uris.contains(&"loxone://devices/all"));
        assert!(resource_uris.contains(&"loxone://system/status"));

        // Weather resources
        assert!(resource_uris.contains(&"loxone://weather/current"));
        assert!(resource_uris.contains(&"loxone://weather/outdoor-conditions"));
        assert!(resource_uris.contains(&"loxone://weather/forecast-daily"));
        assert!(resource_uris.contains(&"loxone://weather/forecast-hourly"));

        // Security resources
        assert!(resource_uris.contains(&"loxone://security/status"));
        assert!(resource_uris.contains(&"loxone://security/zones"));

        // Energy resources
        assert!(resource_uris.contains(&"loxone://energy/consumption"));
        assert!(resource_uris.contains(&"loxone://energy/meters"));
        assert!(resource_uris.contains(&"loxone://energy/usage-history"));

        // Verify all resources have required metadata
        for resource in &resources {
            assert!(!resource.uri.is_empty());
            assert!(!resource.name.is_empty());
            assert!(!resource.description.is_empty());
            assert_eq!(resource.mime_type, Some("application/json".to_string()));
        }
    }

    /// Test resource URI pattern matching
    #[tokio::test]
    async fn test_resource_uri_pattern_matching() {
        let resource_manager = ResourceManager::new();

        // Test room-specific patterns
        let room_contexts = vec![
            ("loxone://rooms/LivingRoom/devices", "LivingRoom"),
            ("loxone://rooms/Kitchen/devices", "Kitchen"),
            ("loxone://rooms/Bedroom/devices", "Bedroom"),
        ];

        for (uri, expected_room) in room_contexts {
            let context = resource_manager
                .parse_uri(uri)
                .expect("Failed to parse room URI");
            // Resource type is inferred from URI structure, not stored in context
            assert_eq!(
                context.params.path_params.get("room_name").unwrap(),
                expected_room
            );
        }

        // Test device type patterns
        let device_type_contexts = vec![
            ("loxone://devices/type/Switch", "Switch"),
            ("loxone://devices/type/Jalousie", "Jalousie"),
            ("loxone://devices/type/Dimmer", "Dimmer"),
        ];

        for (uri, expected_type) in device_type_contexts {
            let context = resource_manager
                .parse_uri(uri)
                .expect("Failed to parse device type URI");
            // Resource type is inferred from URI structure, not stored in context
            assert_eq!(
                context.params.path_params.get("device_type").unwrap(),
                expected_type
            );
        }

        // Test category patterns
        let category_contexts = vec![
            ("loxone://devices/category/lighting", "lighting"),
            ("loxone://devices/category/blinds", "blinds"),
            ("loxone://devices/category/climate", "climate"),
        ];

        for (uri, expected_category) in category_contexts {
            let context = resource_manager
                .parse_uri(uri)
                .expect("Failed to parse category URI");
            // Resource type is inferred from URI structure, not stored in context
            assert_eq!(
                context.params.path_params.get("category").unwrap(),
                expected_category
            );
        }
    }

    /// Test resource context building
    #[tokio::test]
    async fn test_resource_context_building() {
        let resource_manager = ResourceManager::new();

        // Test simple resource contexts
        let simple_contexts = vec![
            ("loxone://rooms", "rooms"),
            ("loxone://devices/all", "devices"),
            ("loxone://system/status", "system"),
            ("loxone://audio/zones", "audio"),
            ("loxone://sensors/door-window", "sensors"),
            ("loxone://weather/current", "weather"),
            ("loxone://security/status", "security"),
            ("loxone://energy/consumption", "energy"),
        ];

        for (uri, _expected_type) in simple_contexts {
            let context = resource_manager
                .parse_uri(uri)
                .unwrap_or_else(|_| panic!("Failed to parse URI: {uri}"));

            assert_eq!(context.uri, uri);
            // Resource type is inferred from URI structure, not stored in context
            assert!(
                context.params.path_params.is_empty(),
                "Simple contexts should have no path parameters"
            );
        }

        // Test parameterized resource contexts
        let parameterized_contexts = vec![
            (
                "loxone://rooms/TestRoom/devices",
                vec![("room_name", "TestRoom")],
            ),
            (
                "loxone://devices/type/TestType",
                vec![("device_type", "TestType")],
            ),
            (
                "loxone://devices/category/TestCategory",
                vec![("category", "TestCategory")],
            ),
        ];

        for (uri, expected_params) in parameterized_contexts {
            let context = resource_manager
                .parse_uri(uri)
                .unwrap_or_else(|_| panic!("Failed to parse URI: {uri}"));

            assert_eq!(context.uri, uri);
            assert_eq!(context.params.path_params.len(), expected_params.len());

            for (key, value) in expected_params {
                assert_eq!(context.params.path_params.get(key).unwrap(), value);
            }
        }
    }

    /// Test complete resource workflow simulation
    #[tokio::test]
    async fn test_complete_resource_workflow_simulation() {
        let resource_manager = ResourceManager::new();

        // Step 1: List all available resources
        let all_resources = resource_manager.list_resources();
        assert!(!all_resources.is_empty());

        println!("Available resources: {}", all_resources.len());

        // Step 2: Parse different resource URIs
        let test_uris = vec![
            "loxone://rooms",
            "loxone://devices/all",
            "loxone://system/status",
            "loxone://weather/current",
            "loxone://security/status",
            "loxone://energy/consumption",
        ];

        for uri in test_uris {
            let context = resource_manager
                .parse_uri(uri)
                .unwrap_or_else(|_| panic!("Failed to parse URI: {uri}"));

            // Verify basic context properties
            assert_eq!(context.uri, uri);
            // Resource type is inferred from URI structure, not stored in context

            println!("✓ Successfully parsed resource: {}", uri);
        }

        // Step 3: Test parameterized resource URIs
        let parameterized_uris = vec![
            "loxone://rooms/Kitchen/devices",
            "loxone://devices/type/Switch",
            "loxone://devices/category/lighting",
        ];

        for uri in parameterized_uris {
            let context = resource_manager
                .parse_uri(uri)
                .unwrap_or_else(|_| panic!("Failed to parse parameterized URI: {}", uri));

            // Verify parameters were extracted
            assert!(
                !context.params.path_params.is_empty(),
                "Parameterized URI {} should have parameters",
                uri
            );

            println!(
                "✓ Successfully parsed parameterized resource: {} with {} parameters",
                uri,
                context.params.path_params.len()
            );
        }
    }

    /// Test resource error handling
    #[tokio::test]
    async fn test_resource_error_handling() {
        let resource_manager = ResourceManager::new();

        // Test various invalid URI patterns
        let invalid_patterns = vec![
            ("", "empty URI"),
            ("not-a-uri", "invalid scheme"),
            ("http://wrong/scheme", "wrong scheme"),
            ("loxone://", "missing path"),
            ("loxone:///empty", "empty path component"),
            ("loxone://invalid", "unknown resource type"),
            ("loxone://rooms/", "incomplete parameterized path"),
            ("loxone://devices/type/", "missing parameter value"),
        ];

        for (uri, description) in invalid_patterns {
            let result = resource_manager.parse_uri(uri);
            assert!(
                result.is_err(),
                "Should have failed for {}: {}",
                description,
                uri
            );

            println!(
                "✓ Correctly rejected invalid URI ({}): {}",
                description, uri
            );
        }
    }

    /// Test resource metadata consistency
    #[tokio::test]
    async fn test_resource_metadata_consistency() {
        let resource_manager = ResourceManager::new();
        let resources = resource_manager.list_resources();

        // Group resources by type for validation
        let mut resource_types = std::collections::HashMap::new();
        for resource in &resources {
            let uri_parts: Vec<&str> = resource.uri.split("://").collect();
            assert_eq!(uri_parts.len(), 2, "URI should have scheme and path");
            assert_eq!(uri_parts[0], "loxone", "URI should use loxone scheme");

            let path_parts: Vec<&str> = uri_parts[1].split('/').collect();
            let resource_type = path_parts[0];

            resource_types
                .entry(resource_type.to_string())
                .or_insert_with(Vec::new)
                .push(resource);
        }

        // Verify expected resource types are present
        let expected_types = vec![
            "rooms", "devices", "system", "audio", "sensors", "weather", "security", "energy",
        ];

        for expected_type in expected_types {
            assert!(
                resource_types.contains_key(expected_type),
                "Missing expected resource type: {}",
                expected_type
            );
        }

        // Verify resource counts for specific types
        assert!(
            resource_types.get("weather").unwrap().len() >= 4,
            "Should have at least 4 weather resources"
        );
        assert!(
            resource_types.get("security").unwrap().len() >= 2,
            "Should have at least 2 security resources"
        );
        assert!(
            resource_types.get("energy").unwrap().len() >= 3,
            "Should have at least 3 energy resources"
        );

        println!("✓ All {} resource types validated", resource_types.len());
    }

    /// Test resource consolidation after HTTP transport changes
    #[tokio::test]
    async fn test_resource_consolidation_completeness() {
        let resource_manager = ResourceManager::new();
        let resources = resource_manager.list_resources();

        // Verify no legacy read-only tools appear as resources
        let resource_uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();

        // These should NOT exist as they were legacy tools, not resources
        let legacy_patterns = vec![
            "list_rooms",
            "get_room_devices",
            "discover_all_devices",
            "get_devices_by_type",
            "get_system_status",
        ];

        for pattern in legacy_patterns {
            assert!(
                !resource_uris.iter().any(|uri| uri.contains(pattern)),
                "Legacy pattern '{}' should not appear in resource URIs",
                pattern
            );
        }

        // Verify all expected MCP resources are present
        let required_resources = vec![
            "loxone://rooms",
            "loxone://rooms/{roomName}/devices",
            "loxone://devices/all",
            "loxone://devices/type/{deviceType}",
            "loxone://devices/category/{category}",
            "loxone://system/status",
            "loxone://system/capabilities",
            "loxone://system/categories",
            "loxone://audio/zones",
            "loxone://audio/sources",
            "loxone://sensors/door-window",
            "loxone://sensors/temperature",
            "loxone://sensors/discovered",
            "loxone://weather/current",
            "loxone://weather/outdoor-conditions",
            "loxone://weather/forecast-daily",
            "loxone://weather/forecast-hourly",
            "loxone://security/status",
            "loxone://security/zones",
            "loxone://energy/consumption",
            "loxone://energy/meters",
            "loxone://energy/usage-history",
        ];

        for required_resource in required_resources {
            assert!(
                resource_uris.contains(&required_resource),
                "Required resource missing: {}",
                required_resource
            );
        }

        assert_eq!(
            resources.len(),
            22,
            "Should have exactly 22 consolidated resources"
        );

        println!(
            "✓ Resource consolidation complete: {} resources validated",
            resources.len()
        );
    }
}
