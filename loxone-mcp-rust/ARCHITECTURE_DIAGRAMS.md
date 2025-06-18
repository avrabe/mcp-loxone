# Architecture Diagrams - Current vs. Proposed

## 🔴 Current Fragmented Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CURRENT PROBLEMATIC STATE                         │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────┐
                              │   DASHBOARD     │
                              │   (Static HTML) │
                              │                 │
                              │ ❌ Complex      │
                              │    Fallback     │
                              │    Logic        │
                              │ ❌ 4 Data       │
                              │    Sources      │
                              └────────┬────────┘
                                       │
          ┌────────────────────────────┴────────────────────────────┐
          │                                                         │
          ▼                                                         ▼
┌─────────────────┐                                       ┌─────────────────┐
│ HTTP TRANSPORT  │                                       │  MCP SERVER     │
│                 │                                       │                 │
│ • Dashboard API │◄─────────────────────────────────────►│ • Resources     │
│ • SSE Events    │         COMPETING ACCESS             │ • Tools         │
│ • Middleware    │                                       │ • Caching       │
└─────────┬───────┘                                       └─────────┬───────┘
          │                                                         │
          │              ┌─────────────────┐                       │
          │              │ CLIENT CONTEXT  │                       │
          └──────────────►│  (SHARED CACHE) │◄──────────────────────┘
                         │                 │
                         │ • devices: Map  │
                         │ • rooms: Map    │
                         │ • capabilities  │
                         │ ❌ STALE DATA   │
                         └─────────┬───────┘
                                   │
                    ┌──────────────┴──────────────┐
                    │                             │
                    ▼                             ▼
          ┌─────────────────┐           ┌─────────────────┐
          │    HISTORY      │           │   MONITORING    │
          │                 │           │                 │
          │ • UnifiedStore  │           │ • InfluxDB      │
          │ • Events        │           │ • Metrics       │
          │ • Tiering       │           │ • Dashboard     │
          │                 │           │                 │
          │ ❌ NOT USED BY  │           │ ❌ PARALLEL     │
          │    DASHBOARD    │           │    SYSTEM       │
          └─────────────────┘           └─────────────────┘
                    │                             │
                    │         ┌─────────────────┐ │
                    │         │  PERFORMANCE    │ │
                    │         │                 │ │
                    │         │ • Metrics       │ │
                    │         │ • Profiler      │ │
                    │         │ • Analyzer      │ │
                    │         │                 │ │
                    │         │ ❌ ISOLATED     │ │
                    │         │    METRICS      │ │
                    │         └─────────────────┘ │
                    │                             │
                    └──────────────┬──────────────┘
                                   │
                         ┌─────────▼───────┐
                         │  MCP TOOLS      │
                         │                 │
                         │ ┌─────┐ ┌─────┐ │
                         │ │Sens │ │Clim │ │
                         │ │ors  │ │ate  │ │
                         │ └─────┘ └─────┘ │
                         │ ┌─────┐ ┌─────┐ │
                         │ │Dev  │ │Enrg │ │
                         │ │ices │ │y    │ │
                         │ └─────┘ └─────┘ │
                         │                 │
                         │ ❌ INDIVIDUAL   │
                         │    API CALLS    │
                         └─────────────────┘
                                   │
                         ┌─────────▼───────┐
                         │ LOXONE API      │
                         │                 │
                         │ ❌ MULTIPLE     │
                         │    ENDPOINTS    │
                         │ ❌ REDUNDANT    │
                         │    CALLS        │
                         └─────────────────┘

