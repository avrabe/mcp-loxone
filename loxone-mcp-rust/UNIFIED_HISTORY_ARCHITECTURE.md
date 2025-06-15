# Unified Historical Data Architecture

## Overview
This document proposes a unified architecture for historical data storage in the Loxone MCP server, consolidating multiple implementations into a coherent system.

## Current State Issues
- Multiple separate implementations (sensor history, stats collector, response cache, etc.)
- Inconsistent retention policies
- No unified visibility in dashboard
- Duplicate functionality across modules
- Mix of in-memory and file storage without clear strategy

## Proposed Architecture

### Core Components

#### 1. Unified Time Series Store (`src/history/mod.rs`)
Central service managing all historical data with consistent APIs.

```rust
pub struct UnifiedHistoryStore {
    // In-memory hot storage (recent data)
    hot_store: Arc<RwLock<HotDataStore>>,
    
    // Persistent cold storage (historical data)
    cold_store: Arc<ColdDataStore>,
    
    // Configuration
    config: HistoryConfig,
    
    // Metrics integration
    metrics: Arc<MetricsCollector>,
}

pub struct HotDataStore {
    // Recent data in ring buffers
    device_states: HashMap<String, RingBuffer<DeviceStateEvent>>,
    sensor_readings: HashMap<String, RingBuffer<SensorReading>>,
    system_metrics: RingBuffer<SystemMetric>,
    
    // Aggregated statistics (1min, 5min, 15min)
    aggregates: HashMap<String, AggregateStats>,
}

pub struct ColdDataStore {
    // File-based storage with compression
    data_dir: PathBuf,
    index: Arc<RwLock<DataIndex>>,
    compression: CompressionStrategy,
}
```

#### 2. Data Categories & Storage Strategy

| Data Type | Hot Storage (In-Memory) | Cold Storage (Persistent) | Retention |
|-----------|------------------------|--------------------------|-----------|
| **Device States** | Last 100 changes | Daily summaries | 30 days |
| **Sensor Readings** | Last hour (1-sec resolution) | Hourly averages | 90 days |
| **System Metrics** | Last 15 minutes | 5-min aggregates | 7 days |
| **Response Cache** | TTL-based (5-60 min) | None | In-memory only |
| **Audit Events** | Last 1000 events | All events | 1 year |
| **Discovery Cache** | All active devices | JSON snapshot | Until invalidated |

#### 3. Unified Data Model

```rust
// Common event structure
pub struct HistoricalEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub category: EventCategory,
    pub source: EventSource,
    pub data: EventData,
    pub metadata: HashMap<String, Value>,
}

pub enum EventCategory {
    DeviceState(DeviceStateChange),
    SensorReading(SensorData),
    SystemMetric(MetricData),
    AuditEvent(AuditData),
    DiscoveryEvent(DiscoveryData),
}

pub struct DeviceStateChange {
    pub device_uuid: String,
    pub device_name: String,
    pub room: Option<String>,
    pub previous_state: Value,
    pub new_state: Value,
    pub triggered_by: String,
}
```

#### 4. Query Interface

```rust
pub trait HistoryQuery {
    // Time-based queries
    async fn query_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: QueryFilters,
    ) -> Result<Vec<HistoricalEvent>>;
    
    // Aggregated data
    async fn query_aggregates(
        &self,
        metric: &str,
        interval: AggregateInterval,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AggregatePoint>>;
    
    // Latest values
    async fn get_latest(
        &self,
        category: EventCategory,
        count: usize,
    ) -> Result<Vec<HistoricalEvent>>;
}
```

#### 5. Dashboard Integration

```rust
// Dashboard-specific views
pub struct DashboardHistoryProvider {
    store: Arc<UnifiedHistoryStore>,
}

impl DashboardHistoryProvider {
    // Real-time data stream
    pub fn subscribe_realtime(&self) -> broadcast::Receiver<HistoricalEvent>;
    
    // Pre-computed dashboard data
    pub async fn get_dashboard_data(&self) -> DashboardHistoryData {
        DashboardHistoryData {
            device_activity: self.get_device_activity_chart().await,
            sensor_trends: self.get_sensor_trend_charts().await,
            system_health: self.get_system_health_timeline().await,
            energy_usage: self.get_energy_usage_graph().await,
            recent_events: self.get_recent_events_feed().await,
        }
    }
}
```

### Implementation Plan

#### Phase 1: Core Infrastructure
1. Create unified history module structure
2. Implement HotDataStore with ring buffers
3. Implement ColdDataStore with file persistence
4. Create migration utilities for existing data

#### Phase 2: Data Migration
1. Migrate sensor state history
2. Migrate device state trackers
3. Consolidate response cache
4. Unify audit logging

#### Phase 3: Dashboard Integration
1. Create dashboard history provider
2. Add real-time WebSocket streaming
3. Implement chart data endpoints
4. Add historical data widgets

#### Phase 4: Advanced Features
1. Data compression for cold storage
2. Automatic tiering (hot → cold)
3. Export capabilities (CSV, JSON)
4. Backup and restore

### Benefits

1. **Unified API**: Single interface for all historical data
2. **Consistent Retention**: Clear policies across all data types
3. **Performance**: Optimized hot/cold storage tiers
4. **Dashboard Visibility**: All data accessible in UI
5. **Maintenance**: Single system to maintain
6. **Scalability**: Easy to add new data types

### Migration Strategy

```rust
// Gradual migration with compatibility layer
pub struct LegacyCompatibilityLayer {
    // Adapters for existing code
    sensor_history_adapter: SensorHistoryAdapter,
    stats_collector_adapter: StatsCollectorAdapter,
    response_cache_adapter: ResponseCacheAdapter,
}

// Example adapter
impl SensorHistoryAdapter {
    pub fn log_state_change(&self, event: StateChangeEvent) {
        // Convert to unified format
        let historical_event = HistoricalEvent::from_sensor_event(event);
        self.unified_store.record(historical_event).await;
    }
}
```

### Configuration

```toml
[history]
# Hot storage limits
hot_storage_device_events = 100
hot_storage_sensor_minutes = 60
hot_storage_metrics_minutes = 15

# Cold storage settings
cold_storage_dir = "/var/lib/loxone-mcp/history"
cold_storage_compression = "zstd"
cold_storage_max_size_gb = 10

# Retention policies
retention_device_states_days = 30
retention_sensor_data_days = 90
retention_system_metrics_days = 7
retention_audit_events_days = 365

# Performance tuning
write_buffer_size = 1000
flush_interval_seconds = 10
query_cache_size_mb = 100
```

## Summary

This unified architecture provides:
- **Single source of truth** for all historical data
- **Clear separation** between hot (in-memory) and cold (persistent) storage
- **Consistent APIs** for querying and streaming
- **Dashboard integration** out of the box
- **Scalable design** that can grow with requirements

The phased implementation allows gradual migration while maintaining backward compatibility.