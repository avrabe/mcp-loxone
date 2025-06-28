//! Comprehensive inline documentation and examples for all MCP tools
//!
//! This module provides detailed usage examples and documentation for each
//! MCP tool in the Loxone MCP server. Each example includes:
//! - Tool purpose and use cases
//! - Parameter descriptions
//! - Expected responses
//! - Error scenarios
//! - Best practices

use serde_json::json;

/// Room Management Tool Examples
pub mod room_examples {

    /// List all rooms in the Loxone system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "list_rooms",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": [
    ///     {
    ///       "uuid": "room-living",
    ///       "name": "Living Room",
    ///       "device_count": 5,
    ///       "devices": {
    ///         "lights": 2,
    ///         "blinds": 2,
    ///         "sensors": 1
    ///       }
    ///     },
    ///     {
    ///       "uuid": "room-kitchen",
    ///       "name": "Kitchen",
    ///       "device_count": 3,
    ///       "devices": {
    ///         "lights": 2,
    ///         "sensors": 1
    ///       }
    ///     }
    ///   ],
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    ///
    /// # Use Cases
    /// - Display all available rooms in UI
    /// - Navigate room structure
    /// - Count total devices per room
    /// - Filter rooms by device types
    pub const LIST_ROOMS_EXAMPLE: &str = r#"
Tool: list_rooms
Purpose: Retrieve all rooms with device counts

No parameters required.

Returns: Array of room objects with UUIDs, names, and device breakdowns.
"#;

    /// Get all devices in a specific room
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_room_devices",
    ///   "arguments": {
    ///     "room_name": "Living Room",
    ///     "device_type": "light"  // Optional filter
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "room": "Living Room",
    ///     "devices": [
    ///       {
    ///         "uuid": "light-living-main",
    ///         "name": "Main Light",
    ///         "type": "Switch",
    ///         "category": "lighting",
    ///         "state": {
    ///           "value": 1,
    ///           "brightness": 100
    ///         }
    ///       },
    ///       {
    ///         "uuid": "light-living-ambient",
    ///         "name": "Ambient Light",
    ///         "type": "Dimmer",
    ///         "category": "lighting",
    ///         "state": {
    ///           "value": 1,
    ///           "brightness": 60
    ///         }
    ///       }
    ///     ],
    ///     "count": 2
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    ///
    /// # Parameters
    /// - `room_name` (required): Name of the room (case-insensitive)
    /// - `device_type` (optional): Filter by device type (light, blind, sensor, etc.)
    ///
    /// # Error Cases
    /// - Room not found: Returns empty device list with context
    /// - Invalid device type: Ignores filter, returns all devices
    pub const GET_ROOM_DEVICES_EXAMPLE: &str = r#"
Tool: get_room_devices
Purpose: List all devices in a specific room with optional filtering

Parameters:
  - room_name (string, required): Name of the room
  - device_type (string, optional): Filter by type (light, blind, sensor)

Returns: Devices in the room with current states and metadata.
"#;
}

/// Device Control Tool Examples
pub mod device_examples {

    /// Control a single device by UUID or name
    ///
    /// # Example Request - Light Control
    /// ```json
    /// {
    ///   "tool": "control_device",
    ///   "arguments": {
    ///     "device": "Living Room Light",
    ///     "action": "on",
    ///     "room": "Living Room"  // Optional, helps disambiguation
    ///   }
    /// }
    /// ```
    ///
    /// # Example Request - Blind Control
    /// ```json
    /// {
    ///   "tool": "control_device",
    ///   "arguments": {
    ///     "device": "bedroom-blind-01",
    ///     "action": "down"
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "device": "Living Room Light",
    ///     "uuid": "light-living-main",
    ///     "action": "on",
    ///     "previous_state": 0,
    ///     "new_state": 1,
    ///     "response": {
    ///       "code": 200,
    ///       "value": 1
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    ///
    /// # Parameters
    /// - `device` (required): Device UUID or name
    /// - `action` (required): Action to perform
    /// - `room` (optional): Room name to help identify device
    ///
    /// # Valid Actions
    /// - Lights: on, off, toggle, pulse
    /// - Blinds: up, down, stop, fullup, fulldown
    /// - Dimmers: on, off, dim (with value)
    /// - General: depends on device type
    ///
    /// # Error Cases
    /// - Device not found
    /// - Invalid action for device type
    /// - Device not responding
    /// - Permission denied
    pub const CONTROL_DEVICE_EXAMPLE: &str = r#"
Tool: control_device
Purpose: Control a single Loxone device

Parameters:
  - device (string, required): UUID or name of device
  - action (string, required): Action to perform (on/off/up/down/stop)
  - room (string, optional): Room name for disambiguation

Common actions:
  - Lights: on, off, toggle, pulse
  - Blinds: up, down, stop, fullup, fulldown
  - Switches: on, off, toggle

Returns: Device info with previous/new state and response details.
"#;