PROBLEMS:
• 4 competing data sources for same sensor values
• 200+ lines of complex fallback logic in dashboard
• No integration between history/monitoring/dashboard
• Multiple redundant API calls
• Inconsistent value parsing
• No change detection
• Stale cached data
```

## 🟢 Proposed Unified Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            UNIFIED ARCHITECTURE                             │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────┐
                              │   DASHBOARD     │
                              │  (Real-time)    │
                              │                 │
                              │ ✅ Simple API   │
                              │ ✅ WebSocket    │
                              │ ✅ Historical   │
                              │    Integration  │
                              └────────┬────────┘
                                       │
                                       │ SINGLE DATA SOURCE
                                       │
                              ┌────────▼────────┐
                              │ UNIFIED STATE   │
                              │    MANAGER      │
                              │                 │
                              │ ✅ Single       │
                              │    Source       │
                              │ ✅ Change       │
                              │    Detection    │
                              │ ✅ Real-time    │
                              │    Updates      │
                              └────────┬────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
              ▼                        ▼                        ▼
    ┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
    │ VALUE RESOLVER  │      │ HISTORY STORE   │      │   MONITORING    │
    │                 │      │                 │      │                 │
    │ ✅ Smart        │      │ ✅ Integrated   │      │ ✅ Unified      │
    │    Parsing      │      │    Events       │      │    Metrics      │
    │ ✅ Sensor       │      │ ✅ Real-time    │      │ ✅ Performance  │
    │    Registry     │      │    Storage      │      │    Tracking     │
    │ ✅ Caching      │      │ ✅ Trends       │      │ ✅ Alerting     │
    └─────────┬───────┘      └─────────┬───────┘      └─────────┬───────┘
              │                        │                        │
              │                        │                        │
              └────────────────────────┼────────────────────────┘
                                       │
                              ┌────────▼────────┐
                              │ SENSOR TYPE     │
                              │   REGISTRY      │
                              │                 │
                              │ ✅ All Types    │
                              │    Detected     │
                              │ ✅ Behavioral   │
                              │    Analysis     │
                              │ ✅ Auto Config  │
                              └────────┬────────┘
                                       │
                              ┌────────▼────────┐
                              │ MCP TOOLS       │
                              │   (Simplified)  │
                              │                 │
                              │ ✅ Unified      │
                              │    Interface    │
                              │ ✅ No Redundant │
                              │    Calls        │
                              └────────┬────────┘
                                       │
                              ┌────────▼────────┐
                              │ OPTIMIZED       │
                              │ LOXONE CLIENT   │
                              │                 │
                              │ ✅ Batched      │
                              │    Requests     │
                              │ ✅ Smart        │
                              │    Caching      │
                              │ ✅ WebSocket    │
                              │    Events       │
                              └─────────────────┘

BENEFITS:
• Single source of truth for all sensor data
• Real-time change detection and notifications
• Integrated history and monitoring
• 80% reduction in API calls
• Comprehensive sensor type coverage
• Simplified dashboard logic (200+ lines → ~50 lines)
• Performance monitoring and optimization
```

## 📊 Data Flow Comparison

### 🔴 Current Complex Data Flow

```
DASHBOARD REQUEST:
├── fetch_mcp_sensor_data()           ← REDUNDANT
│   └── get_temperature_sensors()     ← API CALL 1
│       └── JSON parsing              ← COMPLEX
├── get_device_states(all_uuids)      ← API CALL 2-N
│   └── Complex fallback logic:       ← 200+ LINES
│       ├── LL.value extraction       ← STEP 1
│       ├── Direct numeric parsing    ← STEP 2  
│       ├── String value parsing      ← STEP 3
│       ├── UUID reference lookup     ← STEP 4
│       └── Cached state fallback     ← STEP 5
└── Manual room/device grouping       ← COMPLEX

RESULT: Inconsistent data, high latency, complex debugging
```

### 🟢 Proposed Simple Data Flow

```
DASHBOARD REQUEST:
└── state_manager.get_dashboard_data()
    ├── Cached current states         ← FAST
    ├── Historical trends (if needed) ← INTEGRATED
    └── Real-time updates via WS      ← EFFICIENT

BACKGROUND (AUTOMATED):
└── state_manager.refresh_cycle()
    ├── Batched API calls             ← OPTIMIZED
    ├── Smart value resolution        ← CONSISTENT
    ├── Change detection              ← AUTOMATED
    ├── History recording             ← INTEGRATED
    └── WebSocket notifications       ← REAL-TIME

RESULT: Consistent data, low latency, easy debugging
```

## 🧪 Sensor Analysis - Current vs. Proposed

### 🔴 Current Sensor Handling

```
TEMPERATURE SENSORS:
┌─────────────────────────────────────────────────────────────────┐
│ Current State: Partially Handled                               │
│ ✅ Detection: device_name.contains("temperatur")              │
│ ✅ Parsing: extract_numeric_value("27.0°") → 27.0            │
│ ❌ History: Not stored in unified history                     │
│ ❌ Monitoring: Not tracked in performance metrics             │
│ ❌ Real-time: No change notifications                         │
└─────────────────────────────────────────────────────────────────┘

HUMIDITY SENSORS:
┌─────────────────────────────────────────────────────────────────┐
│ Current State: Partially Handled                               │
│ ✅ Detection: device_name.contains("luftfeuchte")             │
│ ✅ Parsing: extract_numeric_value("58%") → 58.0               │
│ ❌ History: Not stored                                         │
│ ❌ Monitoring: Not tracked                                     │
│ ❌ Range validation: No min/max checking                      │
└─────────────────────────────────────────────────────────────────┘

UNKNOWN SENSORS:
┌─────────────────────────────────────────────────────────────────┐
│ Current State: Not Handled                                     │
│ ❌ Motion sensors: Not detected                                │
│ ❌ Door/window contacts: Not detected                          │
│ ❌ Energy meters: Not properly categorized                     │
│ ❌ Air quality: Not recognized                                 │
│ ❌ Pressure sensors: Not handled                               │
└─────────────────────────────────────────────────────────────────┘
```

