//! Embedded web dashboard for monitoring
//!
//! This module provides a local web dashboard with real-time charts and metrics visualization.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        Html, IntoResponse, Response,
    },
    routing::get,
    Router,
};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

#[cfg(feature = "influxdb")]
use super::influxdb::InfluxManager;
use super::metrics::MetricsCollector;

/// Dashboard state
#[derive(Clone)]
pub struct DashboardState {
    pub metrics_collector: Arc<MetricsCollector>,
    #[cfg(feature = "influxdb")]
    pub influx_manager: Option<Arc<InfluxManager>>,
}

/// Time range query parameters
#[derive(Debug, Deserialize)]
struct TimeRangeQuery {
    /// Time range: "5m", "10m", "1h", "6h", "24h", "7d", "30d"
    range: Option<String>,
    /// Custom start time (ISO 8601)
    start: Option<String>,
    /// Custom end time (ISO 8601)
    end: Option<String>,
}

/// Dashboard routes
pub fn dashboard_routes<S>(state: DashboardState) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(dashboard_index))
        .route("/metrics/live", get(metrics_stream))
        .route("/api/metrics", get(api_metrics))
        .route("/api/history/:sensor_id", get(api_sensor_history))
        .route("/api/historical", get(api_historical_metrics))
        .with_state(state)
}

/// Main dashboard HTML page
async fn dashboard_index() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// Server-sent events stream for live metrics
async fn metrics_stream(
    State(state): State<DashboardState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = stream::unfold(
        (state, interval(Duration::from_secs(1))),
        |(state, mut interval)| async move {
            interval.tick().await;

            // Collect system metrics before exporting
            state.metrics_collector.collect_system_metrics().await;

            // Push to InfluxDB if configured
            #[cfg(feature = "influxdb")]
            if let Err(e) = state.metrics_collector.push_to_influx().await {
                tracing::warn!("Failed to push metrics to InfluxDB: {}", e);
            }

            // Collect current metrics
            let metrics = state.metrics_collector.export_prometheus().await;

            // Parse metrics for dashboard
            let dashboard_data = parse_metrics_for_dashboard(&metrics);

            let event =
                Event::default().data(serde_json::to_string(&dashboard_data).unwrap_or_default());

            Some((Ok(event), (state, interval)))
        },
    );

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    )
}

/// API endpoint for current metrics
async fn api_metrics(State(state): State<DashboardState>) -> Response {
    let metrics = state.metrics_collector.export_prometheus().await;
    let dashboard_data = parse_metrics_for_dashboard(&metrics);

    Json(dashboard_data).into_response()
}

/// API endpoint for sensor history
async fn api_sensor_history(
    State(state): State<DashboardState>,
    axum::extract::Path(sensor_id): axum::extract::Path<String>,
    Query(params): Query<TimeRangeQuery>,
) -> Response {
    #[cfg(feature = "influxdb")]
    if let Some(influx) = &state.influx_manager {
        let time_range = params.range.as_deref().unwrap_or("24h");
        match influx.query_sensor_history(&sensor_id, time_range).await {
            Ok(history) => {
                let data: Vec<HistoryPoint> = history
                    .into_iter()
                    .map(|(time, value)| HistoryPoint {
                        timestamp: time.timestamp_millis(),
                        value,
                    })
                    .collect();
                return Json(data).into_response();
            }
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {e}")).into_response();
            }
        }
    }

    (StatusCode::NOT_IMPLEMENTED, "InfluxDB not configured").into_response()
}

