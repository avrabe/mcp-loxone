# Loxone Statistics Collection System

This document describes the unified statistics collection system for Loxone that integrates with the existing monitoring infrastructure.

## Overview

The Loxone statistics system provides comprehensive monitoring and analytics for your Loxone home automation system, including:

- **Device Usage Statistics**: Track device on/off cycles, runtime, and energy consumption
- **Room Climate Monitoring**: Temperature, humidity, and comfort analysis 
- **System Health Monitoring**: Overall system performance and health scoring
- **Historical Data Storage**: Long-term trend analysis via InfluxDB integration
- **Real-time Dashboard**: Live visualization of all metrics

## Features

### Device Monitoring
- Track power cycles and total runtime for all controllable devices
- Monitor device state changes and activity patterns
- Energy consumption tracking (when available from Loxone energy meters)
- Per-room and per-device-type breakdowns

### Climate Analytics
- Real-time temperature and humidity monitoring
- Comfort index calculation based on ideal ranges
- Historical climate trends and analysis
- Room-by-room climate statistics

### System Health
- Overall system health score (0-100)
- Detection of devices that are always on or unresponsive
- Climate comfort analysis
- Performance and reliability monitoring

### Automation Statistics
- Track automation trigger counts and success rates
- Monitor execution times and performance
- Common trigger pattern analysis

## Setup

### 1. Enable InfluxDB Integration (Optional but Recommended)

First, set up InfluxDB for historical data storage:

```bash
# InfluxDB configuration
export INFLUXDB_TOKEN="your-influxdb-token"
export INFLUXDB_URL="http://localhost:8086"
export INFLUXDB_ORG="loxone-mcp"
export INFLUXDB_BUCKET="loxone_metrics"
```

### 2. Enable Loxone Statistics Collection

```bash
# Enable Loxone statistics collection
export ENABLE_LOXONE_STATS=1

# Optional: Customize collection interval (default: 60 seconds)
export LOXONE_STATS_INTERVAL=60
```

### 3. Start the Server

```bash
# Start with InfluxDB features enabled
cargo run --features=influxdb --bin loxone-mcp-server -- http

# Or build with all features
cargo build --release --features=influxdb
./target/release/loxone-mcp-server http --port 3001
```

## Dashboard Access

Once enabled, you can access the enhanced dashboard at:

```
http://localhost:3001/dashboard/
```

The dashboard includes:

### MCP Server Metrics
- Request rate and response times
- Error rates and system resources
- Rate limiting statistics

### Loxone-Specific Metrics
- **Active Devices**: Currently powered on devices
- **System Health**: Overall health score (0-100)
- **Power Cycles**: Total device activation count
- **Device Runtime**: Total time devices have been on

### Real-time Charts
- **Device Activity**: Active device count over time
- **Room Temperatures**: Current temperature by room
- **System Health**: Health score trends
- **Device Runtime**: Cumulative runtime statistics

## Metrics Collected

### Device Metrics
- `loxone_active_devices`: Number of currently active devices
- `loxone_device_power_cycles_total`: Total device power cycles
- `loxone_device_on_time_seconds`: Total device runtime in seconds
- `loxone_room_temperature_{room_name}`: Temperature per room
- `loxone_room_humidity_{room_name}`: Humidity per room (if available)

### System Metrics
- `loxone_system_health_score`: Overall system health (0-100)
- `loxone_automation_triggers_total`: Automation execution count
- `loxone_energy_consumption_kwh`: Energy consumption (if available)
- `loxone_current_power_w`: Current power usage (if available)

### Comfort Metrics
- `loxone_room_comfort_index`: Comfort score per room (0-100)

## InfluxDB Data Structure

Data is stored in InfluxDB with the following structure:

### Device States
```
measurement: device_state
tags: uuid, name, type, room
fields: state, value
timestamp: collection_time
```

### Sensor Data
```
measurement: sensor_{type}
tags: uuid, name, type, room
fields: value, unit
timestamp: collection_time
```

### MCP Metrics
```
measurement: mcp_metrics
fields: total_requests, failed_requests, cpu_usage, memory_usage_mb, avg_response_time_ms
timestamp: collection_time
```

## Querying Historical Data

### Example InfluxDB Queries

Get device activity over the last 24 hours:
```flux
from(bucket: "loxone_metrics")
  |> range(start: -24h)
  |> filter(fn: (r) => r["_measurement"] == "device_state")
  |> filter(fn: (r) => r["_field"] == "value")
  |> group(columns: ["room"])
```

Get room temperature trends:
```flux
from(bucket: "loxone_metrics")
  |> range(start: -7d)
  |> filter(fn: (r) => r["_measurement"] =~ /sensor_temperature/)
  |> filter(fn: (r) => r["_field"] == "value")
  |> aggregateWindow(every: 1h, fn: mean)
```

## Configuration Options

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_LOXONE_STATS` | disabled | Enable Loxone statistics collection |
| `LOXONE_STATS_INTERVAL` | 60 | Collection interval in seconds |
| `INFLUXDB_TOKEN` | - | InfluxDB authentication token |
| `INFLUXDB_URL` | http://localhost:8086 | InfluxDB server URL |
| `INFLUXDB_ORG` | loxone-mcp | InfluxDB organization |
| `INFLUXDB_BUCKET` | loxone_metrics | InfluxDB bucket name |

### System Health Calculation

The system health score is calculated based on:
- **Device Health** (40%): Devices functioning normally, not stuck on/off
- **Climate Comfort** (30%): Rooms within comfortable temperature/humidity ranges
- **System Performance** (20%): Response times and error rates
- **Automation Success** (10%): Automation execution success rate

Score ranges:
- 90-100: Excellent
- 80-89: Good  
- 70-79: Fair
- 60-69: Poor
- <60: Critical

## Troubleshooting

### Statistics Not Collecting
1. Check that `ENABLE_LOXONE_STATS=1` is set
2. Verify Loxone connection is working
3. Check server logs for initialization errors

### InfluxDB Connection Issues
1. Verify InfluxDB is running and accessible
2. Check token permissions and bucket existence
3. Review InfluxDB configuration variables

### Dashboard Not Showing Loxone Data
1. Ensure InfluxDB feature is enabled during build
2. Check that statistics collection is active
3. Wait for initial data collection (up to 1 minute)

### Missing Device Data
1. Verify devices are properly configured in Loxone
2. Check that devices are controllable (lights, switches, blinds)
3. Review device type detection in logs

## Performance Considerations

- Collection runs every 60 seconds by default
- Data is buffered and batch-written to InfluxDB
- Dashboard updates in real-time via Server-Sent Events
- Historical data retention is configurable in InfluxDB
- System impact is minimal (~1-2% CPU during collection)

## Integration with Existing Tools

The statistics system integrates seamlessly with:
- **Prometheus**: Metrics are exported in Prometheus format
- **Grafana**: Can be used for advanced visualization
- **InfluxDB**: Primary time-series storage
- **MCP Tools**: Statistics enhance existing MCP tool functionality

## Development

To extend the statistics system:

1. **Add New Metrics**: Extend `LoxoneStats` struct in `loxone_stats.rs`
2. **Custom Collectors**: Implement additional data collection methods
3. **Dashboard Widgets**: Add new charts to the HTML dashboard
4. **InfluxDB Schemas**: Define new measurement schemas as needed

See the source code in `src/monitoring/loxone_stats.rs` for implementation details.