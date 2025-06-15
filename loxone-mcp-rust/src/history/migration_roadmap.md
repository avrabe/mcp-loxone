# Migration Roadmap: Unified History Architecture

## Current State → Target State

### Phase 1: Foundation (Week 1-2)

#### 1.1 Create Core Structure
```rust
// src/history/mod.rs
pub mod core;
pub mod hot_storage;
pub mod cold_storage;
pub mod query;
pub mod dashboard;
pub mod migration;

// Keep existing modules working
pub mod compat {
    pub mod sensor_history;
    pub mod stats_collector;
    pub mod response_cache;
}
```

#### 1.2 Implement Base Components
- [ ] Create `UnifiedHistoryStore` trait
- [ ] Implement `HotDataStore` with ring buffers
- [ ] Implement `ColdDataStore` with JSON persistence
- [ ] Create event type definitions

### Phase 2: Adapter Layer (Week 3)

#### 2.1 Create Compatibility Adapters
```rust
// Adapter example for sensor history
impl From<StateChangeEvent> for HistoricalEvent {
    fn from(event: StateChangeEvent) -> Self {
        HistoricalEvent {
            id: Uuid::new_v4(),
            timestamp: event.timestamp,
            category: EventCategory::SensorReading(SensorData {
                sensor_uuid: event.sensor_uuid,
                value: event.new_state,
                sensor_type: event.sensor_type,
            }),
            source: EventSource::Sensor(event.sensor_uuid),
            data: EventData::SensorChange(event),
            metadata: HashMap::new(),
        }
    }
}
```

#### 2.2 Dual-Write Strategy
- Existing code continues to work unchanged
- Adapters write to both old and new storage
- Allows rollback if needed

### Phase 3: Migration (Week 4-5)

#### 3.1 Data Migration Tools
```bash
# Migration CLI tool
cargo run --bin history-migrate -- \
    --source sensor_history \
    --target unified_store \
    --start-date 2024-01-01 \
    --batch-size 1000
```

#### 3.2 Module-by-Module Migration
| Module | Current Location | Migration Steps |
|--------|-----------------|-----------------|
| Sensor History | `tools/sensors.rs` | 1. Add adapter<br>2. Dual-write<br>3. Migrate data<br>4. Switch reads |
| Device Trackers | `monitoring/loxone_stats.rs` | 1. Extract trackers<br>2. Convert to events<br>3. Update collectors |
| Response Cache | `server/response_cache.rs` | 1. Keep as-is (ephemeral)<br>2. Add metrics export |
| Discovery Cache | `discovery/discovery_cache.rs` | 1. Convert to events<br>2. Add query interface |
| Audit Log | `audit_log.rs` | 1. Direct integration<br>2. Keep file backup |

### Phase 4: Dashboard Integration (Week 6)

#### 4.1 Dashboard Data Provider
```rust
// New dashboard endpoints
GET /api/history/devices/{uuid}/timeline
GET /api/history/sensors/{uuid}/chart
GET /api/history/system/metrics
GET /api/history/events/stream (WebSocket)
```

#### 4.2 UI Components
- Timeline widget for device states
- Chart component for sensor trends
- Activity feed for recent events
- System health visualization

### Phase 5: Optimization (Week 7-8)

#### 5.1 Performance Tuning
- Add query result caching
- Implement data compression
- Optimize ring buffer sizes
- Add indexes for common queries

#### 5.2 Monitoring
- Add metrics for history store performance
- Set up alerts for storage issues
- Create capacity planning dashboard

## Migration Checklist

### Pre-Migration
- [ ] Backup existing data
- [ ] Document current data formats
- [ ] Create rollback plan
- [ ] Set up monitoring

### During Migration
- [ ] Run dual-write mode for 1 week
- [ ] Verify data integrity
- [ ] Compare old vs new queries
- [ ] Monitor performance impact

### Post-Migration
- [ ] Remove compatibility adapters
- [ ] Clean up old storage
- [ ] Update documentation
- [ ] Train team on new system

## Risk Mitigation

1. **Data Loss**: 
   - Dual-write during migration
   - Keep backups for 30 days
   - Verify counts match

2. **Performance Impact**:
   - Migrate during low-usage hours
   - Use batch processing
   - Monitor resource usage

3. **API Compatibility**:
   - Keep existing APIs working
   - Add deprecation warnings
   - Provide migration guide

## Success Criteria

- [ ] All historical data migrated successfully
- [ ] No increase in query latency
- [ ] Dashboard shows all data types
- [ ] Reduced storage footprint by 30%
- [ ] Single codebase for history management

## Code Removal Schedule

After successful migration and 2-week stability period:

1. Week 9: Remove sensor history implementation
2. Week 10: Remove stats collector buffers
3. Week 11: Remove discovery cache file logic
4. Week 12: Archive old code, celebrate! 🎉