### 🟢 Proposed Comprehensive Sensor Handling

```
ALL SENSOR TYPES:
┌─────────────────────────────────────────────────────────────────┐
│ ✅ ENVIRONMENTAL SENSORS                                       │
│    • Temperature (°C, °F, K)                                 │
│    • Humidity (%, absolute)                                   │  
│    • Air Pressure (hPa, mmHg, PSI)                           │
│    • Air Quality (AQI, PM2.5, CO2 ppm)                       │
│                                                                │
│ ✅ LIGHT SENSORS                                              │
│    • Illuminance (Lx, fc)                                    │
│    • UV Index (0-11 scale)                                   │
│                                                                │
│ ✅ MOTION & PRESENCE                                          │
│    • PIR Motion Detectors (binary)                           │
│    • Presence Sensors (occupancy %)                          │
│                                                                │
│ ✅ CONTACT & POSITION                                         │
│    • Door/Window Contacts (open/closed)                      │
│    • Window Position (0-100%)                                │
│    • Blind Position (0-100%)                                 │
│                                                                │
│ ✅ ENERGY MONITORING                                          │
│    • Power Meters (W, kW)                                    │
│    • Energy Consumption (Wh, kWh)                            │
│    • Current (A, mA)                                         │
│    • Voltage (V, mV)                                         │
│                                                                │
│ ✅ WEATHER SENSORS                                            │
│    • Wind Speed (m/s, mph, km/h)                             │
│    • Rainfall (mm, inches)                                   │
│                                                                │
│ ✅ AUDIO SENSORS                                              │
│    • Sound Level (dB, dBA)                                   │
│                                                                │
│ ✅ INTELLIGENT DISCOVERY                                      │
│    • Behavioral analysis for unknown devices                 │
│    • Pattern recognition for sensor classification           │
│    • Confidence scoring for detected types                   │
│    • Learning mode for new sensor types                      │
└─────────────────────────────────────────────────────────────────┘

ENHANCED FEATURES:
┌─────────────────────────────────────────────────────────────────┐
│ ✅ REAL-TIME MONITORING                                        │
│    • Change detection with configurable thresholds           │
│    • WebSocket notifications for dashboard                   │
│    • Alert generation for abnormal values                    │
│                                                                │
│ ✅ HISTORICAL INTEGRATION                                     │
│    • All sensor changes stored in unified history            │
│    • Trend analysis and pattern detection                    │
│    • Historical charts and reporting                         │
│                                                                │
│ ✅ VALIDATION & QUALITY                                       │
│    • Range validation for all sensor types                   │
│    • Outlier detection and filtering                         │
│    • Data quality scoring                                    │
│                                                                │
│ ✅ PERFORMANCE OPTIMIZATION                                   │
│    • Batched sensor reading requests                         │
│    • Intelligent caching with TTL                            │
│    • Performance metrics for sensor operations               │
└─────────────────────────────────────────────────────────────────┘
```

## 🔄 Migration Strategy

### Phase 1: Foundation (Weeks 1-2)
```
OLD: Multiple parsing approaches
NEW: UnifiedValueResolver + SensorTypeRegistry

├── Create value resolution service
├── Implement comprehensive sensor detection
├── Build smart parsing for all sensor types
└── Add behavioral analysis for unknown sensors
```

### Phase 2: State Management (Weeks 3-4)
```
OLD: ClientContext with stale cache
NEW: UnifiedStateManager with change detection

├── Replace ClientContext caching
├── Add real-time change detection
├── Integrate with history store
└── Implement state change listeners
```

### Phase 3: Dashboard Integration (Weeks 5-6)
```
OLD: Complex 200+ line fallback logic
NEW: Simple state manager integration

├── Refactor dashboard_data.rs (80% reduction)
├── Add WebSocket real-time updates
├── Remove redundant API calls
└── Add historical data integration
```

### Phase 4: Complete Coverage (Weeks 7-8)
```
OLD: Only temperature/humidity/light sensors
NEW: All sensor types discovered and handled

├── Comprehensive sensor discovery
├── Unknown device behavioral analysis
├── Real environment validation
└── Performance optimization
```

This architecture transformation will solve the current fragmentation issues and provide a robust, maintainable foundation for comprehensive sensor monitoring.