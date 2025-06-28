# mcp-monitoring

**Monitoring, metrics, and observability for MCP servers**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/avrabe/mcp-loxone/blob/main/LICENSE)

This crate provides monitoring and observability features for MCP servers, including metrics collection, health checks, and performance tracking.

## What This Provides

**Metrics Collection:**
- Request/response timing and throughput
- Error rates and types
- Tool usage statistics
- Resource access patterns
- Client connection metrics

**Health Monitoring:**
- Server health checks with detailed status
- Backend connectivity validation
- Resource availability checks
- Performance threshold monitoring

**Observability:**
- Structured logging integration
- Request tracing with correlation IDs
- Performance profiling hooks
- Custom metric collection

## Real-World Usage

This monitoring system is actively used in the **Loxone MCP Server** where it:
- Tracks usage of 30+ home automation tools
- Monitors device response times and errors
- Provides health checks for HTTP transport endpoints
- Collects performance metrics for optimization
- Integrates with system monitoring dashboards

## Quick Start

```toml
[dependencies]
mcp-monitoring = "0.1.0"
mcp-protocol = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Basic Usage

### Health Checks

```rust
use mcp_monitoring::{HealthChecker, HealthConfig, HealthStatus};

// Configure health checks
let config = HealthConfig {
    check_interval_seconds: 30,
    timeout_seconds: 5,
    failure_threshold: 3,
};

let mut health_checker = HealthChecker::new(config);

// Add custom health checks
health_checker.add_check("database", Box::new(|_| {
    Box::pin(async {
        // Check database connectivity
        match database_ping().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("DB error: {}", e)),
        }
    })
}));

// Start monitoring
health_checker.start().await?;

// Check current health
let status = health_checker.get_status().await;
println!("Server health: {:?}", status);
```

### Metrics Collection

```rust
use mcp_monitoring::{MetricsCollector, MetricType, Metric};

let collector = MetricsCollector::new();

// Track tool usage
collector.record(Metric {
    name: "tool_calls_total".to_string(),
    metric_type: MetricType::Counter,
    value: 1.0,
    labels: vec![
        ("tool".to_string(), "get_weather".to_string()),
        ("status".to_string(), "success".to_string()),
    ],
    timestamp: chrono::Utc::now(),
});

// Track response times
collector.record(Metric {
    name: "request_duration_seconds".to_string(),
    metric_type: MetricType::Histogram,
    value: 0.150, // 150ms
    labels: vec![("endpoint".to_string(), "/mcp".to_string())],
    timestamp: chrono::Utc::now(),
});
```

### Performance Tracking

```rust
use mcp_monitoring::{PerformanceTracker, TrackingConfig};

let tracker = PerformanceTracker::new(TrackingConfig {
    enable_detailed_timing: true,
    track_memory_usage: true,
    sample_rate: 1.0, // Track 100% of requests
});

// Track a request
let request_id = tracker.start_request("tool_call", "get_device_status").await;

// Your business logic here
let result = execute_tool_call().await;

// Complete tracking
tracker.finish_request(request_id, result.is_ok()).await;
```

## Current Status

**Useful for basic monitoring with room for advanced features.** The core monitoring functionality works well for understanding server behavior and performance.

**What works well:**
- ✅ Basic health check system
- ✅ Request timing and error tracking
- ✅ Tool usage statistics
- ✅ Integration with HTTP transport
- ✅ Structured logging integration

**Areas for improvement:**
- 📊 More sophisticated metrics aggregation
- 🔧 Better alerting and notification systems
- 📝 More examples for different monitoring setups
- 🧪 Testing utilities for monitoring scenarios

## Health Check System

### Built-in Health Checks

```rust
use mcp_monitoring::builtin_checks;

// Add standard health checks
health_checker.add_check("memory", builtin_checks::memory_usage(80.0)); // 80% threshold
health_checker.add_check("disk", builtin_checks::disk_space("/tmp", 90.0));
health_checker.add_check("cpu", builtin_checks::cpu_usage(95.0));
```

### Custom Health Checks

```rust
use mcp_monitoring::{HealthCheck, HealthStatus};

struct DatabaseHealthCheck {
    connection_pool: DatabasePool,
}

#[async_trait]
impl HealthCheck for DatabaseHealthCheck {
    async fn check(&self) -> HealthStatus {
        match self.connection_pool.ping().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("Database unreachable: {}", e)),
        }
    }
    
    fn name(&self) -> &str {
        "database"
    }
}