/// API endpoint for historical metrics
async fn api_historical_metrics(
    State(state): State<DashboardState>,
    Query(params): Query<TimeRangeQuery>,
) -> Response {
    #[cfg(feature = "influxdb")]
    if let Some(influx) = &state.influx_manager {
        let time_range = params.range.as_deref().unwrap_or("1h");

        // Parse time range or use custom start/end
        let (start_time, end_time) = if let (Some(start), Some(end)) = (&params.start, &params.end)
        {
            match (
                chrono::DateTime::parse_from_rfc3339(start),
                chrono::DateTime::parse_from_rfc3339(end),
            ) {
                (Ok(s), Ok(e)) => (s.with_timezone(&chrono::Utc), e.with_timezone(&chrono::Utc)),
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        "Invalid date format. Use ISO 8601.",
                    )
                        .into_response()
                }
            }
        } else {
            let end = chrono::Utc::now();
            let start = match time_range {
                "5m" => end - chrono::Duration::minutes(5),
                "10m" => end - chrono::Duration::minutes(10),
                "1h" => end - chrono::Duration::hours(1),
                "6h" => end - chrono::Duration::hours(6),
                "24h" => end - chrono::Duration::days(1),
                "7d" => end - chrono::Duration::days(7),
                "30d" => end - chrono::Duration::days(30),
                _ => end - chrono::Duration::hours(1), // default to 1h
            };
            (start, end)
        };

        // Query historical metrics from InfluxDB
        match query_historical_metrics(influx, start_time, end_time).await {
            Ok(data) => return Json(data).into_response(),
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {e}")).into_response();
            }
        }
    }

    (StatusCode::NOT_IMPLEMENTED, "InfluxDB not configured").into_response()
}

/// History data point
#[derive(Serialize)]
struct HistoryPoint {
    timestamp: i64,
    value: f64,
}

/// Historical metrics response
#[derive(Serialize)]
struct HistoricalMetrics {
    time_range: String,
    start_time: i64,
    end_time: i64,
    metrics: HistoricalData,
}

/// Historical data structure
#[derive(Serialize)]
struct HistoricalData {
    request_rate: Vec<TimeSeriesPoint>,
    error_rate: Vec<TimeSeriesPoint>,
    response_time: Vec<TimeSeriesPoint>,
    cpu_usage: Vec<TimeSeriesPoint>,
    memory_usage: Vec<TimeSeriesPoint>,
    active_devices: Vec<TimeSeriesPoint>,
    system_health: Vec<TimeSeriesPoint>,
    temperature_by_room: std::collections::HashMap<String, Vec<TimeSeriesPoint>>,
}

/// Time series data point
#[derive(Serialize)]
struct TimeSeriesPoint {
    timestamp: i64,
    value: f64,
}

/// Query historical metrics from InfluxDB
#[cfg(feature = "influxdb")]
async fn query_historical_metrics(
    _influx: &InfluxManager,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
) -> Result<HistoricalMetrics, Box<dyn std::error::Error + Send + Sync>> {
    use std::collections::HashMap;

    // This is a simplified implementation - in a real scenario, you'd query InfluxDB
    // For now, we'll return empty data with proper structure

    let metrics = HistoricalData {
        request_rate: vec![],
        error_rate: vec![],
        response_time: vec![],
        cpu_usage: vec![],
        memory_usage: vec![],
        active_devices: vec![],
        system_health: vec![],
        temperature_by_room: HashMap::new(),
    };

    Ok(HistoricalMetrics {
        time_range: format!(
            "{} to {}",
            start_time.format("%Y-%m-%d %H:%M:%S"),
            end_time.format("%Y-%m-%d %H:%M:%S")
        ),
        start_time: start_time.timestamp_millis(),
        end_time: end_time.timestamp_millis(),
        metrics,
    })
}

/// Dashboard data structure
#[derive(Serialize)]
struct DashboardData {
    request_rate: f64,
    error_rate: f64,
    avg_response_time: f64,
    active_connections: u32,
    cpu_usage: f64,
    memory_usage: f64,
    uptime_seconds: u64,
    rate_limit_rejections: u64,
    total_requests: f64,
    error_requests: f64,
    // Loxone-specific metrics
    loxone_active_devices: f64,
    loxone_device_power_cycles: f64,
    loxone_system_health: f64,
    loxone_room_temperatures: Vec<RoomTemperature>,
    loxone_device_on_time: f64,
}

