//! Cold storage implementation - persistent file-based storage for historical data

use super::config::{ColdStorageConfig, CompressionType};
use super::events::*;
use crate::error::{LoxoneError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Cold data store for persistent historical data
pub struct ColdDataStore {
    /// Configuration
    config: ColdStorageConfig,

    /// Data index for fast lookups
    index: Arc<RwLock<DataIndex>>,

    /// Storage statistics
    stats: Arc<RwLock<ColdStorageStats>>,
}

/// Index for cold storage data
#[derive(Debug, Default, Serialize, Deserialize)]
struct DataIndex {
    /// File entries by date
    entries: HashMap<String, Vec<IndexEntry>>,

    /// Total size of all files
    total_size_bytes: u64,

    /// Last updated timestamp
    last_updated: DateTime<Utc>,
}

/// Index entry for a data file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    /// File path relative to data directory
    path: String,

    /// Event category
    category: String,

    /// Date of the data
    date: String,

    /// Number of events in the file
    event_count: usize,

    /// File size in bytes
    size_bytes: u64,

    /// Creation timestamp
    created_at: DateTime<Utc>,

    /// Is compressed
    compressed: bool,
}

/// Statistics for cold storage
#[derive(Debug, Default)]
struct ColdStorageStats {
    total_files: usize,
    total_events: u64,
    bytes_written: u64,
    bytes_read: u64,
    compression_ratio: f64,
}

/// Data file format
#[derive(Debug, Serialize, Deserialize)]
struct DataFile {
    /// Metadata about the file
    metadata: DataFileMetadata,

    /// Events in the file
    events: Vec<HistoricalEvent>,
}

/// Metadata for a data file
#[derive(Debug, Serialize, Deserialize)]
struct DataFileMetadata {
    /// File version
    version: u32,

    /// Creation timestamp
    created_at: DateTime<Utc>,

    /// Event category
    category: String,

    /// Date range
    date_range: DateRange,

    /// Number of events
    event_count: usize,

    /// Compression type
    compression: Option<String>,
}

