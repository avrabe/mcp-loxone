# Loxone MCP Automation Scenarios

This document provides real-world automation scenarios that can be implemented using the n8n workflows and Loxone MCP integration.

## 1. Wake-Up Routine

**Trigger**: Time-based (6:30 AM on weekdays, 8:00 AM on weekends)

**Actions**:
1. Gradually increase bedroom lights (0% → 30% over 5 minutes)
2. Open bedroom blinds to 50%
3. Set bathroom temperature to 22°C
4. Start coffee machine (if integrated)
5. Play morning news/music in kitchen
6. Display weather forecast on info screen

**Implementation**:
```javascript
// Morning routine logic
const isWeekday = [1,2,3,4,5].includes(new Date().getDay());
const wakeTime = isWeekday ? "06:30" : "08:00";

const routine = [
  { delay: 0, action: "fade_lights", params: { room: "Bedroom", from: 0, to: 30, duration: 300 } },
  { delay: 60, action: "set_blinds", params: { room: "Bedroom", position: 50 } },
  { delay: 0, action: "set_temperature", params: { room: "Bathroom", temp: 22 } },
  { delay: 120, action: "control_appliance", params: { device: "Coffee_Machine", state: "on" } },
  { delay: 180, action: "play_media", params: { room: "Kitchen", source: "morning_playlist" } }
];
```

## 2. Leaving Home Automation

**Trigger**: 
- Manual: "Goodbye" button press
- Automatic: No motion for 30 minutes + door lock engaged

**Actions**:
1. Turn off all lights except security lights
2. Set temperature to eco mode (18°C winter, 26°C summer)
3. Close all blinds
4. Arm security system (with 60-second delay)
5. Turn off non-essential appliances
6. Send confirmation to mobile app
7. Enable random light simulation (vacation mode)

**Energy Savings**: Estimated 15-20% reduction in daily energy consumption

## 3. Intrusion Detection Response

**Trigger**: Motion detected while security armed

**Actions**:
1. **Immediate** (0-5 seconds):
   - Record all cameras
   - Turn on all exterior lights
   - Flash interior lights
   - Sound alarm

2. **Notification** (5-10 seconds):
   - Send push notification with camera snapshot
   - Call primary contact
   - Send SMS to emergency contacts
   - Log event with timestamp and location

3. **Escalation** (30 seconds if not disarmed):
   - Contact security service
   - Increase alarm volume
   - Activate smoke machines (if installed)

## 4. Energy Peak Management

**Trigger**: Energy usage exceeds 80% of limit during peak hours (4-8 PM)

**Progressive Actions**:
1. **Stage 1** (80% threshold):
   - Reduce AC/heating by 2°C
   - Dim non-essential lights by 50%
   - Notify users via app

2. **Stage 2** (90% threshold):
   - Turn off electric water heater
   - Disable pool pump
   - Close motorized blinds to reduce cooling load
   
3. **Stage 3** (95% threshold):
   - Emergency load shedding
   - Keep only essential circuits active
   - Send critical alert

**Recovery**: Gradual restoration over 30 minutes after peak

## 5. Weather-Responsive Automation

**Trigger**: Weather API data (checked every 15 minutes)

**Scenarios**:

### Strong Wind (>50 km/h)
- Retract awnings
- Close skylights
- Secure outdoor furniture notification

### Rain Detected
- Close all windows
- Retract awnings
- Turn on entrance lights
- Increase indoor lighting

### High UV Index
- Close south-facing blinds to 70%
- Suggest sunscreen reminder
- Adjust pool chemistry notification

### Temperature Extremes
- **Hot (>30°C)**: Close blinds, pre-cool before peak hours
- **Cold (<5°C)**: Check heating, send frost warning

## 6. Adaptive Lighting

**Continuous Adjustment Based On**:
- Time of day
- Natural light levels
- Room occupancy
- Activity type

**Example Profiles**:
```javascript
const lightingProfiles = {
  work: { intensity: 100, temperature: 5000 }, // Cool white, bright
  relax: { intensity: 40, temperature: 2700 }, // Warm white, dim
  dinner: { intensity: 60, temperature: 3000 }, // Neutral, moderate
  movie: { intensity: 10, temperature: 2200 }, // Very warm, very dim
  sleep: { intensity: 0, temperature: 2000 }    // Off or nightlight
};
```

## 7. Guest Mode

**Trigger**: Guest code entered or guest button pressed

**Actions**:
1. Disable bedroom motion sensors
2. Set comfortable temperature in guest room
3. Enable guest WiFi
4. Provide limited lighting control
5. Send house manual to guest's phone
6. Schedule auto-disable after checkout

## 8. Health & Wellness

### Air Quality Management
**Trigger**: CO2 > 1000ppm or VOC alert

**Actions**:
- Increase ventilation
- Open windows (if outdoor air quality is good)
- Alert occupants
- Turn on air purifiers

### Circadian Rhythm Support
**Continuous**:
- Morning: Bright, blue-enriched light
- Evening: Warm, dim light
- Night: Red nightlight only

### Exercise Mode
**Trigger**: "Workout" scene

**Actions**:
- Energizing music
- Bright lights
- Cooler temperature
- Start workout timer
- Disable doorbell

## 9. Maintenance Reminders

**Scheduled Checks**:
- Filter replacement (every 3 months)
- Battery status (monthly)
- System diagnostics (weekly)
- Water leak detection (continuous)

**Smart Notifications**:
```javascript
if (filterRuntime > 2000) {
  notify("Air filter replacement recommended");
} else if (filterRuntime > 2500) {
  notify("Air filter replacement required", "high");
  reduceHVACPower(20); // Protect system
}
```

## 10. Emergency Scenarios

### Fire Detection
1. Sound all alarms
2. Turn on all lights
3. Unlock all doors
4. Open garage doors
5. Turn off HVAC
6. Call emergency services
7. Send evacuation routes to phones

### Medical Emergency
1. Turn on all path lighting
2. Unlock front door
3. Flash porch light
4. Send GPS location to emergency contacts
5. Display medical info on entry screen

### Power Outage
1. Switch to UPS/battery backup
2. Reduce load to essentials
3. Send status notification
4. Monitor restoration
5. Gradual system restart

## Integration Examples

### Calendar Integration
```javascript
// Check calendar for events
if (calendar.hasEvent("Party", today)) {
  automations.schedule("party_mode", eventStart);
  shopping.remind("Party supplies", dayBefore);
}
```

### Voice Commands
- "Alexa, good night" → Activate sleep scene
- "Hey Google, I'm cold" → Increase temperature 2°C
- "Siri, movie time" → Dim lights, close blinds, turn on TV

### Geofencing
- Approaching home: Pre-heat/cool, turn on pathway lights
- Leaving area: Suggest forgotten items, confirm security

## Best Practices

1. **Fail-Safe Defaults**: Always have manual overrides
2. **Gradual Changes**: Avoid jarring transitions
3. **User Preferences**: Learn and adapt to habits
4. **Energy Efficiency**: Balance comfort with consumption
5. **Privacy**: Local processing when possible
6. **Redundancy**: Multiple triggers for critical automations

## Customization Tips

- Start simple, add complexity gradually
- Test automations thoroughly
- Get family buy-in before implementing
- Document all automations for troubleshooting
- Regular reviews and adjustments
- Seasonal adjustment schedules