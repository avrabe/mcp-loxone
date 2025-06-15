//! Query interface for historical data

use super::cold_storage::ColdDataStore;
use super::events::*;
use super::hot_storage::HotDataStore;
use crate::error::Result;
use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Query builder for historical data
pub struct QueryBuilder {
    hot_store: Arc<RwLock<HotDataStore>>,
    cold_store: Arc<ColdDataStore>,
    filters: QueryFilters,
}

/// Query filters
#[derive(Debug, Default, Clone)]
pub struct QueryFilters {
    /// Start time (inclusive)
    pub start_time: Option<DateTime<Utc>>,

    /// End time (inclusive)
    pub end_time: Option<DateTime<Utc>>,

    /// Event categories to include
    pub categories: Option<Vec<String>>,

    /// Event sources to include
    pub sources: Option<Vec<String>>,

    /// Specific device/sensor UUIDs
    pub entity_ids: Option<Vec<String>>,

    /// Room filter
    pub rooms: Option<Vec<String>>,

    /// Maximum number of results
    pub limit: Option<usize>,

    /// Sort order
    pub sort_order: SortOrder,

    /// Include only events with specific metadata keys
    pub metadata_keys: Option<Vec<String>>,
}

/// Sort order for results
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    /// Newest first (default)
    Descending,
    /// Oldest first
    Ascending,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Descending
    }
}

/// Query result
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    /// Matching events
    pub events: Vec<HistoricalEvent>,

    /// Total count (before limit)
    pub total_count: usize,

    /// Query execution time in milliseconds
    pub query_time_ms: u64,

    /// Whether results came from hot storage only
    pub from_hot_storage: bool,
}

impl QueryBuilder {
    /// Create new query builder
    pub fn new(hot_store: Arc<RwLock<HotDataStore>>, cold_store: Arc<ColdDataStore>) -> Self {
        Self {
            hot_store,
            cold_store,
            filters: QueryFilters::default(),
        }
    }

    /// Set time range
    pub fn time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.filters.start_time = Some(start);
        self.filters.end_time = Some(end);
        self
    }

    /// Set start time
    pub fn since(mut self, start: DateTime<Utc>) -> Self {
        self.filters.start_time = Some(start);
        self
    }

    /// Set end time
    pub fn until(mut self, end: DateTime<Utc>) -> Self {
        self.filters.end_time = Some(end);
        self
    }

    /// Filter by category
    pub fn category(mut self, category: impl Into<String>) -> Self {
        let cat = category.into();
        match &mut self.filters.categories {
            Some(cats) => cats.push(cat),
            None => self.filters.categories = Some(vec![cat]),
        }
        self
    }

    /// Filter by multiple categories
    pub fn categories(mut self, categories: Vec<String>) -> Self {
        self.filters.categories = Some(categories);
        self
    }

    /// Filter by source type
    pub fn source_type(mut self, source: EventSource) -> Self {
        let source_str = match source {
            EventSource::Device(_) => "device",
            EventSource::Sensor(_) => "sensor",
            EventSource::System => "system",
            EventSource::User(_) => "user",
            EventSource::Automation(_) => "automation",
            EventSource::Api(_) => "api",
        }
        .to_string();

        match &mut self.filters.sources {
            Some(sources) => sources.push(source_str),
            None => self.filters.sources = Some(vec![source_str]),
        }
        self
    }

    /// Filter by entity ID (device/sensor UUID)
    pub fn entity_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        match &mut self.filters.entity_ids {
            Some(ids) => ids.push(id),
            None => self.filters.entity_ids = Some(vec![id]),
        }
        self
    }

    /// Filter by room
    pub fn room(mut self, room: impl Into<String>) -> Self {
        let room = room.into();
        match &mut self.filters.rooms {
            Some(rooms) => rooms.push(room),
            None => self.filters.rooms = Some(vec![room]),
        }
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.filters.limit = Some(limit);
        self
    }

    /// Set sort order
    pub fn sort(mut self, order: SortOrder) -> Self {
        self.filters.sort_order = order;
        self
    }

    /// Filter by metadata key
    pub fn with_metadata(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        match &mut self.filters.metadata_keys {
            Some(keys) => keys.push(key),
            None => self.filters.metadata_keys = Some(vec![key]),
        }
        self
    }

    /// Execute the query
    pub async fn execute(self) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();
        let mut events = Vec::new();
        let mut from_hot_storage = true;

        // Determine if we need to query cold storage
        let needs_cold_storage = if let Some(start) = self.filters.start_time {
            // If querying data older than hot storage retention
            let hot_cutoff = Utc::now() - chrono::Duration::hours(1);
            start < hot_cutoff
        } else {
            false
        };

        // Query hot storage first
        events.extend(self.query_hot_storage().await?);

        // Query cold storage if needed
        if needs_cold_storage {
            debug!("Querying cold storage for historical data");
            events.extend(self.query_cold_storage().await?);
            from_hot_storage = false;
        }

        // Apply filters
        events = self.apply_filters(events);

        // Sort results
        match self.filters.sort_order {
            SortOrder::Descending => events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)),
            SortOrder::Ascending => events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
        }

        let total_count = events.len();

        // Apply limit
        if let Some(limit) = self.filters.limit {
            events.truncate(limit);
        }

        Ok(QueryResult {
            events,
            total_count,
            query_time_ms: start_time.elapsed().as_millis() as u64,
            from_hot_storage,
        })
    }

    /// Query hot storage
    async fn query_hot_storage(&self) -> Result<Vec<HistoricalEvent>> {
        let mut events = Vec::new();
        let hot_store = self.hot_store.read().await;

        // Get events by category if specified
        if let Some(ref categories) = self.filters.categories {
            for category in categories {
                events.extend(hot_store.get_events_by_category(category, None).await);
            }
        } else {
            // Get all categories
            for category in &[
                "device_state",
                "sensor_reading",
                "system_metric",
                "audit_event",
                "discovery_event",
            ] {
                events.extend(hot_store.get_events_by_category(category, None).await);
            }
        }

        Ok(events)
    }

    /// Query cold storage
    async fn query_cold_storage(&self) -> Result<Vec<HistoricalEvent>> {
        let start = self
            .filters
            .start_time
            .unwrap_or_else(|| Utc::now() - Duration::days(30));
        let end = self.filters.end_time.unwrap_or_else(Utc::now);

        Ok(self
            .cold_store
            .query_events(
                start,
                end,
                self.filters.categories.clone(),
                None, // Apply limit after merging with hot storage
            )
            .await
            .unwrap_or_else(|_| Vec::new()))
    }

    /// Apply filters to events
    fn apply_filters(&self, events: Vec<HistoricalEvent>) -> Vec<HistoricalEvent> {
        events
            .into_iter()
            .filter(|event| {
                // Time range filter
                if let Some(start) = self.filters.start_time {
                    if event.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = self.filters.end_time {
                    if event.timestamp > end {
                        return false;
                    }
                }

                // Source filter
                if let Some(ref sources) = self.filters.sources {
                    let source_type = match &event.source {
                        EventSource::Device(_) => "device",
                        EventSource::Sensor(_) => "sensor",
                        EventSource::System => "system",
                        EventSource::User(_) => "user",
                        EventSource::Automation(_) => "automation",
                        EventSource::Api(_) => "api",
                    };
                    if !sources.contains(&source_type.to_string()) {
                        return false;
                    }
                }

                // Entity ID filter
                if let Some(ref ids) = self.filters.entity_ids {
                    let entity_id = match &event.source {
                        EventSource::Device(id) | EventSource::Sensor(id) => Some(id),
                        _ => None,
                    };
                    if let Some(id) = entity_id {
                        if !ids.contains(id) {
                            return false;
                        }
                    }
                }

                // Room filter
                if let Some(ref rooms) = self.filters.rooms {
                    let event_room = match &event.category {
                        EventCategory::DeviceState(state) => state.room.as_ref(),
                        EventCategory::SensorReading(data) => data.room.as_ref(),
                        _ => None,
                    };
                    if let Some(room) = event_room {
                        if !rooms.contains(room) {
                            return false;
                        }
                    }
                }

                // Metadata filter
                if let Some(ref keys) = self.filters.metadata_keys {
                    for key in keys {
                        if !event.metadata.contains_key(key) {
                            return false;
                        }
                    }
                }

                true
            })
            .collect()
    }
}