/// Date range for events
#[derive(Debug, Serialize, Deserialize)]
struct DateRange {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl ColdDataStore {
    /// Create new cold data store
    pub async fn new(config: ColdStorageConfig) -> Result<Self> {
        // Ensure data directory exists
        fs::create_dir_all(&config.data_dir)
            .await
            .map_err(LoxoneError::from)?;

        // Load or create index
        let index = DataIndex::load_or_create(&config.data_dir).await?;

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            stats: Arc::new(RwLock::new(ColdStorageStats::default())),
        })
    }

    /// Store events in cold storage
    pub async fn store_events(&self, events: Vec<HistoricalEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Group events by category and date
        let mut grouped: HashMap<(String, String), Vec<HistoricalEvent>> = HashMap::new();

        for event in events {
            let category = match &event.category {
                EventCategory::DeviceState(_) => "device_state",
                EventCategory::SensorReading(_) => "sensor_reading",
                EventCategory::SystemMetric(_) => "system_metric",
                EventCategory::AuditEvent(_) => "audit_event",
                EventCategory::DiscoveryEvent(_) => "discovery_event",
                EventCategory::ResponseCache(_) => "response_cache",
            }
            .to_string();

            let date = event.timestamp.format("%Y-%m-%d").to_string();
            grouped.entry((category, date)).or_default().push(event);
        }

        // Store each group
        for ((category, date), events) in grouped {
            self.store_category_events(&category, &date, events).await?;
        }

        Ok(())
    }

    /// Store events for a specific category and date
    async fn store_category_events(
        &self,
        category: &str,
        date: &str,
        mut events: Vec<HistoricalEvent>,
    ) -> Result<()> {
        // Sort events by timestamp
        events.sort_by_key(|e| e.timestamp);

        // Create file path
        let file_name = format!("{}__{}.json", category, date);
        let file_path = self.config.data_dir.join(&file_name);

        // Load existing data if file exists
        let mut existing_events = if file_path.exists() {
            self.load_file(&file_path).await?.events
        } else {
            Vec::new()
        };

        // Merge events
        existing_events.extend(events);
        existing_events.sort_by_key(|e| e.timestamp);
        existing_events.dedup_by_key(|e| e.id);

        // Create data file
        let data_file = DataFile {
            metadata: DataFileMetadata {
                version: 1,
                created_at: Utc::now(),
                category: category.to_string(),
                date_range: DateRange {
                    start: existing_events.first().unwrap().timestamp,
                    end: existing_events.last().unwrap().timestamp,
                },
                event_count: existing_events.len(),
                compression: match self.config.compression {
                    CompressionType::None => None,
                    CompressionType::Gzip => Some("gzip".to_string()),
                    CompressionType::Zstd => Some("zstd".to_string()),
                    CompressionType::Lz4 => Some("lz4".to_string()),
                },
            },
            events: existing_events,
        };

        // Write file
        self.write_file(&file_path, &data_file).await?;

        // Update index
        self.update_index(&file_name, &data_file.metadata).await?;

        info!("Stored {} events in {}", data_file.events.len(), file_name);

        Ok(())
    }

    /// Load a data file
    async fn load_file(&self, path: &Path) -> Result<DataFile> {
        let data = fs::read(path).await.map_err(LoxoneError::from)?;

        self.stats.write().await.bytes_read += data.len() as u64;

        // Decompress if needed
        let decompressed = match self.config.compression {
            CompressionType::None => data,
            CompressionType::Gzip => {
                use flate2::read::GzDecoder;
                use std::io::Read;
                let mut decoder = GzDecoder::new(&data[..]);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(LoxoneError::from)?;
                decompressed
            }
            CompressionType::Zstd => zstd::decode_all(&data[..]).map_err(|e| {
                LoxoneError::Generic(anyhow::anyhow!("Failed to decompress: {}", e))
            })?,
            CompressionType::Lz4 => lz4::block::decompress(&data, None).map_err(|e| {
                LoxoneError::Generic(anyhow::anyhow!("Failed to decompress: {}", e))
            })?,
        };

        // Deserialize
        serde_json::from_slice(&decompressed)
            .map_err(|e| LoxoneError::parsing_error(format!("Failed to parse data file: {}", e)))
    }

    /// Write a data file
    async fn write_file(&self, path: &Path, data: &DataFile) -> Result<()> {
        // Serialize
        let json = serde_json::to_vec_pretty(data)
            .map_err(|e| LoxoneError::parsing_error(format!("Failed to serialize data: {}", e)))?;

        // Compress if configured
        let compressed = match self.config.compression {
            CompressionType::None => json.clone(),
            CompressionType::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&json).map_err(|e| {
                    LoxoneError::Generic(anyhow::anyhow!("Failed to compress: {}", e))
                })?;
                encoder.finish().map_err(|e| {
                    LoxoneError::Generic(anyhow::anyhow!("Failed to finish compression: {}", e))
                })?
            }
            CompressionType::Zstd => zstd::encode_all(&json[..], 3)
                .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to compress: {}", e)))?,
            CompressionType::Lz4 => lz4::block::compress(&json, None, true)
                .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to compress: {}", e)))?,
        };

        // Update compression ratio
        if !json.is_empty() {
            let ratio = compressed.len() as f64 / json.len() as f64;
            self.stats.write().await.compression_ratio = ratio;
        }

        // Write atomically using temp file
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &compressed)
            .await
            .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to write file: {}", e)))?;

        fs::rename(&temp_path, path)
            .await
            .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to rename file: {}", e)))?;

        self.stats.write().await.bytes_written += compressed.len() as u64;

        Ok(())
    }

    /// Update the index
    async fn update_index(&self, file_name: &str, metadata: &DataFileMetadata) -> Result<()> {
        let mut index = self.index.write().await;

        let entry = IndexEntry {
            path: file_name.to_string(),
            category: metadata.category.clone(),
            date: metadata.date_range.start.format("%Y-%m-%d").to_string(),
            event_count: metadata.event_count,
            size_bytes: 0, // Will be updated on next index save
            created_at: metadata.created_at,
            compressed: metadata.compression.is_some(),
        };

        index
            .entries
            .entry(entry.date.clone())
            .or_insert_with(Vec::new)
            .push(entry);

        index.last_updated = Utc::now();

        // Save index periodically
        if index.last_updated.timestamp() % 60 == 0 {
            index.save(&self.config.data_dir).await?;
        }

        Ok(())
    }

    /// Query events from cold storage
    pub async fn query_events(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        categories: Option<Vec<String>>,
        limit: Option<usize>,
    ) -> Result<Vec<HistoricalEvent>> {
        let index = self.index.read().await;
        let mut results = Vec::new();

        // Find relevant files
        let start_date = start.format("%Y-%m-%d").to_string();
        let end_date = end.format("%Y-%m-%d").to_string();

        for (date, entries) in &index.entries {
            if date >= &start_date && date <= &end_date {
                for entry in entries {
                    // Filter by category if specified
                    if let Some(ref cats) = categories {
                        if !cats.contains(&entry.category) {
                            continue;
                        }
                    }

                    // Load file
                    let file_path = self.config.data_dir.join(&entry.path);
                    match self.load_file(&file_path).await {
                        Ok(data_file) => {
                            // Filter events by time range
                            let filtered: Vec<_> = data_file
                                .events
                                .into_iter()
                                .filter(|e| e.timestamp >= start && e.timestamp <= end)
                                .collect();
                            results.extend(filtered);
                        }
                        Err(e) => {
                            error!("Failed to load file {}: {}", entry.path, e);
                        }
                    }

                    // Check limit
                    if let Some(limit) = limit {
                        if results.len() >= limit {
                            results.truncate(limit);
                            return Ok(results);
                        }
                    }
                }
            }
        }

        // Sort by timestamp
        results.sort_by_key(|e| e.timestamp);

        Ok(results)
    }

    /// Clean up old data based on retention policies
    pub async fn cleanup(&self, retention_days: HashMap<String, u32>) -> Result<()> {
        let mut index = self.index.write().await;
        let now = Utc::now();
        let mut files_to_remove = Vec::new();

        for entries in index.entries.values() {
            for entry in entries {
                // Get retention for this category
                let retention = retention_days.get(&entry.category).copied().unwrap_or(30);
                let cutoff = now - chrono::Duration::days(retention as i64);

                if entry.created_at < cutoff {
                    files_to_remove.push(entry.path.clone());
                }
            }
        }

        // Remove files
        for file_name in &files_to_remove {
            let file_path = self.config.data_dir.join(file_name);
            if let Err(e) = fs::remove_file(&file_path).await {
                error!("Failed to remove file {}: {}", file_name, e);
            } else {
                debug!("Removed expired file: {}", file_name);
            }
        }

        // Update index
        for (_, entries) in index.entries.iter_mut() {
            entries.retain(|e| !files_to_remove.contains(&e.path));
        }

        // Remove empty dates
        index.entries.retain(|_, entries| !entries.is_empty());

        info!("Cleaned up {} expired files", files_to_remove.len());

        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> (usize, u64, f64) {
        let stats = self.stats.read().await;
        let index = self.index.read().await;

        let total_events: usize = index
            .entries
            .values()
            .flat_map(|entries| entries.iter())
            .map(|e| e.event_count)
            .sum();

        (
            stats.total_files,
            total_events as u64,
            stats.compression_ratio,
        )
    }
}