    /// Control all lights in the system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "control_all_lights",
    ///   "arguments": {
    ///     "action": "off"
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "action": "off",
    ///     "total_lights": 8,
    ///     "successful": 8,
    ///     "failed": 0,
    ///     "results": [
    ///       {
    ///         "uuid": "light-living-main",
    ///         "name": "Living Room Main Light",
    ///         "room": "Living Room",
    ///         "success": true,
    ///         "previous_state": 1,
    ///         "new_state": 0
    ///       },
    ///       {
    ///         "uuid": "light-kitchen-ceiling",
    ///         "name": "Kitchen Ceiling",
    ///         "room": "Kitchen",
    ///         "success": true,
    ///         "previous_state": 1,
    ///         "new_state": 0
    ///       }
    ///     ]
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const CONTROL_ALL_LIGHTS_EXAMPLE: &str = r#"
Tool: control_all_lights
Purpose: Control all lights in the entire system simultaneously

Parameters:
  - action (string, required): on or off

Use cases:
  - "All lights off" when leaving home
  - Emergency lighting (all on)
  - Energy saving mode

Returns: Summary with total lights affected and individual results.
"#;

    /// Control lights in a specific room
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "control_room_lights",
    ///   "arguments": {
    ///     "room": "Living Room",
    ///     "action": "on"
    ///   }
    /// }
    /// ```
    pub const CONTROL_ROOM_LIGHTS_EXAMPLE: &str = r#"
Tool: control_room_lights
Purpose: Control all lights in a specific room

Parameters:
  - room (string, required): Room name
  - action (string, required): on or off

Returns: Summary of lights controlled in the room.
"#;
}

/// Blind/Rolladen Control Examples
pub mod blind_examples {

    /// Control all blinds/rolladen in the system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "control_all_rolladen",
    ///   "arguments": {
    ///     "action": "down"
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "action": "down",
    ///     "total_blinds": 5,
    ///     "successful": 5,
    ///     "failed": 0,
    ///     "results": [
    ///       {
    ///         "uuid": "blind-living-window",
    ///         "name": "Living Room Window",
    ///         "room": "Living Room",
    ///         "success": true,
    ///         "position": 0
    ///       }
    ///     ]
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const CONTROL_ALL_ROLLADEN_EXAMPLE: &str = r#"
Tool: control_all_rolladen
Purpose: Control all blinds/rolladen in the entire system

Parameters:
  - action (string, required): up, down, or stop

Actions:
  - up: Open blinds (100% open)
  - down: Close blinds (0% open)
  - stop: Stop current movement

Use cases:
  - Morning routine: all blinds up
  - Night security: all blinds down
  - Emergency stop during movement

Returns: Summary with total blinds affected and positions.
"#;

    /// Control blinds in a specific room
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "control_room_rolladen",
    ///   "arguments": {
    ///     "room": "Bedroom",
    ///     "action": "up"
    ///   }
    /// }
    /// ```
    pub const CONTROL_ROOM_ROLLADEN_EXAMPLE: &str = r#"
Tool: control_room_rolladen
Purpose: Control all blinds/rolladen in a specific room

Parameters:
  - room (string, required): Room name
  - action (string, required): up, down, or stop

Returns: Summary of blinds controlled in the room.
"#;
}

/// Discovery and System Information Examples
pub mod discovery_examples {

    /// Discover all devices in the system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "discover_all_devices",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "total_devices": 25,
    ///     "by_type": {
    ///       "Switch": 8,
    ///       "Dimmer": 2,
    ///       "Jalousie": 5,
    ///       "InfoOnlyDigital": 10
    ///     },
    ///     "by_room": {
    ///       "Living Room": 5,
    ///       "Kitchen": 3,
    ///       "Bedroom": 4
    ///     },
    ///     "devices": [
    ///       {
    ///         "uuid": "light-living-main",
    ///         "name": "Living Room Main Light",
    ///         "type": "Switch",
    ///         "room": "Living Room",
    ///         "category": "lighting",
    ///         "controls": ["on", "off", "toggle"]
    ///       }
    ///     ]
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const DISCOVER_ALL_DEVICES_EXAMPLE: &str = r#"
Tool: discover_all_devices
Purpose: Discover and list all devices in the system with detailed information

No parameters required.

Returns comprehensive device inventory including:
  - Total device count
  - Breakdown by device type
  - Breakdown by room
  - Full device list with metadata

Use cases:
  - System inventory
  - Device audit
  - Finding specific device types
  - Understanding system capabilities
"#;

