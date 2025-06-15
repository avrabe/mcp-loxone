# Unified Dashboard Architecture

## Problem Statement

The current dashboard implementation has several issues:
- **Empty Widgets**: Room Temperatures, Device Activity, System Health, and Device Runtime widgets show no data
- **Fragmented Architecture**: Separate `/dashboard/` (InfluxDB) and `/history/` (UnifiedHistoryStore) endpoints create confusion
- **Missing Operational Metrics**: No visibility into rate limiter hits, API performance, or security events
- **Poor Data Flow**: Disconnected data collection between real-time monitoring and historical storage

## Proposed Solution: Single Unified Dashboard

### Core Principles

1. **Single Data Pipeline**: One unified data collection system feeding both real-time and historical views
2. **Single Dashboard Endpoint**: Consolidate all monitoring at `/dashboard/`
3. **Operational Visibility**: Include system health metrics that matter for operations
4. **Real-time Updates**: WebSocket-based live updates for all widgets
5. **Progressive Enhancement**: Works without JavaScript, enhanced with real-time updates

### Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Loxone Data   │────│ Data Collector  │────│ Unified Storage │
│    Sources      │    │   Pipeline      │    │    System       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │                        │
                                ▼                        ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ Real-time       │    │    Dashboard    │    │   Historical    │
│ WebSocket       │◄───│   Controller    │────│    Analysis     │
│ Updates         │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Unified Dashboard UI                         │
├─────────────────┬─────────────────┬─────────────────┬───────────┤
│ Real-time       │ Device & Room   │ Operational     │ Historical│
│ Monitoring      │ Overview        │ Metrics         │ Analysis  │
└─────────────────┴─────────────────┴─────────────────┴───────────┘
```

### Dashboard Layout

#### Section 1: Real-time Monitoring (Top Row)
- **System Health**: Connection status, last update, error rate
- **Active Sensors**: Live sensor readings with visual indicators
- **Recent Activity**: Last 10 device state changes in real-time

#### Section 2: Device & Room Overview (Second Row)
- **Room Temperature Grid**: 6 climate controllers with current temp/setpoint
- **Device Status Matrix**: All devices grouped by room with status indicators
- **Quick Controls**: Most-used device controls (lights, blinds)

#### Section 3: Operational Metrics (Third Row)
- **API Performance**: Request rate, response times, error rates
- **Rate Limiter Status**: Current limits, recent hits, blocked requests
- **Security Events**: Failed auth attempts, suspicious activity
- **Resource Utilization**: Memory, CPU, connection counts

#### Section 4: Historical Analysis (Bottom Row)
- **Temperature Trends**: 24h temperature graphs for each room
- **Device Usage Patterns**: Most active devices and usage times
- **System Performance History**: API performance over time
- **Audit Trail**: Recent configuration changes and user actions

### Data Collection Pipeline

#### Unified Data Collector
```rust
pub struct UnifiedDataCollector {
    // Loxone client connections
    clients: HashMap<String, Arc<dyn LoxoneClient>>,
    
    // Storage systems
    hot_storage: Arc<RwLock<HotDataStore>>,
    cold_storage: Arc<ColdDataStore>,
    
    // Real-time distribution
    websocket_manager: Arc<WebSocketManager>,
    
    // Metrics collection
    operational_metrics: Arc<OperationalMetricsCollector>,
}
```

#### Data Flow
1. **Collection**: Single service polls Loxone every 5 seconds
2. **Processing**: Normalize data into unified event format
3. **Distribution**: 
   - Hot storage for real-time dashboard
   - Cold storage for historical analysis
   - WebSocket broadcast for live updates
4. **Aggregation**: Pre-compute common metrics for dashboard performance

### Implementation Plan

#### Phase 1: Core Infrastructure
1. Create `UnifiedDataCollector` service
2. Implement unified data pipeline
3. Create WebSocket endpoint for real-time updates
4. Consolidate storage access layer

#### Phase 2: Dashboard Implementation
1. Create single dashboard HTML template
2. Implement JavaScript for real-time updates
3. Add operational metrics collection
4. Create responsive CSS for all screen sizes

#### Phase 3: Advanced Features
1. Add historical analysis widgets
2. Implement user preferences
3. Add dashboard configuration
4. Performance optimization

### API Endpoints

#### Primary Dashboard
- `GET /dashboard/` - Main dashboard HTML page
- `GET /dashboard/api/status` - Current system status (JSON)
- `GET /dashboard/api/data` - All dashboard data (JSON)
- `WS /dashboard/ws` - Real-time updates WebSocket

#### Data APIs
- `GET /dashboard/api/rooms` - Room status and devices
- `GET /dashboard/api/metrics` - Operational metrics
- `GET /dashboard/api/history/{type}` - Historical data by type
- `GET /dashboard/api/trends/{period}` - Trend analysis

### Operational Metrics Collection

#### Rate Limiter Metrics
- Current request counts per client
- Rate limit hits and rejections
- Peak usage times
- Client behavior patterns

#### API Performance Metrics
- Response time percentiles (P50, P95, P99)
- Error rates by endpoint
- Request volume trends
- Slow query identification

#### Security Metrics
- Authentication failures
- Suspicious request patterns
- IP-based activity monitoring
- Security event timeline

#### Resource Metrics
- WebSocket connection counts
- Memory usage trends
- CPU utilization
- Disk storage usage

### Benefits

1. **Operational Visibility**: Clear view of system health and performance
2. **Unified Experience**: Single place for all monitoring needs
3. **Real-time Feedback**: Immediate updates when devices change
4. **Historical Context**: Understanding long-term patterns and trends
5. **Troubleshooting**: Easy identification of issues and their timeline
6. **Performance Monitoring**: Proactive identification of bottlenecks

### Migration Strategy

1. **Phase 1**: Keep existing endpoints, add new unified dashboard
2. **Phase 2**: Migrate users to new dashboard, mark old endpoints deprecated
3. **Phase 3**: Remove deprecated endpoints after user adoption

This architecture addresses all the current issues while providing a foundation for future dashboard enhancements.