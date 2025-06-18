# Exhaustive TODO List - Loxone MCP Rust Server Consolidation

## Overview
This document provides a step-by-step implementation plan where each step is independently buildable and usable. The first 10% focuses on fixing clippy and build warnings.

## Phase 0: Code Quality (10% - Immediate)

### 1. Fix Clippy Warnings ✓ (Completed)
```bash
cargo clippy --fix --lib -p loxone-mcp-rust
```
- Fix unused imports
- Fix unused variables
- Fix redundant clones
- Fix unnecessary mutability

### 2. Fix Build Warnings
```bash
cargo build --release
```
- Fix deprecated API usage
- Update dependency versions
- Remove unused dependencies
- Fix feature flag warnings

### 3. Format Code
```bash
cargo fmt
```
- Ensure consistent formatting
- Fix line length issues

## Phase 1: Critical Fixes (Week 1)

### 4. Fix Sensor Value Display Issue
**Problem**: Dashboards show "Off"/"Idle" instead of real values
**Solution**: Wire up UnifiedValueResolver

```rust
// In server/mod.rs
impl ServerContext {
    pub async fn get_sensor_value(&self, uuid: &str) -> Result<SensorValue> {
        self.value_resolver.resolve_sensor_value(uuid).await
    }
}
```

**Files to modify**:
- `server/mod.rs` - Add value_resolver to ServerContext
- `server/handlers.rs` - Use value_resolver instead of direct access
- `tools/sensors.rs` - Route through value_resolver

**Testable**: Dashboard should show real sensor values

### 5. Implement Batch API Calls
**Problem**: 50-100 individual API calls per dashboard load
**Solution**: Add batch endpoint to LoxoneHttpClient

```rust
// In client/loxone_http_client.rs
pub async fn get_batch_values(&self, uuids: &[String]) -> Result<HashMap<String, f64>> {
    let request = format!("jdev/sps/batch/{}", uuids.join(","));
    // Implementation
}
```

**Files to modify**:
- `client/loxone_http_client.rs` - Add batch method
- `services/value_resolution.rs` - Use batch calls
- `http_transport/dashboard_data_unified.rs` - Batch sensor requests

**Testable**: Network tab should show 1-3 requests instead of 50+

## Phase 2: Dashboard Consolidation (Week 2)

### 6. Switch to Unified Dashboard
**Problem**: 8 different dashboard implementations
**Solution**: Use only dashboard_data_unified.rs

**Steps**:
1. Update `http_transport/mod.rs` to use unified dashboard
2. Remove route to legacy dashboard
3. Test dashboard still works
4. Delete `dashboard_data.rs` (save 200+ lines)

**Testable**: Dashboard loads with cleaner code

### 7. Remove Duplicate Dashboard UIs
**Problem**: Multiple monitoring dashboard implementations
**Solution**: Keep only one clean implementation

**Delete these files** (one at a time, test after each):
1. `monitoring/unified_dashboard.rs` (original complex version)
2. `monitoring/unified_dashboard_new.rs` (failed rewrite)
3. `monitoring/key_management_ui.rs` (duplicate UI)
4. `monitoring/key_management_ui_new.rs` (another duplicate)
5. `history/dashboard.rs` (unused)

**Keep**: `monitoring/clean_dashboard.rs`

**Testable**: Monitoring dashboard still accessible at /admin

## Phase 3: Cache Consolidation (Week 3)

### 8. Activate EnhancedCacheManager
**Problem**: 5 different cache implementations
**Solution**: Use only EnhancedCacheManager

**Steps**:
1. Wire up EnhancedCacheManager in ServerContext
2. Replace value_cache with enhanced_cache
3. Remove old cache implementations
4. Update all cache access points

**Files to modify**:
- `server/mod.rs` - Replace value_cache
- `services/cache_manager.rs` - Ensure fully implemented
- `client/mod.rs` - Remove ClientContext cache

**Testable**: Cache hit rates visible in metrics

### 9. Remove Redundant Caches
**Delete**:
- Old value_cache from ServerContext
- ClientContext structure cache
- History storage cache

**Testable**: Memory usage reduced, single cache stats

## Phase 4: State Management (Week 4)

### 10. Wire Up StateManager
**Problem**: No unified state management
**Solution**: Activate existing StateManager

**Steps**:
1. Add StateManager to ServerContext
2. Route all state updates through it
3. Enable change notifications
4. Remove redundant state handling

**Files to modify**:
- `server/mod.rs` - Add state_manager
- `server/handlers.rs` - Use state_manager
- `services/state_manager.rs` - Ensure subscriptions work