    /// Get devices filtered by type
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_devices_by_type",
    ///   "arguments": {
    ///     "device_type": "Jalousie"
    ///   }
    /// }
    /// ```
    pub const GET_DEVICES_BY_TYPE_EXAMPLE: &str = r#"
Tool: get_devices_by_type
Purpose: Get all devices of a specific type

Parameters:
  - device_type (string, optional): Device type filter (e.g., Switch, Jalousie, Dimmer)

Common device types:
  - Switch: On/off lights and switches
  - Dimmer: Dimmable lights
  - Jalousie: Blinds and rolladen
  - InfoOnlyDigital: Binary sensors
  - InfoOnlyAnalog: Analog sensors

Returns: List of devices matching the type filter.
"#;

    /// Get system status and health
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_system_status",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "connection": {
    ///       "status": "connected",
    ///       "host": "192.168.1.100",
    ///       "latency_ms": 15
    ///     },
    ///     "system": {
    ///       "version": "12.3.4.5",
    ///       "serial": "502F12345678",
    ///       "project": "Smart Home",
    ///       "uptime_days": 45
    ///     },
    ///     "devices": {
    ///       "total": 25,
    ///       "online": 25,
    ///       "offline": 0
    ///     },
    ///     "performance": {
    ///       "cpu_load": 15,
    ///       "memory_usage": 45,
    ///       "temperature": 42
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_SYSTEM_STATUS_EXAMPLE: &str = r#"
Tool: get_system_status
Purpose: Get overall system status and health information

No parameters required.

Returns system information including:
  - Connection status
  - Miniserver version and details
  - Device statistics
  - Performance metrics

Use cases:
  - Health monitoring
  - Troubleshooting
  - System dashboard
"#;
}

/// Audio Control Examples
pub mod audio_examples {

    /// Get audio zones and status
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_audio_zones",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": [
    ///     {
    ///       "uuid": "audio-zone-living",
    ///       "name": "Living Room Audio",
    ///       "room": "Living Room",
    ///       "state": {
    ///         "power": true,
    ///         "volume": 45,
    ///         "muted": false,
    ///         "source": "Spotify",
    ///         "playing": true,
    ///         "track": "Favorite Song"
    ///       }
    ///     }
    ///   ],
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_AUDIO_ZONES_EXAMPLE: &str = r#"
Tool: get_audio_zones
Purpose: Get all audio zones and their current playback status

No parameters required.

Returns audio zone information including:
  - Zone names and locations
  - Power and volume status
  - Current source and track
  - Playback state

Use cases:
  - Multi-room audio control
  - Volume management
  - Source selection
"#;

    /// Control audio zone
    ///
    /// # Example Requests
    /// ```json
    /// // Play/pause control
    /// {
    ///   "tool": "control_audio_zone",
    ///   "arguments": {
    ///     "zone_name": "Living Room Audio",
    ///     "action": "play"
    ///   }
    /// }
    ///
    /// // Volume control
    /// {
    ///   "tool": "control_audio_zone",
    ///   "arguments": {
    ///     "zone_name": "Living Room Audio",
    ///     "action": "volume",
    ///     "value": 60
    ///   }
    /// }
    /// ```
    pub const CONTROL_AUDIO_ZONE_EXAMPLE: &str = r#"
Tool: control_audio_zone
Purpose: Control an audio zone (play, stop, volume control)

Parameters:
  - zone_name (string, required): Name of the audio zone
  - action (string, required): Action to perform
  - value (number, optional): Value for volume actions (0-100)

Actions:
  - play: Start playback
  - stop: Stop playback
  - pause: Pause playback
  - volume: Set volume (requires value)
  - mute: Mute audio
  - unmute: Unmute audio
  - next: Next track
  - previous: Previous track

Returns: Zone status after the action.
"#;
}

/// Sensor and Climate Examples
pub mod sensor_examples {

