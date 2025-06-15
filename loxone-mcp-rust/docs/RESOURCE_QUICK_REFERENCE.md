# Loxone MCP Resources - Quick Reference

## Resource URI Patterns

```
loxone://rooms                                    # All rooms
loxone://rooms/{roomName}/devices                 # Devices in a room

loxone://devices/all                              # All devices
loxone://devices/type/{deviceType}                # Devices by type
loxone://devices/category/{category}              # Devices by category

loxone://system/status                            # System health
loxone://system/capabilities                      # Available features
loxone://system/categories                        # Category overview

loxone://audio/zones                              # Audio zones
loxone://audio/sources                            # Audio sources

loxone://sensors/door-window                      # Door/window sensors
loxone://sensors/temperature                      # Temperature sensors
loxone://sensors/discovered                       # Discovered sensors
```

## Common Query Parameters

```
?limit=20                    # Limit results
?offset=40                   # Skip first 40 results
?sort=name                   # Sort by name ascending
?sort=-name                  # Sort by name descending
?room=Kitchen                # Filter by room
?type=Switch                 # Filter by device type
?category=lighting           # Filter by category
```

## Examples

```bash
# Get all lights
loxone://devices/category/lighting

# Get kitchen devices
loxone://rooms/Kitchen/devices

# Get all switches sorted by name
loxone://devices/type/Switch?sort=name

# Get first 10 temperature sensors
loxone://sensors/temperature?limit=10

# Get blinds in living room
loxone://rooms/Living%20Room/devices?type=Jalousie
```

## Categories

- `lighting` - Lights, dimmers, switches
- `blinds` - Rolladen, jalousies, shades  
- `climate` - Thermostats, room controllers
- `sensors` - All sensor types
- `audio` - Audio zones and controls

## Device Types

Common types include:
- `Switch`
- `Dimmer`
- `LightController`
- `Jalousie`
- `IRoomControllerV2`
- `AnalogInput`
- `DigitalInput`
- `AudioZone`

## Cache TTLs

- Room/device lists: 10 minutes
- System info: 5 minutes
- Sensor data: 30 seconds
- Audio status: 10 seconds