/// Aggregation support for time-series data
#[derive(Debug, Clone, Copy)]
pub enum AggregateInterval {
    Minute,
    FiveMinutes,
    FifteenMinutes,
    Hour,
    Day,
}

/// Aggregated data point
#[derive(Debug, Serialize, Deserialize)]
pub struct AggregatePoint {
    pub timestamp: DateTime<Utc>,
    pub count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub avg: Option<f64>,
    pub sum: Option<f64>,
}

impl QueryBuilder {
    /// Get aggregated data for metrics
    pub async fn aggregate(
        self,
        metric_name: &str,
        interval: AggregateInterval,
    ) -> Result<Vec<AggregatePoint>> {
        let events = self.execute().await?.events;

        // Filter to system metrics with matching name
        let metrics: Vec<_> = events
            .into_iter()
            .filter_map(|e| match e.category {
                EventCategory::SystemMetric(ref data) if data.metric_name == metric_name => {
                    Some((e.timestamp, data.value))
                }
                _ => None,
            })
            .collect();

        // Group by interval
        let mut aggregates = std::collections::HashMap::new();

        for (timestamp, value) in metrics {
            let bucket = match interval {
                AggregateInterval::Minute => timestamp
                    .date_naive()
                    .and_hms_opt(timestamp.hour(), timestamp.minute(), 0)
                    .unwrap()
                    .and_utc(),
                AggregateInterval::FiveMinutes => {
                    let minute = (timestamp.minute() / 5) * 5;
                    timestamp
                        .date_naive()
                        .and_hms_opt(timestamp.hour(), minute, 0)
                        .unwrap()
                        .and_utc()
                }
                AggregateInterval::FifteenMinutes => {
                    let minute = (timestamp.minute() / 15) * 15;
                    timestamp
                        .date_naive()
                        .and_hms_opt(timestamp.hour(), minute, 0)
                        .unwrap()
                        .and_utc()
                }
                AggregateInterval::Hour => timestamp
                    .date_naive()
                    .and_hms_opt(timestamp.hour(), 0, 0)
                    .unwrap()
                    .and_utc(),
                AggregateInterval::Day => timestamp
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc(),
            };

            aggregates
                .entry(bucket)
                .or_insert_with(Vec::new)
                .push(value);
        }

        // Calculate aggregates
        let mut results: Vec<_> = aggregates
            .into_iter()
            .map(|(timestamp, values)| {
                let count = values.len();
                let sum: f64 = values.iter().sum();
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let avg = sum / count as f64;

                AggregatePoint {
                    timestamp,
                    count,
                    min: Some(min),
                    max: Some(max),
                    avg: Some(avg),
                    sum: Some(sum),
                }
            })
            .collect();

        results.sort_by_key(|p| p.timestamp);
        Ok(results)
    }
}
