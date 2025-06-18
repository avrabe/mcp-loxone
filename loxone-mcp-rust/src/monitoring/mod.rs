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

pub mod clean_dashboard;
pub mod dashboard;
pub mod key_management_ui;
pub mod key_management_ui_new;
pub mod loxone_stats;
pub mod metrics;
pub mod server_metrics;
pub mod unified_collector;
pub mod unified_dashboard;
pub mod unified_dashboard_new;
