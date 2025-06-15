//! Automatic data tiering from hot to cold storage

use super::cold_storage::ColdDataStore;
use super::hot_storage::HotDataStore;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Manager for automatic data tiering
pub struct TieringManager {
    hot_store: Arc<RwLock<HotDataStore>>,
    cold_store: Arc<ColdDataStore>,
    interval_seconds: u64,
    stats: Arc<RwLock<TieringStats>>,
}

/// Statistics for tiering operations
#[derive(Debug, Default)]
struct TieringStats {
    total_cycles: u64,
    events_tiered: u64,
    bytes_tiered: u64,
    last_run: Option<chrono::DateTime<chrono::Utc>>,
    last_duration_ms: u64,
}

impl TieringManager {
    /// Create new tiering manager
    pub fn new(
        hot_store: Arc<RwLock<HotDataStore>>,
        cold_store: Arc<ColdDataStore>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            hot_store,
            cold_store,
            interval_seconds,
            stats: Arc::new(RwLock::new(TieringStats::default())),
        }
    }

    /// Run a tiering cycle
    pub async fn run_tiering_cycle(&self) -> Result<()> {
        let start_time = std::time::Instant::now();
        debug!("Starting tiering cycle");

        // Check if tiering is needed
        let needs_tiering = self.hot_store.read().await.needs_tiering().await;

        if !needs_tiering {
            debug!("No tiering needed, skipping cycle");
            return Ok(());
        }

        // Get candidates for tiering
        let candidates = self.hot_store.read().await.get_tiering_candidates().await;

        if candidates.is_empty() {
            debug!("No tiering candidates found");
            return Ok(());
        }

        info!("Tiering {} events to cold storage", candidates.len());

        // Store in cold storage
        self.cold_store.store_events(candidates.clone()).await?;

        // Remove from hot storage
        let event_ids: Vec<_> = candidates.iter().map(|e| e.id).collect();
        self.hot_store
            .write()
            .await
            .remove_tiered_events(&event_ids)
            .await?;

        // Update statistics
        let duration = start_time.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_cycles += 1;
        stats.events_tiered += candidates.len() as u64;
        stats.last_run = Some(chrono::Utc::now());
        stats.last_duration_ms = duration.as_millis() as u64;

        info!(
            "Tiering cycle completed: {} events in {}ms",
            candidates.len(),
            duration.as_millis()
        );

        Ok(())
    }

    /// Force tiering regardless of need
    pub async fn force_tiering(&self) -> Result<()> {
        let candidates = self.hot_store.read().await.get_tiering_candidates().await;

        if candidates.is_empty() {
            warn!("No events available for forced tiering");
            return Ok(());
        }

        info!("Force tiering {} events", candidates.len());

        self.cold_store.store_events(candidates.clone()).await?;

        let event_ids: Vec<_> = candidates.iter().map(|e| e.id).collect();
        self.hot_store
            .write()
            .await
            .remove_tiered_events(&event_ids)
            .await?;

        Ok(())
    }

    /// Get tiering statistics
    pub async fn get_stats(&self) -> TieringStatsSnapshot {
        let stats = self.stats.read().await;
        TieringStatsSnapshot {
            total_cycles: stats.total_cycles,
            events_tiered: stats.events_tiered,
            bytes_tiered: stats.bytes_tiered,
            last_run: stats.last_run,
            last_duration_ms: stats.last_duration_ms,
            interval_seconds: self.interval_seconds,
        }
    }
}

/// Snapshot of tiering statistics
#[derive(Debug, serde::Serialize)]
pub struct TieringStatsSnapshot {
    pub total_cycles: u64,
    pub events_tiered: u64,
    pub bytes_tiered: u64,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub last_duration_ms: u64,
    pub interval_seconds: u64,
}