/// Request rate tracking for calculating actual rates
static mut LAST_REQUEST_COUNT: f64 = 0.0;
static mut LAST_UPDATE_TIME: Option<std::time::Instant> = None;

/// Room temperature data for dashboard
#[derive(Serialize)]
struct RoomTemperature {
    room: String,
    temperature: f64,
}

/// Parse Prometheus metrics for dashboard
fn parse_metrics_for_dashboard(prometheus_text: &str) -> DashboardData {
    let mut data = DashboardData {
        request_rate: 0.0,
        error_rate: 0.0,
        avg_response_time: 0.0,
        active_connections: 0,
        cpu_usage: 0.0,
        memory_usage: 0.0,
        uptime_seconds: 0,
        rate_limit_rejections: 0,
        total_requests: 0.0,
        error_requests: 0.0,
        // Initialize Loxone metrics
        loxone_active_devices: 0.0,
        loxone_device_power_cycles: 0.0,
        loxone_system_health: 0.0,
        loxone_room_temperatures: Vec::new(),
        loxone_device_on_time: 0.0,
    };

    // Debug: log the metrics being parsed
    tracing::debug!(
        "Parsing metrics for dashboard. Text length: {}",
        prometheus_text.len()
    );

    let mut request_count = 0.0;
    let mut response_time_sum = 0.0;

    // Simple parsing of Prometheus format
    for line in prometheus_text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let metric_name = parts[0].split('{').next().unwrap_or(parts[0]);
            let value = parts.last().unwrap_or(&"0").parse::<f64>().unwrap_or(0.0);

            // Debug: log found metrics
            if metric_name.starts_with("loxone_")
                || metric_name.starts_with("mcp_")
                || metric_name.starts_with("system_")
            {
                tracing::debug!("Found metric: {} = {}", metric_name, value);
            }

            match metric_name {
                "mcp_requests_total" => {
                    data.total_requests = value;
                    // Calculate actual request rate (requests per minute)
                    unsafe {
                        let now = std::time::Instant::now();
                        if let Some(last_time) = LAST_UPDATE_TIME {
                            let time_diff = now.duration_since(last_time).as_secs_f64();
                            if time_diff > 0.0 {
                                let request_diff = value - LAST_REQUEST_COUNT;
                                data.request_rate = (request_diff / time_diff) * 60.0;
                                // per minute
                            }
                        }
                        LAST_REQUEST_COUNT = value;
                        LAST_UPDATE_TIME = Some(now);
                    }
                }
                "mcp_requests_by_status_5xx" => data.error_requests = value,
                "mcp_request_duration_ms_sum" => response_time_sum = value,
                "mcp_request_duration_ms_count" => request_count = value,
                "system_cpu_usage_percent" => data.cpu_usage = value,
                "system_memory_usage_mb" => data.memory_usage = value,
                "process_uptime_seconds" => data.uptime_seconds = value as u64,
                "rate_limit_rejections_total" => data.rate_limit_rejections = value as u64,
                // Loxone-specific metrics
                "loxone_active_devices" => data.loxone_active_devices = value,
                "loxone_device_power_cycles_total" => data.loxone_device_power_cycles = value,
                "loxone_system_health_score" => data.loxone_system_health = value,
                "loxone_device_on_time_seconds" => data.loxone_device_on_time = value,
                _ => {
                    // Parse room temperature metrics
                    if metric_name.starts_with("loxone_room_temperature_") {
                        let room_name = metric_name
                            .strip_prefix("loxone_room_temperature_")
                            .unwrap_or("Unknown")
                            .replace('_', " ");
                        data.loxone_room_temperatures.push(RoomTemperature {
                            room: room_name,
                            temperature: value,
                        });
                    }
                }
            }
        }
    }

    // Calculate averages and rates
    if request_count > 0.0 {
        data.avg_response_time = response_time_sum / request_count;
    }

    // Calculate error rate as percentage
    if data.total_requests > 0.0 {
        data.error_rate = (data.error_requests / data.total_requests) * 100.0;
    }

    data
}