    /// Get all door/window sensors
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_all_door_window_sensors",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "sensors": [
    ///       {
    ///         "uuid": "sensor-front-door",
    ///         "name": "Front Door",
    ///         "room": "Entrance",
    ///         "state": "closed",
    ///         "value": 0,
    ///         "last_changed": "2024-01-15T09:15:00Z"
    ///       },
    ///       {
    ///         "uuid": "sensor-kitchen-window",
    ///         "name": "Kitchen Window",
    ///         "room": "Kitchen",
    ///         "state": "open",
    ///         "value": 1,
    ///         "last_changed": "2024-01-15T10:00:00Z"
    ///       }
    ///     ],
    ///     "summary": {
    ///       "total": 8,
    ///       "open": 1,
    ///       "closed": 7
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_DOOR_WINDOW_SENSORS_EXAMPLE: &str = r#"
Tool: get_all_door_window_sensors
Purpose: Get status of all door and window sensors

No parameters required.

Returns sensor information including:
  - Current state (open/closed)
  - Numeric value (0=closed, 1=open)
  - Last state change time
  - Summary statistics

Use cases:
  - Security monitoring
  - Energy efficiency (open windows)
  - Home automation rules
"#;

    /// Get temperature sensors
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_temperature_sensors",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "sensors": [
    ///       {
    ///         "uuid": "temp-living-room",
    ///         "name": "Living Room Temperature",
    ///         "room": "Living Room",
    ///         "value": 21.5,
    ///         "unit": "°C",
    ///         "target": 22.0,
    ///         "heating": true
    ///       }
    ///     ],
    ///     "average_temperature": 21.2
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_TEMPERATURE_SENSORS_EXAMPLE: &str = r#"
Tool: get_temperature_sensors
Purpose: Get all temperature sensors and their readings

No parameters required.

Returns temperature data including:
  - Current temperature values
  - Target temperatures (if set)
  - Heating/cooling status
  - Room assignments
  - Average temperature

Use cases:
  - Climate monitoring
  - Energy optimization
  - Comfort analysis
"#;
}

/// Health Check Examples
pub mod health_examples {

    /// Perform comprehensive health check
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_health_check",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "overall_status": "healthy",
    ///     "checks": {
    ///       "connectivity": {
    ///         "status": "pass",
    ///         "latency_ms": 12,
    ///         "message": "Connection stable"
    ///       },
    ///       "authentication": {
    ///         "status": "pass",
    ///         "message": "Authentication valid"
    ///       },
    ///       "api_response": {
    ///         "status": "pass",
    ///         "response_time_ms": 45,
    ///         "message": "API responding normally"
    ///       },
    ///       "device_communication": {
    ///         "status": "pass",
    ///         "responsive_devices": 25,
    ///         "total_devices": 25,
    ///         "message": "All devices responding"
    ///       }
    ///     },
    ///     "metrics": {
    ///       "uptime_seconds": 3600,
    ///       "requests_handled": 150,
    ///       "error_rate": 0.02
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const HEALTH_CHECK_EXAMPLE: &str = r#"
Tool: get_health_check
Purpose: Perform comprehensive health check of the Loxone system and MCP server

No parameters required.

Performs multiple health checks:
  - Connection stability
  - Authentication validity
  - API response times
  - Device communication
  - Server metrics

Returns detailed health status for monitoring and diagnostics.
"#;

    /// Get basic health status
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_health_status",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "healthy": true,
    ///     "connected": true,
    ///     "latency_ms": 15
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const HEALTH_STATUS_EXAMPLE: &str = r#"
Tool: get_health_status
Purpose: Get basic health status (lightweight check)

No parameters required.

Quick health check returning:
  - Overall health status
  - Connection state
  - Network latency

Use for frequent polling or simple health monitoring.
"#;
}

/// Climate Control Examples
pub mod climate_examples {

    /// Get climate control overview
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_climate_control",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "total_devices": 15,
    ///     "room_controllers": [
    ///       {
    ///         "uuid": "climate-living-room",
    ///         "name": "Living Room Climate",
    ///         "room": "Living Room",
    ///         "current_temperature": 21.5,
    ///         "target_temperature": 22.0,
    ///         "mode": "heating"
    ///       }
    ///     ],
    ///     "average_temperature": 21.2,
    ///     "system_mode": "auto"
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_CLIMATE_CONTROL_EXAMPLE: &str = r#"
Tool: get_climate_control
Purpose: Get overview of all climate control devices and their status

No parameters required.

Returns climate system information including:
  - Room controllers with temperature data
  - Temperature sensors
  - Heating/cooling devices
  - System operating mode
  - Average temperatures

Use cases:
  - Climate system overview
  - Energy optimization
  - Comfort monitoring
"#;

