//! Performance monitoring middleware for HTTP server integration

use super::{PerformanceContext, PerformanceMeasurement, PerformanceMonitor};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Performance middleware state
#[derive(Clone)]
pub struct PerformanceMiddleware {
    monitor: Arc<PerformanceMonitor>,
    active_measurements: Arc<RwLock<std::collections::HashMap<String, PerformanceMeasurement>>>,
}

impl PerformanceMiddleware {
    /// Create new performance middleware
    pub fn new(monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            monitor,
            active_measurements: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Extract operation information from request
    fn extract_operation_info(method: &Method, uri: &Uri) -> (String, String) {
        let operation_type = match *method {
            Method::GET => "http_get",
            Method::POST => "http_post",
            Method::PUT => "http_put",
            Method::DELETE => "http_delete",
            Method::PATCH => "http_patch",
            Method::OPTIONS => "http_options",
            Method::HEAD => "http_head",
            _ => "http_other",
        }
        .to_string();

        let operation_id = format!("{}_{}", operation_type, uri.path().replace('/', "_"));

        (operation_id, operation_type)
    }

    /// Extract client identifier from headers
    fn extract_client_id(headers: &HeaderMap) -> Option<String> {
        // Try various headers to identify the client
        if let Some(client_id) = headers.get("x-client-id").and_then(|v| v.to_str().ok()) {
            return Some(client_id.to_string());
        }

        if let Some(user_agent) = headers.get("user-agent").and_then(|v| v.to_str().ok()) {
            return Some(format!("ua:{}", &user_agent[..20.min(user_agent.len())]));
        }

        if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            return Some(format!("api:{}", &api_key[..8.min(api_key.len())]));
        }

        None
    }

    /// Create performance context from request
    fn create_context(method: &Method, uri: &Uri, headers: &HeaderMap) -> PerformanceContext {
        let (operation_id, operation_type) = Self::extract_operation_info(method, uri);
        let request_id = Uuid::new_v4().to_string();

        let mut context = PerformanceContext::new(
            format!("{}_{}", operation_id, &request_id[..8]),
            operation_type,
        );

        if let Some(client_id) = Self::extract_client_id(headers) {
            context = context.with_client_id(client_id);
        }

        // Add request context data
        context = context
            .with_context("method".to_string(), method.to_string())
            .with_context("path".to_string(), uri.path().to_string())
            .with_context("request_id".to_string(), request_id);

        if let Some(query) = uri.query() {
            context = context.with_context("query".to_string(), query.to_string());
        }

        context
    }

    /// Add performance headers to response
    fn add_performance_headers(response: &mut Response, measurement: &PerformanceMeasurement) {
        let headers = response.headers_mut();

        // Add timing information
        if let Some(duration) = measurement.timing.get_duration() {
            headers.insert(
                "X-Response-Time",
                format!("{}ms", duration.as_millis()).parse().unwrap(),
            );
        }

        // Add request ID for tracing
        headers.insert(
            "X-Request-ID",
            measurement.context.operation_id.clone().parse().unwrap(),
        );

        // Add performance metrics if available
        if let Some(cpu) = measurement.resource_usage.cpu_usage {
            headers.insert("X-CPU-Usage", format!("{cpu:.1}").parse().unwrap());
        }

        if let Some(memory) = measurement.resource_usage.memory_usage {
            headers.insert("X-Memory-Usage", format!("{memory}").parse().unwrap());
        }

        // Add performance score if issues detected
        if !measurement.issues.is_empty() {
            let critical_issues = measurement
                .issues
                .iter()
                .filter(|issue| {
                    matches!(
                        issue.severity,
                        crate::performance::PerformanceIssueSeverity::Critical
                    )
                })
                .count();

            if critical_issues > 0 {
                headers.insert(
                    "X-Performance-Warning",
                    "critical-issues-detected".parse().unwrap(),
                );
            }
        }
    }

