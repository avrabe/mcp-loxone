//! Unified historical data storage system
//!
//! This module provides a consolidated approach to storing and retrieving
//! historical data across the Loxone MCP server, replacing multiple separate
//! implementations with a single, efficient system.

pub mod cold_storage;
pub mod compat;
pub mod config;
pub mod core;
pub mod dashboard_api;
pub mod dynamic_dashboard;
pub mod events;
pub mod hot_storage;
pub mod query;
pub mod tiering;

pub use config::HistoryConfig;
pub use core::UnifiedHistoryStore;
pub use dashboard_api::create_dashboard_router;
pub use dynamic_dashboard::{DynamicDashboard, DynamicDashboardConfig, DynamicDashboardLayout};
pub use events::{EventCategory, EventData, EventSource, HistoricalEvent};
pub use query::{QueryBuilder, QueryFilters, QueryResult};

/// Initialize the history subsystem
pub async fn init(config: HistoryConfig) -> crate::error::Result<UnifiedHistoryStore> {
    tracing::info!("Initializing unified history store");

    let store = UnifiedHistoryStore::new(config).await?;

    tracing::info!("Unified history store initialized successfully");
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_history_store_creation() {
        let temp_dir = std::env::temp_dir().join("loxone_test_history");
        let mut config = HistoryConfig::default();
        config.cold_storage.data_dir = temp_dir;
        let store = init(config).await.unwrap();

        // Test basic event recording
        let event = HistoricalEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: EventCategory::SystemMetric(events::MetricData {
                metric_name: "test_metric".to_string(),
                value: 42.0,
                unit: "count".to_string(),
                tags: Default::default(),
            }),
            source: EventSource::System,
            data: EventData::Generic(serde_json::json!({"test": true})),
            metadata: Default::default(),
        };

        assert!(store.record(event).await.is_ok());
    }
}
