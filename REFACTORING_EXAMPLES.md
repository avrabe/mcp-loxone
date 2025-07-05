# Refactoring Examples and Patterns

## High Priority Refactoring Examples

### 1. Mutex Lock Handling

**Current Code (request_coalescing.rs:333)**
```rust
let mut metrics = self.metrics.lock().unwrap();
metrics.batch_executed(batch_size, execution_time);
```

**Refactored Code**
```rust
// Option 1: Propagate error
let mut metrics = self.metrics.lock()
    .map_err(|e| LoxoneError::Internal(format!("Metrics lock poisoned: {}", e)))?;
metrics.batch_executed(batch_size, execution_time);

// Option 2: Log and continue (for non-critical metrics)
match self.metrics.lock() {
    Ok(mut metrics) => {
        metrics.batch_executed(batch_size, execution_time);
    }
    Err(e) => {
        error!("Failed to update metrics: lock poisoned - {}", e);
        // Continue execution without updating metrics
    }
}

// Option 3: Create a safe wrapper
impl MetricsCollector {
    pub fn record_batch(&self, batch_size: usize, execution_time: Duration) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.batch_executed(batch_size, execution_time);
        } else {
            // Metrics are non-critical, log and continue
            error!("Metrics collection failed - lock poisoned");
        }
    }
}
```

### 2. Parameter Extraction Pattern

**Current Code (backend.rs:261)**
```rust
let room_name = params.get("room_name").unwrap();
let devices = self.loxone_service.get_room_devices(room_name).await?;
```

**Refactored Code**
```rust
// Option 1: Early validation with descriptive errors
let room_name = params.get("room_name")
    .and_then(|v| v.as_str())
    .ok_or_else(|| LoxoneError::Validation(
        "Missing or invalid 'room_name' parameter".to_string()
    ))?;
let devices = self.loxone_service.get_room_devices(room_name).await?;

// Option 2: Create a validation helper
fn extract_required_param<'a>(params: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    params.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| LoxoneError::Validation(
            format!("Missing or invalid required parameter: '{}'", key)
        ))
}

// Usage:
let room_name = extract_required_param(&params, "room_name")?;
let devices = self.loxone_service.get_room_devices(room_name).await?;

// Option 3: Use a macro for repeated patterns
macro_rules! extract_param {
    ($params:expr, $key:literal) => {
        $params.get($key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| LoxoneError::Validation(
                format!("Missing or invalid parameter: '{}'", $key)
            ))?
    };
}

let room_name = extract_param!(params, "room_name");
```

### 3. Parse Error Handling

**Current Code (resources.rs:825)**
```rust
let limit: u32 = value.parse().unwrap();
```

**Refactored Code**
```rust
// Option 1: Simple error propagation
let limit: u32 = value.parse()
    .map_err(|e| LoxoneError::Validation(
        format!("Invalid limit value '{}': {}", value, e)
    ))?;

// Option 2: With default fallback
let limit: u32 = value.parse().unwrap_or_else(|_| {
    warn!("Invalid limit value '{}', using default: 100", value);
    100
});

// Option 3: With validation bounds
let limit: u32 = value.parse()
    .map_err(|e| LoxoneError::Validation(
        format!("Invalid limit value '{}': {}", value, e)
    ))?;

if limit > 1000 {
    return Err(LoxoneError::Validation(
        format!("Limit {} exceeds maximum allowed value of 1000", limit)
    ));
}

// Option 4: Create a parsing helper
fn parse_limit(value: &str) -> Result<u32> {
    let limit = value.parse::<u32>()
        .map_err(|e| LoxoneError::Validation(
            format!("Invalid limit value '{}': {}", value, e)
        ))?;
    
    if limit == 0 {
        return Err(LoxoneError::Validation("Limit must be greater than 0".to_string()));
    }
    
    if limit > 1000 {
        return Err(LoxoneError::Validation(
            format!("Limit {} exceeds maximum allowed value of 1000", limit)
        ));
    }
    
    Ok(limit)
}
```

## Clone Optimization Examples

### 1. Hot Path UUID Collection

**Current Code (sensors_unified.rs)**
```rust
let uuids: Vec<String> = temperature_sensors.iter().map(|d| d.uuid.clone()).collect();
```

**Refactored Code**
```rust
// Option 1: Use references if lifetime allows
let uuids: Vec<&str> = temperature_sensors.iter().map(|d| d.uuid.as_str()).collect();

// Option 2: Use Cow for flexibility
use std::borrow::Cow;
let uuids: Vec<Cow<str>> = temperature_sensors.iter()
    .map(|d| Cow::Borrowed(&d.uuid))
    .collect();

// Option 3: If you need owned strings but want to avoid allocations
// Use a string interner or cache
struct UuidCache {
    cache: Arc<DashMap<String, Arc<str>>>,
}

impl UuidCache {
    fn intern(&self, uuid: &str) -> Arc<str> {
        self.cache.entry(uuid.to_string())
            .or_insert_with(|| Arc::from(uuid))
            .clone()
    }
}

// Option 4: Change API to accept iterators
async fn get_device_states<'a, I>(&self, uuids: I) -> Result<HashMap<String, DeviceState>>
where
    I: IntoIterator<Item = &'a str>,
{
    // Implementation
}

// Usage - no cloning needed
let states = client.get_device_states(
    temperature_sensors.iter().map(|d| d.uuid.as_str())
).await?;
```