    /// Get room climate information
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_room_climate",
    ///   "arguments": {
    ///     "room_name": "Living Room"
    ///   }
    /// }
    /// ```
    pub const GET_ROOM_CLIMATE_EXAMPLE: &str = r#"
Tool: get_room_climate
Purpose: Get climate information for a specific room

Parameters:
  - room_name (string, required): Name of the room

Returns room climate data including:
  - Current temperature
  - Target temperature
  - Humidity levels
  - Operating mode
  - Climate devices

Use cases:
  - Room comfort monitoring
  - Individual room control
  - Climate optimization
"#;

    /// Set room temperature
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "set_room_temperature",
    ///   "arguments": {
    ///     "room_name": "Living Room",
    ///     "temperature": 22.5
    ///   }
    /// }
    /// ```
    pub const SET_ROOM_TEMPERATURE_EXAMPLE: &str = r#"
Tool: set_room_temperature
Purpose: Set target temperature for a specific room

Parameters:
  - room_name (string, required): Name of the room
  - temperature (number, required): Target temperature in Celsius

Valid temperature range: 10-30°C

Returns:
  - Previous temperature setting
  - New temperature setting
  - Estimated time to reach target

Use cases:
  - Comfort adjustment
  - Energy saving modes
  - Schedule-based temperature control
"#;

    /// Get temperature readings
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_temperature_readings",
    ///   "arguments": {}
    /// }
    /// ```
    pub const GET_TEMPERATURE_READINGS_EXAMPLE: &str = r#"
Tool: get_temperature_readings
Purpose: Get all temperature sensor readings across the system

No parameters required.

Returns temperature data from all sensors including:
  - Indoor temperatures by room
  - Outdoor temperature
  - Average temperatures
  - Temperature trends

Use cases:
  - System-wide temperature monitoring
  - Heat map visualization
  - Energy efficiency analysis
"#;

    /// Set room climate mode
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "set_room_mode",
    ///   "arguments": {
    ///     "room_name": "Living Room",
    ///     "mode": "eco"
    ///   }
    /// }
    /// ```
    pub const SET_ROOM_MODE_EXAMPLE: &str = r#"
Tool: set_room_mode
Purpose: Set operating mode for room climate control

Parameters:
  - room_name (string, required): Name of the room
  - mode (string, required): Operating mode

Valid modes:
  - auto: Automatic control
  - heating: Heating only
  - cooling: Cooling only
  - eco: Energy saving mode
  - off: Disable climate control

Returns confirmation and new mode status.
"#;
}

/// Energy Management Examples
pub mod energy_examples {

    /// Get energy consumption
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_energy_consumption",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "current_power": 3450,
    ///     "daily_consumption": 28.5,
    ///     "monthly_consumption": 425.2,
    ///     "by_category": {
    ///       "lighting": 450,
    ///       "heating": 1200,
    ///       "appliances": 1800
    ///     },
    ///     "peak_today": 5200,
    ///     "cost_estimate": {
    ///       "daily": 8.55,
    ///       "monthly": 127.56,
    ///       "currency": "EUR"
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_ENERGY_CONSUMPTION_EXAMPLE: &str = r#"
Tool: get_energy_consumption
Purpose: Get current energy consumption and usage statistics

No parameters required.

Returns energy data including:
  - Current power draw (watts)
  - Daily/monthly consumption (kWh)
  - Breakdown by category
  - Peak usage information
  - Cost estimates

Use cases:
  - Energy monitoring dashboard
  - Cost tracking
  - Identifying high consumers
  - Optimization opportunities
"#;

    /// Get power meters
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_power_meters",
    ///   "arguments": {}
    /// }
    /// ```
    pub const GET_POWER_METERS_EXAMPLE: &str = r#"
Tool: get_power_meters
Purpose: Get readings from all power meters in the system

No parameters required.

Returns power meter data including:
  - Individual meter readings
  - Circuit breaker status
  - Phase information
  - Power quality metrics

Use cases:
  - Detailed power monitoring
  - Load balancing
  - Electrical system health
  - Circuit-level analysis
"#;

    /// Get solar production
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_solar_production",
    ///   "arguments": {}
    /// }
    /// ```
    pub const GET_SOLAR_PRODUCTION_EXAMPLE: &str = r#"
Tool: get_solar_production
Purpose: Get solar panel production data (if available)

No parameters required.

Returns solar data including:
  - Current production (watts)
  - Daily/monthly generation
  - Efficiency metrics
  - Grid feed-in data
  - Self-consumption rate

Use cases:
  - Solar monitoring
  - ROI tracking
  - Energy independence metrics
  - Grid interaction monitoring
"#;