health_checker.add_check_instance(Box::new(DatabaseHealthCheck {
    connection_pool: db_pool,
}));
```

### Health Endpoints

```rust
// Expose health checks via HTTP
use axum::{Router, Json};
use mcp_monitoring::HealthChecker;

async fn health_endpoint(
    health_checker: &HealthChecker,
) -> Json<serde_json::Value> {
    let status = health_checker.get_detailed_status().await;
    Json(serde_json::json!({
        "status": status.overall,
        "checks": status.checks,
        "timestamp": chrono::Utc::now()
    }))
}

let app = Router::new()
    .route("/health", get(health_endpoint));
```

## Metrics System

### Metric Types

```rust
use mcp_monitoring::MetricType;

// Counter - Always increasing values
MetricType::Counter // Total requests, total errors

// Gauge - Current value
MetricType::Gauge // Active connections, memory usage

// Histogram - Distribution of values
MetricType::Histogram // Request durations, response sizes

// Summary - Similar to histogram with quantiles
MetricType::Summary // Response time percentiles
```

### Common Metrics

```rust
// Request metrics
collector.increment_counter("requests_total", &[
    ("method", "POST"),
    ("endpoint", "/mcp"),
]);

collector.record_histogram("request_duration_seconds", duration.as_secs_f64(), &[
    ("endpoint", "/mcp"),
    ("status", "200"),
]);

// Tool usage metrics
collector.increment_counter("tool_calls_total", &[
    ("tool", "control_device"),
    ("status", "success"),
]);

// Error tracking
collector.increment_counter("errors_total", &[
    ("type", "validation_error"),
    ("tool", "get_weather"),
]);
```

### Integration with MCP Server

```rust
use mcp_server::{ServerConfig, MiddlewareConfig};
use mcp_monitoring::MonitoringMiddleware;

let monitoring_config = MonitoringConfig {
    enable_metrics: true,
    enable_health_checks: true,
    metrics_endpoint: Some("/metrics".to_string()),
    health_endpoint: Some("/health".to_string()),
};

let server_config = ServerConfig {
    middleware_config: MiddlewareConfig {
        monitoring: Some(monitoring_config),
        // ... other middleware
    },
    // ... other config
};

// Monitoring happens automatically
```

## Performance Tracking

### Request Tracing

```rust
use mcp_monitoring::RequestTracer;

let tracer = RequestTracer::new();

// Start tracing a request
let trace_id = tracer.start_trace("mcp_request");
tracer.add_span(trace_id, "validation", start_time, duration);
tracer.add_span(trace_id, "backend_call", start_time, duration);
tracer.add_span(trace_id, "response_formatting", start_time, duration);

// Complete the trace
tracer.finish_trace(trace_id);
```

### Memory and Resource Monitoring

```rust
use mcp_monitoring::ResourceMonitor;

let monitor = ResourceMonitor::new();

// Track resource usage
let snapshot = monitor.take_snapshot().await;
println!("Memory usage: {} MB", snapshot.memory_mb);
println!("CPU usage: {}%", snapshot.cpu_percent);
println!("Open connections: {}", snapshot.connections);
```

## Real-World Examples

### Loxone Server Monitoring

```rust
// Monitor home automation tool performance
collector.record_histogram("device_response_time", response_time, &[
    ("device_type", "light"),
    ("room", "living_room"),
]);

// Track automation success rates
collector.increment_counter("automation_executions", &[
    ("type", "rolladen_control"),
    ("result", if success { "success" } else { "failure" }),
]);

// Monitor connection health
health_checker.add_check("loxone_miniserver", Box::new(|_| {
    Box::pin(async {
        match ping_miniserver().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("Miniserver unreachable: {}", e)),
        }
    })
}));
```

### Dashboard Integration

```rust
// Expose metrics for Grafana/Prometheus
use mcp_monitoring::prometheus_exporter;

let exporter = prometheus_exporter::new(&collector);
let metrics_data = exporter.export().await;

// Returns Prometheus format:
// # HELP tool_calls_total Total number of tool calls
// # TYPE tool_calls_total counter
// tool_calls_total{tool="control_device",status="success"} 150
```

## Contributing

Monitoring and observability can always be improved. Most valuable contributions:

1. **New metric types** - Domain-specific metrics for MCP servers
2. **Integration examples** - How to integrate with popular monitoring systems
3. **Performance optimization** - Low-overhead monitoring approaches
4. **Alerting systems** - Smart alerting based on MCP server patterns

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

**Repository:** https://github.com/avrabe/mcp-loxone