impl DataIndex {
    /// Load index from disk or create new
    async fn load_or_create(data_dir: &Path) -> Result<Self> {
        let index_path = data_dir.join("index.json");

        if index_path.exists() {
            let data = fs::read(&index_path).await.map_err(|e| {
                LoxoneError::Generic(anyhow::anyhow!("Failed to read index: {}", e))
            })?;

            serde_json::from_slice(&data)
                .map_err(|e| LoxoneError::parsing_error(format!("Failed to parse index: {}", e)))
        } else {
            Ok(Self::default())
        }
    }

    /// Save index to disk
    async fn save(&self, data_dir: &Path) -> Result<()> {
        let index_path = data_dir.join("index.json");
        let temp_path = data_dir.join("index.json.tmp");

        let data = serde_json::to_vec_pretty(self)
            .map_err(|e| LoxoneError::parsing_error(format!("Failed to serialize index: {}", e)))?;

        fs::write(&temp_path, data)
            .await
            .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to write index: {}", e)))?;

        fs::rename(&temp_path, &index_path)
            .await
            .map_err(|e| LoxoneError::Generic(anyhow::anyhow!("Failed to rename index: {}", e)))?;

        Ok(())
    }
}

// Add compression dependencies to Cargo.toml
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cold_storage() {
        let temp_dir = tempdir().unwrap();
        let config = ColdStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
            compression: CompressionType::None,
            max_size_bytes: 1024 * 1024,
            index_cache_size_mb: 1,
        };

        let store = ColdDataStore::new(config).await.unwrap();

        // Test storing events
        let events = vec![HistoricalEvent::system_metric(MetricData {
            metric_name: "test".to_string(),
            value: 42.0,
            unit: "count".to_string(),
            tags: HashMap::new(),
        })];

        assert!(store.store_events(events).await.is_ok());
    }
}
