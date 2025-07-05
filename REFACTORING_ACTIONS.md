# Refactoring Action Plan - Prioritized Implementation

## Quick Wins (Day 1-2)
**Effort: 4-6 hours | Impact: High**

### 1. Critical Mutex Fixes
**Files to modify:**
- `src/server/request_coalescing.rs` (2 instances)

**Action:**
```bash
# Search and fix all lock().unwrap() patterns
grep -n "lock().*unwrap()" src/ -r --include="*.rs"
```

**Implementation:**
- Replace with error propagation or safe wrappers
- Add logging for lock failures
- Test with concurrent load

### 2. JSON Serialization Safety
**Files to modify:**
- `src/tools/devices.rs`
- `src/tools/climate.rs`
- `src/tools/documentation.rs`

**Action:**
```bash
# Find all serde_json unwraps
grep -n "serde_json.*unwrap()" src/tools/ -r
```

**Implementation:**
- Create `safe_json_response()` helper function
- Propagate serialization errors properly

## Week 1: High-Risk Parse Operations
**Effort: 8-10 hours | Impact: High**

### 3. HTTP Header Parsing
**Files to modify:**
- `src/performance/middleware.rs` (6 instances)
- `src/auth/middleware.rs` (1 instance)
- `src/security/middleware.rs` (1 instance)

**Pattern:** `parse().unwrap()` on HeaderValue

**Action:**
```bash
# Audit all header parsing
grep -n "parse().*unwrap()" src/**/middleware.rs
```

**Implementation:**
- Create `parse_header_value()` helper
- Add validation for header values
- Test with malformed headers

### 4. Configuration Parsing
**Files to modify:**
- `src/config/mod.rs` (4 instances)
- `src/bin/loxone-mcp-auth.rs` (1 instance)
- `src/server/resources.rs` (1 instance)

**Action:**
```bash
# Find config-related parse unwraps
grep -n "parse().*unwrap()" src/config/ src/bin/ -r
```

**Implementation:**
- Add proper error context
- Validate configuration at startup
- Provide helpful error messages

## Week 2: API Parameter Safety
**Effort: 12-16 hours | Impact: Medium**

### 5. Backend Parameter Extraction
**File:** `src/framework_integration/backend.rs` (30+ instances)

**Action:**
```bash
# Count parameter unwraps
grep -c "params.get.*unwrap()" src/framework_integration/backend.rs
```

**Implementation Plan:**
1. Create parameter extraction utilities:
   ```rust
   mod param_utils {
       pub fn extract_string(params: &Map<String, Value>, key: &str) -> Result<&str>
       pub fn extract_bool(params: &Map<String, Value>, key: &str) -> Result<bool>
       pub fn extract_number(params: &Map<String, Value>, key: &str) -> Result<f64>
   }
   ```

2. Refactor each handler systematically:
   - Group by parameter type
   - Add validation rules
   - Improve error messages

3. Add integration tests for missing parameters

### 6. Tool Parameter Validation
**Files:** All files in `src/tools/` directory

**Action:**
```bash
# Audit tool parameter handling
grep -n "get.*unwrap()" src/tools/ -r
```

**Implementation:**
- Standardize parameter extraction across tools
- Add input validation layer
- Create tool-specific error types

## Week 3: Performance Optimization
**Effort: 16-20 hours | Impact: High for performance**

### 7. Hot Path Clone Reduction
**Priority Files:**
- `src/tools/sensors_unified.rs` (8 instances of UUID cloning)
- `src/tools/security.rs` (5 instances)
- `src/tools/lighting.rs` (3 instances)

**Action:**
```bash
# Find clone patterns in loops
grep -B2 -A2 "iter.*map.*clone()" src/tools/ -r
```

**Implementation Strategy:**
1. **Phase 1**: Change APIs to accept iterators
2. **Phase 2**: Use `Cow<str>` for flexible ownership
3. **Phase 3**: Implement UUID interning for frequently used IDs

### 8. Configuration Clone Optimization
**Files:**
- Service initialization code
- Request handlers passing config

**Action:**
```bash
# Find config clones
grep -n "config.*clone()" src/ -r --include="*.rs"
```

**Implementation:**
- Wrap configs in `Arc<Config>`
- Use partial cloning for subsystems
- Implement `Clone` more efficiently

## Week 4: Error Message Optimization
**Effort: 8-10 hours | Impact: Medium**

### 9. Error Propagation Efficiency
**Files:**
- `src/server/request_coalescing.rs`
- Error handling throughout codebase

**Implementation:**
- Use `Arc<str>` for error messages
- Implement error interning
- Create static error constants

### 10. Collection Entry Patterns
**Files:** Various aggregation code

**Action:**
```bash
# Find entry().clone() patterns
grep -n "entry.*clone()" src/ -r
```

**Implementation:**
- Use `Cow` for map keys
- Consider `smallstr` for short strings
- Profile memory usage

## Continuous Improvements

### Automation Setup (Week 5)
1. **Clippy Configuration**
   ```toml
   # .clippy.toml
   warn = [
       "clippy::unwrap_used",
       "clippy::expect_used",
       "clippy::panic",
       "clippy::unimplemented"
   ]
   ```

2. **Pre-commit Hook**
   ```bash
   #!/bin/bash
   # .git/hooks/pre-commit
   
   # Check for new unwraps
   if git diff --cached --name-only | xargs grep -l "\.unwrap()" > /dev/null; then
       echo "Error: New .unwrap() calls detected. Use proper error handling."
       exit 1
   fi
   ```

3. **CI Pipeline Addition**
   ```yaml
   - name: Check unwrap usage
     run: |
       unwrap_count=$(grep -r "\.unwrap()" src/ --include="*.rs" | wc -l)
       if [ $unwrap_count -gt $UNWRAP_THRESHOLD ]; then
         echo "Error: Too many unwrap() calls: $unwrap_count"
         exit 1
       fi
   ```

## Measurement and Validation

### Metrics to Track
1. **Safety Metrics**
   - Panic rate in production
   - Error message quality score
   - Test coverage for error paths

2. **Performance Metrics**
   - Memory allocations per request
   - String allocation overhead
   - Clone operation count

3. **Code Quality Metrics**
   - Unwrap count trend
   - Error handling consistency
   - Code review time

### Validation Steps
1. Run stress tests after each phase
2. Monitor production error rates
3. Profile memory usage before/after
4. Collect developer feedback

## Rollback Plan
Each refactoring should be:
1. Feature-flagged if high risk
2. Deployed incrementally
3. Monitored for 24 hours
4. Reverted if issues arise

## Success Criteria
- [ ] Zero panics in production for 30 days
- [ ] 30% reduction in memory allocations
- [ ] 100% of unwraps have explicit handling
- [ ] All parse operations validate input
- [ ] Performance benchmarks show improvement