    /// Optimize energy usage
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "optimize_energy_usage",
    ///   "arguments": {}
    /// }
    /// ```
    pub const OPTIMIZE_ENERGY_USAGE_EXAMPLE: &str = r#"
Tool: optimize_energy_usage
Purpose: Get energy optimization recommendations

No parameters required.

Analyzes current usage and provides:
  - Optimization suggestions
  - Potential savings
  - Device scheduling recommendations
  - Peak load shifting opportunities

Use cases:
  - Energy cost reduction
  - Load management
  - Sustainability improvements
  - Smart scheduling
"#;
}

/// Security System Examples
pub mod security_examples {

    /// Get alarm status
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_alarm_status",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "armed": true,
    ///     "mode": "away",
    ///     "zones": [
    ///       {
    ///         "name": "Perimeter",
    ///         "status": "armed",
    ///         "sensors": 12
    ///       },
    ///       {
    ///         "name": "Interior",
    ///         "status": "bypassed",
    ///         "sensors": 8
    ///       }
    ///     ],
    ///     "last_armed": "2024-01-15T08:00:00Z",
    ///     "alerts": []
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_ALARM_STATUS_EXAMPLE: &str = r#"
Tool: get_alarm_status
Purpose: Get current alarm system status and zone information

No parameters required.

Returns alarm status including:
  - Armed/disarmed state
  - Active mode (home/away/night)
  - Zone status
  - Recent alerts
  - Last status change

Use cases:
  - Security monitoring
  - Status dashboard
  - Alert management
  - Zone control
"#;

    /// Arm alarm system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "arm_alarm",
    ///   "arguments": {
    ///     "mode": "away"
    ///   }
    /// }
    /// ```
    pub const ARM_ALARM_EXAMPLE: &str = r#"
Tool: arm_alarm
Purpose: Arm the alarm system in specified mode

Parameters:
  - mode (string, required): Arming mode

Valid modes:
  - away: Full system armed
  - home: Perimeter only
  - night: Night mode with motion bypass

Returns:
  - Confirmation of arming
  - Active zones
  - Exit delay information

Use cases:
  - Departure routines
  - Night security
  - Scheduled arming
"#;

    /// Disarm alarm system
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "disarm_alarm",
    ///   "arguments": {}
    /// }
    /// ```
    pub const DISARM_ALARM_EXAMPLE: &str = r#"
Tool: disarm_alarm
Purpose: Disarm the alarm system

No parameters required.

Returns:
  - Confirmation of disarming
  - Previous mode
  - Any triggered alerts during armed period

Use cases:
  - Arrival routines
  - Emergency disarming
  - Scheduled disarming
"#;

    /// Get security cameras
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_security_cameras",
    ///   "arguments": {}
    /// }
    /// ```
    pub const GET_SECURITY_CAMERAS_EXAMPLE: &str = r#"
Tool: get_security_cameras
Purpose: Get status of security cameras (if integrated)

No parameters required.

Returns camera information including:
  - Camera locations
  - Online/offline status
  - Recording status
  - Motion detection state

Use cases:
  - Camera monitoring
  - Recording verification
  - System health check
"#;
}

/// Weather Station Examples
pub mod weather_examples {

    /// Get weather data
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_weather_data",
    ///   "arguments": {}
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "status": "success",
    ///   "data": {
    ///     "current": {
    ///       "temperature": 18.5,
    ///       "humidity": 65,
    ///       "pressure": 1013.2,
    ///       "wind_speed": 12.5,
    ///       "wind_direction": "NW",
    ///       "rainfall": 0.0,
    ///       "uv_index": 3
    ///     },
    ///     "indoor": {
    ///       "temperature": 21.5,
    ///       "humidity": 45
    ///     },
    ///     "station": {
    ///       "name": "Home Weather Station",
    ///       "last_update": "2024-01-15T10:29:00Z"
    ///     }
    ///   },
    ///   "timestamp": "2024-01-15T10:30:00Z"
    /// }
    /// ```
    pub const GET_WEATHER_DATA_EXAMPLE: &str = r#"
Tool: get_weather_data
Purpose: Get current weather data from integrated weather station

No parameters required.

Returns weather data including:
  - Outdoor conditions (temp, humidity, pressure)
  - Wind information
  - Rainfall data
  - UV index
  - Indoor comparison

Use cases:
  - Weather monitoring
  - Climate automation triggers
  - Garden/irrigation control
  - Energy optimization
"#;

