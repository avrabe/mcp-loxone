# MCP Integration Examples

## Real-World Usage Scenarios

### 1. Daily Status Check (Automatic)
```
User: "Are all my doors and windows closed?"

Claude automatically:
1. Calls check_doors_windows_status()
2. Discovers sensors dynamically 
3. Reports: "I found 3 door/window sensors. 2 are closed, but 1 is currently open. 
   Would you like me to identify which door is open?"

User: "Yes, which door is open?"

Claude:
1. Calls monitor_sensor_live() on the open sensor
2. "Please close the door that's currently open so I can identify it"
3. Detects state change and identifies the specific door
```

### 2. Home Security Check
```
User: "Check all entry points before I go to bed"

Claude:
1. Calls discover_runtime_sensors() if not already done
2. Calls check_doors_windows_status() 
3. Reports detailed status of all doors/windows
4. "All entry points are secure. 12 sensors checked, all closed."
```

### 3. Setup/Configuration (One-time)
```
User: "Help me map all my door and window sensors"

Claude:
1. Calls discover_runtime_sensors()
2. Calls identify_unknown_sensors()
3. Guides user through opening/closing each door
4. Creates a complete mapping
5. Saves the mapping for future use
```

### 4. Troubleshooting
```
User: "My home automation says no doors are open but I think one is"

Claude:
1. Calls check_doors_windows_status()
2. Compares WindowMonitor vs runtime sensors
3. "Your WindowMonitor shows 0 open, but I found 1 sensor that's actually open.
   This confirms the WindowMonitor is using broken sensors."
```

## Integration with Claude Desktop

### claude_desktop_config.json
```json
{
  "mcpServers": {
    "loxone": {
      "command": "uvx",
      "args": ["--from", ".", "loxone-mcp-server"],
      "cwd": "/path/to/mcp-loxone-gen1"
    }
  }
}
```

### Typical Conversation Flow
```
User: "What's the status of my house?"

Claude: I'll check your Loxone system for you.

[Automatically calls check_doors_windows_status()]

Based on your Loxone system:
- **WindowMonitor**: Shows 14 sensors total, 0 currently open
- **Runtime Sensors**: Found 3 working sensors, 1 is actually OPEN
- **Discrepancy Found**: Your WindowMonitor isn't working correctly

The working sensors show:
- Sensor 855703: CLOSED (Terrassen Tür R.)  
- Sensor 85BEB3: OPEN ⚠️ (needs identification)
- Sensor 86F4B4: CLOSED (Arbeitszimmer window)

Would you like me to help identify which door sensor 85BEB3 corresponds to?
```

## Advanced Integration Scenarios

### 1. Proactive Monitoring
```python
# The MCP server could be enhanced to provide ongoing monitoring
@mcp.tool()
async def setup_door_monitoring() -> dict[str, Any]:
    """Set up continuous monitoring of discovered sensors."""
    # Discover all sensors once
    sensors = await discover_runtime_sensors()
    
    # Store sensor list for ongoing monitoring
    # Return monitoring configuration
    return {
        "monitoring_active": True,
        "sensors_monitored": len(sensors["all_sensors"]),
        "check_interval": "30 seconds"
    }
```

### 2. Smart Home Integration
```
User: "Create a bedtime routine"

Claude:
1. Calls check_doors_windows_status()
2. If any doors/windows open: "I found 2 windows open. Should I include closing them in your bedtime routine?"
3. Integrates door/window status with other smart home controls
```

### 3. Context-Aware Responses
```
User: "Turn off all lights"

Claude:
1. Calls check_doors_windows_status() 
2. "I'll turn off the lights. I notice the kitchen window is open - would you like me to remind you to close it for security?"
```

## Error Handling & Fallbacks

### When Sensors Aren't Found
```
User: "Check my doors"

Claude:
[Calls discover_runtime_sensors() - finds 0 sensors]

"I couldn't find any working door/window sensors using the standard runtime UUID pattern. This could mean:
1. Your system uses a different sensor pattern
2. The sensors need to be configured differently  
3. You might need to set up the sensor mapping first

Would you like me to:
- Try different search patterns?
- Help you identify your sensor configuration?
- Check your WindowMonitor setup?"
```

### When Discovery Fails
```python
@mcp.tool()
async def fallback_sensor_check() -> dict[str, Any]:
    """Fallback method when dynamic discovery fails."""
    # Try WindowMonitor only
    # Try structure-based discovery
    # Provide manual configuration guidance
```

## Performance Considerations

### Caching Discovered Sensors
```python
# Cache sensors to avoid rediscovery on every call
_sensor_cache = {}

async def get_cached_sensors():
    if not _sensor_cache or cache_expired():
        _sensor_cache = await discover_runtime_sensors()
    return _sensor_cache
```

### Background Discovery
```python
# Run discovery in background when server starts
async def background_sensor_discovery():
    """Discover sensors once at startup."""
    global _known_sensors
    try:
        result = await discover_runtime_sensors(max_search_time=60)
        _known_sensors = result["all_sensors"]
        logger.info(f"Discovered {len(_known_sensors)} sensors at startup")
    except Exception as e:
        logger.warning(f"Background discovery failed: {e}")
```

## User Experience Flow

### First-Time Setup
1. User installs MCP server
2. Claude automatically discovers sensors on first door/window query
3. Claude asks user to help identify unknown sensors
4. Mapping is saved for future use

### Daily Usage  
1. User asks about doors/windows
2. Claude uses cached sensor mapping
3. Provides instant status without re-discovery
4. Updates mapping if new sensors found

### Maintenance
1. Claude detects when sensors stop working
2. Suggests re-running discovery
3. Helps troubleshoot sensor issues

This creates a seamless experience where the user doesn't need to know about UUIDs or technical details - they just ask about their doors and windows, and Claude handles all the complexity behind the scenes!