**Testable**: State changes trigger notifications

## Phase 5: Service Layer Pattern (Week 5)

### 11. Route Tools Through Services
**Problem**: Tools directly access client
**Solution**: All tools use service layer

**For each tool**:
1. Replace direct client calls with service calls
2. Test tool still works
3. Move to next tool

**Priority order**:
1. `sensors.rs` - Most used
2. `lights.rs` - Common operations
3. `rolladen.rs` - Blind control
4. Others

**Testable**: Each tool works independently

### 12. Consolidate Similar Tools
**Problem**: 15+ tools with overlaps
**Solution**: Merge similar functionality

**Merge**:
- All light tools → single light tool with parameters
- All blind tools → single blind tool
- All sensor tools → single sensor tool

**Testable**: Fewer but more capable tools

## Phase 6: Dead Code Removal (Week 6)

### 13. Remove Unused Modules
**Delete entire directories**:
1. `history/` - Never integrated
2. `wasm/` - Not used
3. `audit_log/` - Not connected

**Testable**: Build still succeeds

### 14. Remove Unused Features
**Delete unused code**:
- Unused client methods
- Unused error types
- Unused utility functions
- Commented code blocks

**Testable**: Reduced binary size

## Phase 7: Performance Optimization (Week 7)

### 15. Implement Request Coalescing
**Problem**: Multiple concurrent requests
**Solution**: Batch concurrent requests

**Add RequestCoalescer**:
```rust
pub struct RequestCoalescer {
    pending: Arc<Mutex<HashMap<String, Vec<oneshot::Sender<Value>>>>>
}
```

**Testable**: Concurrent requests consolidated

### 16. Enable Predictive Prefetching
**Problem**: Cache misses on predictable patterns
**Solution**: Activate prefetching in EnhancedCacheManager

**Steps**:
1. Enable pattern learning
2. Set prefetch thresholds
3. Monitor prefetch accuracy

**Testable**: Higher cache hit rate

## Phase 8: Sensor Support (Week 8)

### 17. Implement All Sensor Types
**Add support for**:
- Motion sensors
- Door/window contacts
- Energy meters
- Air quality sensors
- Presence detectors
- Weather station data
- Alarm sensors
- Water sensors

**Files to modify**:
- `services/sensor_registry.rs` - Add types
- `services/value_parsers.rs` - Add parsers
- `tools/sensors.rs` - Add specific methods

**Testable**: Each sensor type returns correct values

### 18. Add Sensor Discovery
**Problem**: Manual sensor configuration
**Solution**: Auto-discovery of sensor types

**Implement behavioral analysis**:
- Monitor value patterns
- Detect sensor type
- Auto-register in registry

**Testable**: Unknown sensors auto-detected

## Phase 9: Real-time Updates (Week 9)

### 19. Activate WebSocket Client
**Problem**: No real-time updates
**Solution**: Enable WebSocket connection

**Steps**:
1. Wire up WebSocket client
2. Subscribe to value changes
3. Push updates to StateManager
4. Emit SSE events

**Testable**: Live value updates in dashboard

### 20. Implement Subscription System
**Problem**: No selective updates
**Solution**: Subscribe to specific devices/rooms

**Add subscription management**:
- Subscribe by UUID
- Subscribe by room
- Subscribe by device type
- Manage subscription lifecycle

**Testable**: Only subscribed updates received

## Phase 10: Final Optimization (Week 10)

### 21. Optimize Dashboard Load Time
**Goal**: <100ms load time

**Optimizations**:
1. Preload common data
2. Compress responses
3. Cache rendered HTML
4. Optimize database queries

**Testable**: Load time metrics

### 22. Add Integration Tests
**Coverage for**:
- Service layer
- API endpoints
- Tool operations
- Cache behavior
- State management

**Testable**: CI/CD pipeline green

## Success Metrics

After completing all steps:
1. **Code**: 50% reduction (7,500 lines)
2. **API Calls**: 95% reduction (1-3 per load)
3. **Performance**: <100ms dashboard load
4. **Sensors**: All types supported
5. **Real-time**: Live updates working
6. **Maintenance**: Single place to update

## Build Commands

After each step, run:
```bash
# Check it builds
cargo build --release

# Run clippy
cargo clippy -- -D warnings

# Run tests
cargo test

# Check formatting
cargo fmt -- --check
```

## Commit Strategy

Each completed step gets its own commit:
```bash
git add -p  # Stage specific changes
git commit -m "fix(step-N): [description]

- What was fixed
- Why it was needed
- Impact on system

Refs: #step-number"
```

This ensures each step is independently buildable and revertable if needed.