    /// Get outdoor conditions
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_outdoor_conditions",
    ///   "arguments": {}
    /// }
    /// ```
    pub const GET_OUTDOOR_CONDITIONS_EXAMPLE: &str = r#"
Tool: get_outdoor_conditions
Purpose: Get simplified outdoor conditions summary

No parameters required.

Returns:
  - Temperature
  - Weather description
  - Key conditions for automation

Use cases:
  - Quick status check
  - Automation decisions
  - Display panels
"#;

    /// Get daily weather forecast
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_weather_forecast_daily",
    ///   "arguments": {
    ///     "days": 3
    ///   }
    /// }
    /// ```
    pub const GET_WEATHER_FORECAST_DAILY_EXAMPLE: &str = r#"
Tool: get_weather_forecast_daily
Purpose: Get daily weather forecast

Parameters:
  - days (number, optional): Number of days (1-7, default 3)

Returns daily forecast including:
  - High/low temperatures
  - Precipitation chance
  - Weather conditions
  - Wind forecasts

Use cases:
  - Planning automation
  - Energy optimization
  - Irrigation scheduling
"#;

    /// Get hourly weather forecast
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "tool": "get_weather_forecast_hourly",
    ///   "arguments": {
    ///     "hours": 12
    ///   }
    /// }
    /// ```
    pub const GET_WEATHER_FORECAST_HOURLY_EXAMPLE: &str = r#"
Tool: get_weather_forecast_hourly
Purpose: Get hourly weather forecast

Parameters:
  - hours (number, optional): Number of hours (1-48, default 24)

Returns hourly forecast including:
  - Temperature progression
  - Precipitation timing
  - Wind changes
  - Condition changes

Use cases:
  - Precise automation timing
  - Event planning
  - Energy usage planning
"#;
}

/// Tool Usage Best Practices
pub mod best_practices {

    pub const GENERAL_BEST_PRACTICES: &str = r#"
# MCP Tool Best Practices for Loxone Control

## 1. Device Identification
- Use device names for user-friendly control
- Use UUIDs for precise, unambiguous control
- Provide room context when using names to avoid ambiguity

## 2. Error Handling
- Tools return success with empty data rather than errors when possible
- Check the 'status' field in responses
- Look for 'message' field for additional context

## 3. Performance Optimization
- Use batch operations (control_all_*, control_room_*) when possible
- Request coalescing happens automatically for concurrent requests
- Rate limiting protects the Miniserver from overload

## 4. State Management
- Device states are fetched on-demand (not cached indefinitely)
- Use get_device_info for current state before control operations
- Monitor state changes through response data

## 5. Schema Validation
- All parameters are validated before execution
- UUID format: 12345678-1234-1234-1234-123456789abc or 12345678.1234.5678
- Actions are case-insensitive but device-type specific

## 6. Common Patterns

### Turning off all lights when leaving:
1. control_all_lights with action "off"

### Setting room ambiance:
1. get_room_devices to list devices
2. control_device for each light/blind as needed

### Security check:
1. get_all_door_window_sensors to check all entries
2. control_all_rolladen to secure blinds if needed

### Climate control:
1. get_temperature_sensors to check current temps
2. set_room_temperature to adjust as needed

## 7. Monitoring and Observability
- All requests are logged with structured fields
- Request IDs enable tracing through the system
- Metrics track performance and error rates
- Health checks monitor system status
"#;