### 2. Error Message Propagation

**Current Code (request_coalescing.rs:357)**
```rust
for request in batch.requests {
    let _ = request.response_sender
        .send(Err(LoxoneError::config(error_msg.clone())));
}
```

**Refactored Code**
```rust
// Option 1: Use Arc for shared error
let error = Arc::new(LoxoneError::config(error_msg));
for request in batch.requests {
    let _ = request.response_sender.send(Err(Arc::clone(&error)));
}

// Option 2: Create error once, clone lightweight enum
#[derive(Clone)]
enum LoxoneError {
    Config(Arc<str>), // Use Arc<str> instead of String
    // other variants...
}

let error = LoxoneError::Config(Arc::from(error_msg.as_str()));
for request in batch.requests {
    let _ = request.response_sender.send(Err(error.clone()));
}

// Option 3: Use static error messages where possible
const ERROR_BATCH_FAILED: &str = "Batch execution failed";
for request in batch.requests {
    let _ = request.response_sender
        .send(Err(LoxoneError::config(ERROR_BATCH_FAILED)));
}
```

### 3. Configuration Cloning

**Current Code**
```rust
struct Service {
    config: Config,
}

impl Service {
    async fn handle_request(&self) {
        let config = self.config.clone(); // Expensive clone
        spawn_task(config).await;
    }
}
```

**Refactored Code**
```rust
// Option 1: Use Arc
struct Service {
    config: Arc<Config>,
}

impl Service {
    async fn handle_request(&self) {
        let config = Arc::clone(&self.config); // Cheap Arc clone
        spawn_task(config).await;
    }
}

// Option 2: Pass references where possible
impl Service {
    async fn handle_request(&self) {
        spawn_task_with_ref(&self.config).await;
    }
}

// Option 3: Clone only needed fields
struct Service {
    config: Config,
}

impl Config {
    fn connection_params(&self) -> ConnectionParams {
        ConnectionParams {
            url: self.url.clone(), // Clone only what's needed
            timeout: self.timeout,
        }
    }
}

impl Service {
    async fn handle_request(&self) {
        let params = self.config.connection_params();
        spawn_task(params).await;
    }
}
```

## Helper Functions and Utilities

### Error Handling Utilities

```rust
/// Extension trait for Option to provide better error messages
trait OptionExt<T> {
    fn ok_or_validation<S: Into<String>>(self, param_name: S) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_validation<S: Into<String>>(self, param_name: S) -> Result<T> {
        self.ok_or_else(|| LoxoneError::Validation(
            format!("Missing required parameter: '{}'", param_name.into())
        ))
    }
}

// Usage:
let room_name = params.get("room_name")
    .and_then(|v| v.as_str())
    .ok_or_validation("room_name")?;
```

### Safe Mutex Wrapper

```rust
/// A wrapper around Mutex that handles poisoned locks gracefully
pub struct SafeMutex<T> {
    inner: Mutex<T>,
    default: fn() -> T,
}

impl<T> SafeMutex<T> {
    pub fn new(value: T, default: fn() -> T) -> Self {
        Self {
            inner: Mutex::new(value),
            default,
        }
    }
    
    pub fn with<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        match self.inner.lock() {
            Ok(mut guard) => Some(f(&mut *guard)),
            Err(poisoned) => {
                error!("Mutex poisoned, recreating with default value");
                let mut guard = poisoned.into_inner();
                *guard = (self.default)();
                Some(f(&mut *guard))
            }
        }
    }
}
```

## Testing Patterns

### Testing Error Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_missing_parameter() {
        let params = serde_json::json!({
            "other_param": "value"
        });
        
        let result = extract_required_param(
            params.as_object().unwrap(), 
            "room_name"
        );
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("room_name"));
    }
    
    #[test]
    fn test_invalid_parse() {
        let result = parse_limit("not_a_number");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid limit"));
    }
    
    #[test]
    fn test_mutex_poisoning() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex_clone = Arc::clone(&mutex);
        
        // Poison the mutex
        let handle = std::thread::spawn(move || {
            let _guard = mutex_clone.lock().unwrap();
            panic!("Poisoning mutex");
        });
        
        let _ = handle.join();
        
        // Should handle poisoned mutex gracefully
        let result = mutex.lock();
        assert!(result.is_err());
    }
}
```