    /// Log performance measurement
    fn log_performance(measurement: &PerformanceMeasurement, status_code: StatusCode) {
        let duration = measurement
            .timing
            .get_duration()
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let operation = &measurement.context.operation_type;
        let unknown_string = "unknown".to_string();
        let path = measurement
            .context
            .context_data
            .get("path")
            .unwrap_or(&unknown_string);

        if !measurement.issues.is_empty() {
            let critical_issues = measurement
                .issues
                .iter()
                .filter(|issue| {
                    matches!(
                        issue.severity,
                        crate::performance::PerformanceIssueSeverity::Critical
                    )
                })
                .count();

            if critical_issues > 0 {
                warn!(
                    "Performance critical: {} {} - {}ms - {} - {} critical issues",
                    operation, path, duration, status_code, critical_issues
                );
            } else {
                warn!(
                    "Performance warning: {} {} - {}ms - {} - {} issues",
                    operation,
                    path,
                    duration,
                    status_code,
                    measurement.issues.len()
                );
            }
        } else if duration > 1000 {
            info!(
                "Performance slow: {} {} - {}ms - {}",
                operation, path, duration, status_code
            );
        } else {
            debug!(
                "Performance: {} {} - {}ms - {}",
                operation, path, duration, status_code
            );
        }
    }
}

/// Performance monitoring middleware handler for Axum
pub async fn performance_middleware_handler(
    State(perf_middleware): State<Arc<PerformanceMiddleware>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();

    // Create performance context
    let context = PerformanceMiddleware::create_context(&method, &uri, &headers);
    let measurement_id = context.operation_id.clone();

    debug!("Starting performance measurement for: {}", measurement_id);

    // Start performance measurement
    let measurement = match perf_middleware.monitor.start_measurement(context).await {
        Ok(measurement) => measurement,
        Err(e) => {
            warn!("Failed to start performance measurement: {}", e);
            // Continue without performance monitoring
            return Ok(next.run(request).await);
        }
    };

    // Store active measurement
    {
        let mut active_measurements = perf_middleware.active_measurements.write().await;
        active_measurements.insert(measurement_id.clone(), measurement);
    }

    // Process request
    let response = next.run(request).await;
    let status_code = response.status();

    // Retrieve and finish measurement
    let measurement = {
        let mut active_measurements = perf_middleware.active_measurements.write().await;
        active_measurements.remove(&measurement_id)
    };

    if let Some(mut measurement) = measurement {
        // Finish performance measurement
        match perf_middleware
            .monitor
            .finish_measurement(measurement.clone())
            .await
        {
            Ok(finished_measurement) => {
                measurement = finished_measurement;

                // Add performance headers to response
                let mut response = response;
                PerformanceMiddleware::add_performance_headers(&mut response, &measurement);

                // Log performance information
                PerformanceMiddleware::log_performance(&measurement, status_code);

                // Record additional metrics
                let request_duration = start_time.elapsed();
                let mut tags = std::collections::HashMap::new();
                tags.insert("method".to_string(), method.to_string());
                tags.insert("status".to_string(), status_code.as_u16().to_string());
                tags.insert("path".to_string(), uri.path().to_string());

                if let Some(client_id) = &measurement.context.client_id {
                    tags.insert("client".to_string(), client_id.clone());
                }

                // Record latency metric
                if let Err(e) = perf_middleware
                    .monitor
                    .record_metric(
                        "http_request_duration_ms".to_string(),
                        request_duration.as_millis() as f64,
                        tags.clone(),
                    )
                    .await
                {
                    debug!("Failed to record latency metric: {}", e);
                }

                // Record request count metric
                if let Err(e) = perf_middleware
                    .monitor
                    .record_metric("http_requests_total".to_string(), 1.0, tags)
                    .await
                {
                    debug!("Failed to record request count metric: {}", e);
                }

                Ok(response)
            }
            Err(e) => {
                warn!("Failed to finish performance measurement: {}", e);
                Ok(response)
            }
        }
    } else {
        // Measurement was lost, just return response
        warn!("Performance measurement lost for: {}", measurement_id);
        Ok(response)
    }
}