    pub const ERROR_HANDLING_GUIDE: &str = r#"
# Error Handling Guide

## Common Error Types

### Device Not Found
- Response: Success with empty data and context message
- Handling: Check device name/UUID, verify room context

### Invalid Action
- Response: Error with validation message
- Handling: Check valid actions for device type

### Connection Issues
- Response: Error with connection details
- Handling: Check network, verify credentials

### Rate Limiting
- Response: Error with retry information
- Handling: Implement exponential backoff

### Authentication Failures
- Response: Error with auth details
- Handling: Re-authenticate, check credentials

## Best Practices

1. Always check response.status
2. Parse response.data for results
3. Use response.message for user feedback
4. Log errors with request context
5. Implement retries for transient failures
"#;
}

/// Generate tool documentation summary
pub fn generate_tool_documentation() -> serde_json::Value {
    json!({
        "tools": {
            "room_management": {
                "list_rooms": room_examples::LIST_ROOMS_EXAMPLE,
                "get_room_devices": room_examples::GET_ROOM_DEVICES_EXAMPLE
            },
            "device_control": {
                "control_device": device_examples::CONTROL_DEVICE_EXAMPLE,
                "control_all_lights": device_examples::CONTROL_ALL_LIGHTS_EXAMPLE,
                "control_room_lights": device_examples::CONTROL_ROOM_LIGHTS_EXAMPLE
            },
            "blind_control": {
                "control_all_rolladen": blind_examples::CONTROL_ALL_ROLLADEN_EXAMPLE,
                "control_room_rolladen": blind_examples::CONTROL_ROOM_ROLLADEN_EXAMPLE
            },
            "discovery": {
                "discover_all_devices": discovery_examples::DISCOVER_ALL_DEVICES_EXAMPLE,
                "get_devices_by_type": discovery_examples::GET_DEVICES_BY_TYPE_EXAMPLE,
                "get_system_status": discovery_examples::GET_SYSTEM_STATUS_EXAMPLE
            },
            "audio": {
                "get_audio_zones": audio_examples::GET_AUDIO_ZONES_EXAMPLE,
                "control_audio_zone": audio_examples::CONTROL_AUDIO_ZONE_EXAMPLE
            },
            "sensors": {
                "get_all_door_window_sensors": sensor_examples::GET_DOOR_WINDOW_SENSORS_EXAMPLE,
                "get_temperature_sensors": sensor_examples::GET_TEMPERATURE_SENSORS_EXAMPLE
            },
            "climate": {
                "get_climate_control": climate_examples::GET_CLIMATE_CONTROL_EXAMPLE,
                "get_room_climate": climate_examples::GET_ROOM_CLIMATE_EXAMPLE,
                "set_room_temperature": climate_examples::SET_ROOM_TEMPERATURE_EXAMPLE,
                "get_temperature_readings": climate_examples::GET_TEMPERATURE_READINGS_EXAMPLE,
                "set_room_mode": climate_examples::SET_ROOM_MODE_EXAMPLE
            },
            "energy": {
                "get_energy_consumption": energy_examples::GET_ENERGY_CONSUMPTION_EXAMPLE,
                "get_power_meters": energy_examples::GET_POWER_METERS_EXAMPLE,
                "get_solar_production": energy_examples::GET_SOLAR_PRODUCTION_EXAMPLE,
                "optimize_energy_usage": energy_examples::OPTIMIZE_ENERGY_USAGE_EXAMPLE
            },
            "security": {
                "get_alarm_status": security_examples::GET_ALARM_STATUS_EXAMPLE,
                "arm_alarm": security_examples::ARM_ALARM_EXAMPLE,
                "disarm_alarm": security_examples::DISARM_ALARM_EXAMPLE,
                "get_security_cameras": security_examples::GET_SECURITY_CAMERAS_EXAMPLE
            },
            "weather": {
                "get_weather_data": weather_examples::GET_WEATHER_DATA_EXAMPLE,
                "get_outdoor_conditions": weather_examples::GET_OUTDOOR_CONDITIONS_EXAMPLE,
                "get_weather_forecast_daily": weather_examples::GET_WEATHER_FORECAST_DAILY_EXAMPLE,
                "get_weather_forecast_hourly": weather_examples::GET_WEATHER_FORECAST_HOURLY_EXAMPLE
            },
            "health": {
                "get_health_check": health_examples::HEALTH_CHECK_EXAMPLE,
                "get_health_status": health_examples::HEALTH_STATUS_EXAMPLE
            }
        },
        "best_practices": best_practices::GENERAL_BEST_PRACTICES,
        "error_handling": best_practices::ERROR_HANDLING_GUIDE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_documentation_generation() {
        let docs = generate_tool_documentation();

        // Verify structure
        assert!(docs["tools"].is_object());
        assert!(docs["tools"]["room_management"].is_object());
        assert!(docs["tools"]["device_control"].is_object());
        assert!(docs["best_practices"].is_string());

        // Verify content exists
        let room_docs = docs["tools"]["room_management"]["list_rooms"]
            .as_str()
            .unwrap();
        assert!(room_docs.contains("list_rooms"));
        assert!(room_docs.contains("Purpose:"));
    }

    #[test]
    fn test_all_categories_documented() {
        let docs = generate_tool_documentation();
        let tools = docs["tools"].as_object().unwrap();

        // Ensure all major categories are present
        let expected_categories = vec![
            "room_management",
            "device_control",
            "blind_control",
            "discovery",
            "audio",
            "sensors",
            "climate",
            "energy",
            "security",
            "weather",
            "health",
        ];

        for category in expected_categories {
            assert!(
                tools.contains_key(category),
                "Missing category: {category}"
            );
        }
    }
}
