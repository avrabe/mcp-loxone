//! Monitoring and time series data management
//!
//! This module provides comprehensive monitoring capabilities including:
//! - InfluxDB integration for historical data storage
//! - Real-time metrics collection
//! - Embedded dashboard with charts
//! - Prometheus-compatible exports
//! - Loxone-specific statistics collection

#[cfg(feature = "influxdb")]
pub mod influxdb;

pub mod dashboard;
pub mod loxone_stats;
pub mod metrics;
pub mod unified_collector;
pub mod unified_dashboard;