/// Performance statistics endpoint handler
pub async fn performance_stats_handler(
    State(perf_middleware): State<Arc<PerformanceMiddleware>>,
) -> std::result::Result<axum::Json<crate::performance::PerformanceStatistics>, StatusCode> {
    match perf_middleware.monitor.get_statistics().await {
        Ok(stats) => Ok(axum::Json(stats)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Performance metrics endpoint handler (Prometheus format)
pub async fn performance_metrics_handler(
    State(perf_middleware): State<Arc<PerformanceMiddleware>>,
) -> std::result::Result<String, StatusCode> {
    match perf_middleware.monitor.get_statistics().await {
        Ok(stats) => {
            let mut metrics = String::new();

            // HTTP request duration metrics
            metrics.push_str(
                "# HELP http_request_duration_seconds HTTP request duration in seconds\n",
            );
            metrics.push_str("# TYPE http_request_duration_seconds histogram\n");
            metrics.push_str(&format!(
                "http_request_duration_seconds_sum {}\n",
                stats.request_stats.avg_response_time.as_secs_f64()
                    * stats.request_stats.total_requests as f64
            ));
            metrics.push_str(&format!(
                "http_request_duration_seconds_count {}\n",
                stats.request_stats.total_requests
            ));

            // Requests per second
            metrics.push_str("# HELP http_requests_per_second Current HTTP requests per second\n");
            metrics.push_str("# TYPE http_requests_per_second gauge\n");
            metrics.push_str(&format!(
                "http_requests_per_second {}\n",
                stats.request_stats.requests_per_second
            ));

            // Error rate
            metrics.push_str("# HELP http_error_rate HTTP error rate percentage\n");
            metrics.push_str("# TYPE http_error_rate gauge\n");
            metrics.push_str(&format!(
                "http_error_rate {}\n",
                100.0 - stats.request_stats.success_rate
            ));

            // Resource usage
            metrics.push_str("# HELP cpu_usage_percent CPU usage percentage\n");
            metrics.push_str("# TYPE cpu_usage_percent gauge\n");
            metrics.push_str(&format!(
                "cpu_usage_percent {}\n",
                stats.resource_stats.avg_cpu_usage
            ));

            metrics.push_str("# HELP memory_usage_bytes Memory usage in bytes\n");
            metrics.push_str("# TYPE memory_usage_bytes gauge\n");
            metrics.push_str(&format!(
                "memory_usage_bytes {}\n",
                stats.resource_stats.avg_memory_usage
            ));

            Ok(metrics)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Performance trends endpoint handler
pub async fn performance_trends_handler(
    State(perf_middleware): State<Arc<PerformanceMiddleware>>,
) -> std::result::Result<axum::Json<crate::performance::PerformanceTrends>, StatusCode> {
    match perf_middleware.monitor.get_statistics().await {
        Ok(stats) => Ok(axum::Json(stats.trends)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Create performance monitoring router
pub fn create_performance_router(perf_middleware: Arc<PerformanceMiddleware>) -> axum::Router {
    use axum::routing::get;

    axum::Router::new()
        .route("/stats", get(performance_stats_handler))
        .route("/metrics", get(performance_metrics_handler))
        .route("/trends", get(performance_trends_handler))
        .with_state(perf_middleware)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{PerformanceConfig, PerformanceMonitor};

    use axum::http::{Method, Uri};

    #[test]
    fn test_extract_operation_info() {
        let method = Method::GET;
        let uri: Uri = "/api/devices".parse().unwrap();

        let (operation_id, operation_type) =
            PerformanceMiddleware::extract_operation_info(&method, &uri);

        assert_eq!(operation_type, "http_get");
        assert!(operation_id.starts_with("http_get_"));
    }

    #[test]
    fn test_extract_client_id() {
        let mut headers = HeaderMap::new();
        headers.insert("x-client-id", "test-client-123".parse().unwrap());

        let client_id = PerformanceMiddleware::extract_client_id(&headers);
        assert_eq!(client_id, Some("test-client-123".to_string()));
    }

    #[test]
    fn test_create_context() {
        let method = Method::POST;
        let uri: Uri = "/api/devices?room=kitchen".parse().unwrap();
        let headers = HeaderMap::new();

        let context = PerformanceMiddleware::create_context(&method, &uri, &headers);

        assert_eq!(context.operation_type, "http_post");
        assert_eq!(
            context.context_data.get("method"),
            Some(&"POST".to_string())
        );
        assert_eq!(
            context.context_data.get("path"),
            Some(&"/api/devices".to_string())
        );
        assert_eq!(
            context.context_data.get("query"),
            Some(&"room=kitchen".to_string())
        );
    }

    #[tokio::test]
    async fn test_performance_middleware_creation() {
        let config = PerformanceConfig::testing();
        let monitor = Arc::new(PerformanceMonitor::new(config).unwrap());
        let middleware = PerformanceMiddleware::new(monitor);

        assert_eq!(middleware.active_measurements.read().await.len(), 0);
    }
}