/// Embedded dashboard HTML
const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loxone MCP Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        .chart-container {
            position: relative;
            height: 300px;
            margin: 20px 0;
        }
    </style>
</head>
<body class="bg-gray-100">
    <div class="container mx-auto px-4 py-8">
        <div class="flex justify-between items-center mb-8">
            <h1 class="text-3xl font-bold text-gray-800">Loxone MCP Monitoring Dashboard</h1>

            <!-- Time Range Controls -->
            <div class="flex items-center space-x-4">
                <label class="text-sm font-medium text-gray-700">Time Range:</label>
                <select id="timeRange" class="border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent">
                    <option value="live">Live</option>
                    <option value="5m">Last 5 minutes</option>
                    <option value="10m">Last 10 minutes</option>
                    <option value="1h" selected>Last 1 hour</option>
                    <option value="6h">Last 6 hours</option>
                    <option value="24h">Last 24 hours</option>
                    <option value="7d">Last 7 days</option>
                    <option value="30d">Last 30 days</option>
                    <option value="custom">Custom Range</option>
                </select>

                <!-- Custom Range Inputs (hidden by default) -->
                <div id="customRangeInputs" class="hidden flex items-center space-x-2">
                    <input type="datetime-local" id="startTime" class="border border-gray-300 rounded-md px-2 py-1 text-sm">
                    <span class="text-gray-500">to</span>
                    <input type="datetime-local" id="endTime" class="border border-gray-300 rounded-md px-2 py-1 text-sm">
                </div>

                <button id="refreshBtn" class="bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                    Refresh
                </button>

                <button id="pauseBtn" class="bg-gray-500 hover:bg-gray-600 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                    Pause
                </button>
            </div>
        </div>

        <!-- MCP Server Status Cards -->
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-gray-500">Request Rate</h3>
                <p class="text-2xl font-bold text-gray-900" id="request-rate">0</p>
                <p class="text-sm text-gray-600">requests/min</p>
            </div>
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-gray-500">Error Rate</h3>
                <p class="text-2xl font-bold text-red-600" id="error-rate">0%</p>
                <p class="text-sm text-gray-600">5xx errors</p>
            </div>
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-gray-500">Avg Response Time</h3>
                <p class="text-2xl font-bold text-gray-900" id="response-time">0</p>
                <p class="text-sm text-gray-600">ms</p>
            </div>
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-gray-500">Uptime</h3>
                <p class="text-2xl font-bold text-green-600" id="uptime">0h</p>
                <p class="text-sm text-gray-600">hours</p>
            </div>
        </div>

        <!-- Loxone Status Cards -->
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
            <div class="bg-blue-50 rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-blue-600">Active Devices</h3>
                <p class="text-2xl font-bold text-blue-900" id="loxone-active-devices">0</p>
                <p class="text-sm text-blue-600">currently on</p>
            </div>
            <div class="bg-green-50 rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-green-600">System Health</h3>
                <p class="text-2xl font-bold text-green-900" id="loxone-health">0</p>
                <p class="text-sm text-green-600">score (0-100)</p>
            </div>
            <div class="bg-purple-50 rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-purple-600">Power Cycles</h3>
                <p class="text-2xl font-bold text-purple-900" id="loxone-power-cycles">0</p>
                <p class="text-sm text-purple-600">total today</p>
            </div>
            <div class="bg-orange-50 rounded-lg shadow p-6">
                <h3 class="text-sm font-medium text-orange-600">Device Runtime</h3>
                <p class="text-2xl font-bold text-orange-900" id="loxone-device-time">0h</p>
                <p class="text-sm text-orange-600">total on time</p>
            </div>
        </div>

        <!-- Charts -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
            <!-- Request Rate Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Request Rate</h3>
                <div class="chart-container">
                    <canvas id="requestChart"></canvas>
                </div>
            </div>

            <!-- Response Time Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Response Times</h3>
                <div class="chart-container">
                    <canvas id="responseChart"></canvas>
                </div>
            </div>

            <!-- System Resources Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">System Resources</h3>
                <div class="chart-container">
                    <canvas id="resourceChart"></canvas>
                </div>
            </div>

            <!-- Error Rate Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Error Rate</h3>
                <div class="chart-container">
                    <canvas id="errorChart"></canvas>
                </div>
            </div>
        </div>

        <!-- Loxone Charts -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <!-- Device Activity Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Device Activity</h3>
                <div class="chart-container">
                    <canvas id="deviceActivityChart"></canvas>
                </div>
            </div>

            <!-- Room Temperatures Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Room Temperatures</h3>
                <div class="chart-container">
                    <canvas id="temperatureChart"></canvas>
                </div>
            </div>

            <!-- System Health Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">System Health</h3>
                <div class="chart-container">
                    <canvas id="healthChart"></canvas>
                </div>
            </div>

            <!-- Energy Usage Chart -->
            <div class="bg-white rounded-lg shadow p-6">
                <h3 class="text-lg font-semibold text-gray-800 mb-4">Device Runtime</h3>
                <div class="chart-container">
                    <canvas id="runtimeChart"></canvas>
                </div>
            </div>
        </div>

        <!-- Connection Status -->
        <div class="mt-8 text-center">
            <span id="connection-status" class="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-green-100 text-green-800">
                <span class="w-2 h-2 bg-green-400 rounded-full mr-2"></span>
                Connected
            </span>
        </div>
    </div>

    <script>
        // Chart configuration
        const chartOptions = {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
                y: {
                    beginAtZero: true
                }
            },
            plugins: {
                legend: {
                    display: false
                }
            }
        };

        // Initialize charts
        const requestChart = new Chart(document.getElementById('requestChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Requests/min',
                    data: [],
                    borderColor: 'rgb(59, 130, 246)',
                    backgroundColor: 'rgba(59, 130, 246, 0.1)',
                    tension: 0.4
                }]
            },
            options: chartOptions
        });

        const responseChart = new Chart(document.getElementById('responseChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Response Time (ms)',
                    data: [],
                    borderColor: 'rgb(34, 197, 94)',
                    backgroundColor: 'rgba(34, 197, 94, 0.1)',
                    tension: 0.4
                }]
            },
            options: chartOptions
        });

        const resourceChart = new Chart(document.getElementById('resourceChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'CPU %',
                    data: [],
                    borderColor: 'rgb(168, 85, 247)',
                    backgroundColor: 'rgba(168, 85, 247, 0.1)',
                    tension: 0.4
                }, {
                    label: 'Memory MB',
                    data: [],
                    borderColor: 'rgb(251, 146, 60)',
                    backgroundColor: 'rgba(251, 146, 60, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                ...chartOptions,
                plugins: {
                    legend: {
                        display: true
                    }
                }
            }
        });

        const errorChart = new Chart(document.getElementById('errorChart'), {
            type: 'bar',
            data: {
                labels: [],
                datasets: [{
                    label: 'Errors',
                    data: [],
                    backgroundColor: 'rgba(239, 68, 68, 0.5)',
                    borderColor: 'rgb(239, 68, 68)',
                    borderWidth: 1
                }]
            },
            options: chartOptions
        });

        // Initialize Loxone charts
        const deviceActivityChart = new Chart(document.getElementById('deviceActivityChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Active Devices',
                    data: [],
                    borderColor: 'rgb(59, 130, 246)',
                    backgroundColor: 'rgba(59, 130, 246, 0.1)',
                    tension: 0.4
                }]
            },
            options: chartOptions
        });

        const temperatureChart = new Chart(document.getElementById('temperatureChart'), {
            type: 'bar',
            data: {
                labels: [],
                datasets: [{
                    label: 'Temperature (°C)',
                    data: [],
                    backgroundColor: 'rgba(34, 197, 94, 0.5)',
                    borderColor: 'rgb(34, 197, 94)',
                    borderWidth: 1
                }]
            },
            options: {
                ...chartOptions,
                scales: {
                    y: {
                        beginAtZero: false,
                        min: 15,
                        max: 30
                    }
                }
            }
        });

        const healthChart = new Chart(document.getElementById('healthChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Health Score',
                    data: [],
                    borderColor: 'rgb(34, 197, 94)',
                    backgroundColor: 'rgba(34, 197, 94, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                ...chartOptions,
                scales: {
                    y: {
                        beginAtZero: true,
                        max: 100
                    }
                }
            }
        });

        const runtimeChart = new Chart(document.getElementById('runtimeChart'), {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Runtime (hours)',
                    data: [],
                    borderColor: 'rgb(251, 146, 60)',
                    backgroundColor: 'rgba(251, 146, 60, 0.1)',
                    tension: 0.4
                }]
            },
            options: chartOptions
        });

        // Data storage
        const maxDataPoints = 60;
        let requestData = [];
        let responseData = [];
        let cpuData = [];
        let memoryData = [];
        let errorData = [];
        let labels = [];
        // Loxone data
        let deviceActivityData = [];
        let systemHealthData = [];
        let deviceRuntimeData = [];
        let temperatureData = {};

        // Update charts with new data
        function updateCharts(data) {
            const now = new Date().toLocaleTimeString();

            // Update labels
            labels.push(now);
            if (labels.length > maxDataPoints) {
                labels.shift();
            }

            // Update data arrays
            requestData.push(data.request_rate);
            responseData.push(data.avg_response_time);
            cpuData.push(data.cpu_usage);
            memoryData.push(data.memory_usage);
            errorData.push(data.error_rate);

            // Update Loxone data arrays
            deviceActivityData.push(data.loxone_active_devices);
            systemHealthData.push(data.loxone_system_health);
            deviceRuntimeData.push(data.loxone_device_on_time / 3600); // Convert to hours

            // Keep only last maxDataPoints
            [requestData, responseData, cpuData, memoryData, errorData,
             deviceActivityData, systemHealthData, deviceRuntimeData].forEach(arr => {
                if (arr.length > maxDataPoints) {
                    arr.shift();
                }
            });

            // Update charts
            requestChart.data.labels = labels;
            requestChart.data.datasets[0].data = requestData;
            requestChart.update('none');

            responseChart.data.labels = labels;
            responseChart.data.datasets[0].data = responseData;
            responseChart.update('none');

            resourceChart.data.labels = labels;
            resourceChart.data.datasets[0].data = cpuData;
            resourceChart.data.datasets[1].data = memoryData;
            resourceChart.update('none');

            errorChart.data.labels = labels.slice(-10); // Last 10 for bar chart
            errorChart.data.datasets[0].data = errorData.slice(-10);
            errorChart.update('none');

            // Update Loxone charts
            deviceActivityChart.data.labels = labels;
            deviceActivityChart.data.datasets[0].data = deviceActivityData;
            deviceActivityChart.update('none');

            healthChart.data.labels = labels;
            healthChart.data.datasets[0].data = systemHealthData;
            healthChart.update('none');

            runtimeChart.data.labels = labels;
            runtimeChart.data.datasets[0].data = deviceRuntimeData;
            runtimeChart.update('none');

            // Update temperature chart with room data
            if (data.loxone_room_temperatures && data.loxone_room_temperatures.length > 0) {
                const roomLabels = data.loxone_room_temperatures.map(room => room.room);
                const roomTemps = data.loxone_room_temperatures.map(room => room.temperature);

                temperatureChart.data.labels = roomLabels;
                temperatureChart.data.datasets[0].data = roomTemps;
                temperatureChart.update('none');
            }

            // Update status cards
            document.getElementById('request-rate').textContent = data.request_rate.toFixed(0);
            document.getElementById('error-rate').textContent = (data.error_rate > 0 ? data.error_rate.toFixed(1) : 0) + '%';
            document.getElementById('response-time').textContent = data.avg_response_time.toFixed(1);
            document.getElementById('uptime').textContent = (data.uptime_seconds / 3600).toFixed(1) + 'h';

            // Update Loxone status cards
            document.getElementById('loxone-active-devices').textContent = data.loxone_active_devices.toFixed(0);
            document.getElementById('loxone-health').textContent = data.loxone_system_health.toFixed(0);
            document.getElementById('loxone-power-cycles').textContent = data.loxone_device_power_cycles.toFixed(0);
            document.getElementById('loxone-device-time').textContent = (data.loxone_device_on_time / 3600).toFixed(1) + 'h';
        }

        // Time range management
        let currentMode = 'live';
        let isLiveMode = true;
        let isPaused = false;
        let eventSource = null;

        // Time range controls
        const timeRangeSelect = document.getElementById('timeRange');
        const customRangeInputs = document.getElementById('customRangeInputs');
        const startTimeInput = document.getElementById('startTime');
        const endTimeInput = document.getElementById('endTime');
        const refreshBtn = document.getElementById('refreshBtn');
        const pauseBtn = document.getElementById('pauseBtn');

        // Handle time range selection
        timeRangeSelect.addEventListener('change', function() {
            const selectedRange = this.value;

            if (selectedRange === 'custom') {
                customRangeInputs.classList.remove('hidden');
                isLiveMode = false;
                if (eventSource) {
                    eventSource.close();
                    eventSource = null;
                }
            } else {
                customRangeInputs.classList.add('hidden');

                if (selectedRange === 'live') {
                    isLiveMode = true;
                    if (!isPaused) {
                        startLiveMode();
                    }
                } else {
                    isLiveMode = false;
                    if (eventSource) {
                        eventSource.close();
                        eventSource = null;
                    }
                    loadHistoricalData(selectedRange);
                }
            }
            currentMode = selectedRange;
        });

        // Handle refresh button
        refreshBtn.addEventListener('click', function() {
            if (currentMode === 'live' && !isPaused) {
                // In live mode, restart the connection
                if (eventSource) {
                    eventSource.close();
                }
                startLiveMode();
            } else if (currentMode === 'custom') {
                // Load custom range data
                const start = startTimeInput.value;
                const end = endTimeInput.value;
                if (start && end) {
                    loadCustomRangeData(start, end);
                }
            } else {
                // Load historical data for selected range
                loadHistoricalData(currentMode);
            }
        });

        // Handle pause/resume button
        pauseBtn.addEventListener('click', function() {
            isPaused = !isPaused;

            if (isPaused) {
                if (eventSource) {
                    eventSource.close();
                    eventSource = null;
                }
                pauseBtn.textContent = 'Resume';
                pauseBtn.classList.remove('bg-gray-500', 'hover:bg-gray-600');
                pauseBtn.classList.add('bg-green-500', 'hover:bg-green-600');
            } else {
                pauseBtn.textContent = 'Pause';
                pauseBtn.classList.remove('bg-green-500', 'hover:bg-green-600');
                pauseBtn.classList.add('bg-gray-500', 'hover:bg-gray-600');

                if (isLiveMode) {
                    startLiveMode();
                }
            }
        });

        // Start live mode
        function startLiveMode() {
            if (eventSource) return; // Already connected

            eventSource = new EventSource('/dashboard/metrics/live');

            eventSource.onmessage = function(event) {
                try {
                    const data = JSON.parse(event.data);
                    updateCharts(data);
                } catch (e) {
                    console.error('Failed to parse metrics:', e);
                }
            };

            eventSource.onerror = function(error) {
                console.error('SSE error:', error);
                updateConnectionStatus(false);
            };

            eventSource.onopen = function() {
                updateConnectionStatus(true);
            };
        }

        // Load historical data
        async function loadHistoricalData(range) {
            try {
                const response = await fetch(`/dashboard/api/historical?range=${range}`);
                const historicalData = await response.json();

                // Clear current chart data
                clearChartData();

                // Populate charts with historical data
                updateChartsWithHistoricalData(historicalData);

            } catch (error) {
                console.error('Error loading historical data:', error);
            }
        }

        // Load custom range data
        async function loadCustomRangeData(start, end) {
            try {
                const startISO = new Date(start).toISOString();
                const endISO = new Date(end).toISOString();

                const response = await fetch(`/dashboard/api/historical?start=${startISO}&end=${endISO}`);
                const historicalData = await response.json();

                // Clear current chart data
                clearChartData();

                // Populate charts with historical data
                updateChartsWithHistoricalData(historicalData);

            } catch (error) {
                console.error('Error loading custom range data:', error);
            }
        }

        // Clear chart data
        function clearChartData() {
            requestData = [];
            responseData = [];
            cpuData = [];
            memoryData = [];
            errorData = [];
            deviceActivityData = [];
            systemHealthData = [];
            deviceRuntimeData = [];
            labels = [];
            temperatureData = {};
        }

        // Update charts with historical data
        function updateChartsWithHistoricalData(historicalData) {
            const metrics = historicalData.metrics;

            // Convert historical data to chart format
            if (metrics.request_rate.length > 0) {
                labels = metrics.request_rate.map(point =>
                    new Date(point.timestamp).toLocaleTimeString()
                );
                requestData = metrics.request_rate.map(point => point.value);
                responseData = metrics.response_time.map(point => point.value);
                cpuData = metrics.cpu_usage.map(point => point.value);
                memoryData = metrics.memory_usage.map(point => point.value);
                errorData = metrics.error_rate.map(point => point.value);
                deviceActivityData = metrics.active_devices.map(point => point.value);
                systemHealthData = metrics.system_health.map(point => point.value);
            }

            // Update all charts
            updateChartDisplay();
        }

        // Update chart display
        function updateChartDisplay() {
            requestChart.data.labels = labels;
            requestChart.data.datasets[0].data = requestData;
            requestChart.update();

            responseChart.data.labels = labels;
            responseChart.data.datasets[0].data = responseData;
            responseChart.update();

            resourceChart.data.labels = labels;
            resourceChart.data.datasets[0].data = cpuData;
            resourceChart.data.datasets[1].data = memoryData;
            resourceChart.update();

            errorChart.data.labels = labels.slice(-10);
            errorChart.data.datasets[0].data = errorData.slice(-10);
            errorChart.update();

            deviceActivityChart.data.labels = labels;
            deviceActivityChart.data.datasets[0].data = deviceActivityData;
            deviceActivityChart.update();

            healthChart.data.labels = labels;
            healthChart.data.datasets[0].data = systemHealthData;
            healthChart.update();

            runtimeChart.data.labels = labels;
            runtimeChart.data.datasets[0].data = deviceRuntimeData;
            runtimeChart.update();
        }

        // Update connection status
        function updateConnectionStatus(connected) {
            const statusElement = document.getElementById('connection-status');
            if (connected) {
                statusElement.innerHTML = `
                    <span class="w-2 h-2 bg-green-400 rounded-full mr-2"></span>
                    Connected
                `;
                statusElement.className =
                    'inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-green-100 text-green-800';
            } else {
                statusElement.innerHTML = `
                    <span class="w-2 h-2 bg-red-400 rounded-full mr-2"></span>
                    Disconnected
                `;
                statusElement.className =
                    'inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-red-100 text-red-800';
            }
        }

        // Start in live mode by default
        startLiveMode();
    </script>
</body>
</html>
"#;

use axum::Json;
