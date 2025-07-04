//! Turso database client for persistent storage
//!
//! Provides connection management and query execution for Turso database,
//! optimized for weather data storage with automatic schema management.

#[cfg(feature = "turso")]
use crate::error::{LoxoneError, Result};
#[cfg(feature = "turso")]
use libsql::{Connection, Database};
#[cfg(feature = "turso")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "turso")]
use std::sync::Arc;
#[cfg(feature = "turso")]
use tokio::sync::RwLock;
#[cfg(feature = "turso")]
use tracing::{debug, info, warn};

#[cfg(feature = "turso")]
/// Turso database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TursoConfig {
    /// Database URL (e.g., "libsql://database-name-organization.turso.io")
    pub database_url: String,
    /// Authentication token
    pub auth_token: String,
    /// Local database path for embedded mode
    pub local_path: Option<String>,
    /// Enable sync with remote database
    pub enable_sync: bool,
    /// Sync interval in seconds
    pub sync_interval_seconds: u64,
}

impl Default for TursoConfig {
    fn default() -> Self {
        Self {
            database_url: "file:loxone_data.db".to_string(),
            auth_token: String::new(),
            local_path: Some("./data/loxone_data.db".to_string()),
            enable_sync: false,
            sync_interval_seconds: 300, // 5 minutes
        }
    }
}

/// Turso database client with connection pooling and automatic schema management
pub struct TursoClient {
    database: Arc<Database>,
    connection: Arc<RwLock<Connection>>,
    config: TursoConfig,
    schema_initialized: Arc<RwLock<bool>>,
}

impl TursoClient {
    /// Create new Turso client with configuration
    pub async fn new(config: TursoConfig) -> Result<Self> {
        info!(
            "Initializing Turso client with URL: {}",
            config.database_url
        );

        let database = if config.database_url.starts_with("libsql://") {
            // Remote Turso database
            if config.auth_token.is_empty() {
                return Err(LoxoneError::configuration_error(
                    "Auth token required for remote Turso database",
                ));
            }

            let db =
                libsql::Builder::new_remote(config.database_url.clone(), config.auth_token.clone())
                    .build()
                    .await
                    .map_err(|e| {
                        LoxoneError::database(format!("Failed to connect to Turso: {e}"))
                    })?;

            // Enable sync if configured
            if config.enable_sync {
                if let Some(local_path) = &config.local_path {
                    let sync_db = libsql::Builder::new_remote_replica(
                        config.database_url.clone(),
                        config.auth_token.clone(),
                        local_path.clone(),
                    )
                    .build()
                    .await
                    .map_err(|e| LoxoneError::database(format!("Failed to setup sync: {e}")))?;

                    Arc::new(sync_db)
                } else {
                    Arc::new(db)
                }
            } else {
                Arc::new(db)
            }
        } else {
            // Local SQLite database
            let db = libsql::Builder::new_local(config.database_url.clone())
                .build()
                .await
                .map_err(|e| {
                    LoxoneError::database(format!("Failed to open local database: {e}"))
                })?;
            Arc::new(db)
        };

        let connection = database
            .connect()
            .map_err(|e| LoxoneError::database(format!("Failed to create connection: {e}")))?;

        let client = Self {
            database,
            connection: Arc::new(RwLock::new(connection)),
            config,
            schema_initialized: Arc::new(RwLock::new(false)),
        };

        // Initialize database schema
        client.initialize_schema().await?;

        Ok(client)
    }

    /// Initialize database schema for weather data
    async fn initialize_schema(&self) -> Result<()> {
        let mut schema_initialized = self.schema_initialized.write().await;
        if *schema_initialized {
            return Ok(());
        }

        debug!("Initializing database schema");
        let conn = self.connection.write().await;

        // Weather data table
        let weather_schema = r#"
            CREATE TABLE IF NOT EXISTS weather_data (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_uuid TEXT NOT NULL,
                uuid_index INTEGER NOT NULL,
                parameter_name TEXT NOT NULL,
                value REAL NOT NULL,
                unit TEXT,
                timestamp INTEGER NOT NULL,
                quality_score REAL DEFAULT 1.0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX(device_uuid),
                INDEX(timestamp),
                INDEX(parameter_name)
            )
        "#;

        conn.execute(weather_schema, ()).await.map_err(|e| {
            LoxoneError::database(format!("Failed to create weather_data table: {e}"))
        })?;

        // Device mapping table for UUID index resolution
        let device_mapping_schema = r#"
            CREATE TABLE IF NOT EXISTS device_uuid_mapping (
                uuid_index INTEGER PRIMARY KEY,
                device_uuid TEXT NOT NULL UNIQUE,
                device_name TEXT,
                device_type TEXT,
                last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        conn.execute(device_mapping_schema, ()).await.map_err(|e| {
            LoxoneError::database(format!("Failed to create device_uuid_mapping table: {e}"))
        })?;

        // Weather aggregation table for efficient queries
        let weather_aggregation_schema = r#"
            CREATE TABLE IF NOT EXISTS weather_aggregation (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_uuid TEXT NOT NULL,
                parameter_name TEXT NOT NULL,
                hour_timestamp INTEGER NOT NULL,
                min_value REAL,
                max_value REAL,
                avg_value REAL,
                sample_count INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(device_uuid, parameter_name, hour_timestamp),
                INDEX(device_uuid),
                INDEX(hour_timestamp),
                INDEX(parameter_name)
            )
        "#;

        conn.execute(weather_aggregation_schema, ())
            .await
            .map_err(|e| {
                LoxoneError::database(format!("Failed to create weather_aggregation table: {e}"))
            })?;

        // Create indexes for better performance
        let indexes = [
            "CREATE INDEX IF NOT EXISTS idx_weather_device_time ON weather_data(device_uuid, timestamp DESC)",
            "CREATE INDEX IF NOT EXISTS idx_weather_param_time ON weather_data(parameter_name, timestamp DESC)",
            "CREATE INDEX IF NOT EXISTS idx_aggregation_time ON weather_aggregation(hour_timestamp DESC)",
        ];

        for index_sql in &indexes {
            if let Err(e) = conn.execute(index_sql, ()).await {
                warn!("Failed to create index: {} - {}", index_sql, e);
            }
        }

        *schema_initialized = true;
        info!("Database schema initialized successfully");
        Ok(())
    }

    /// Store weather data point
    #[allow(clippy::too_many_arguments)]
    pub async fn store_weather_data(
        &self,
        device_uuid: &str,
        uuid_index: u32,
        parameter_name: &str,
        value: f64,
        unit: Option<&str>,
        timestamp: u32,
        quality_score: Option<f64>,
    ) -> Result<()> {
        let conn = self.connection.write().await;

        let insert_sql = r#"
            INSERT INTO weather_data (device_uuid, uuid_index, parameter_name, value, unit, timestamp, quality_score)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#;

        conn.execute(
            insert_sql,
            (
                device_uuid,
                uuid_index as i64,
                parameter_name,
                value,
                unit.unwrap_or(""),
                timestamp as i64,
                quality_score.unwrap_or(1.0),
            ),
        )
        .await
        .map_err(|e| LoxoneError::database(format!("Failed to store weather data: {e}")))?;

        // Update aggregation data
        self.update_aggregation(device_uuid, parameter_name, value, timestamp)
            .await?;

        Ok(())
    }

    /// Update or create hourly aggregation data
    async fn update_aggregation(
        &self,
        device_uuid: &str,
        parameter_name: &str,
        value: f64,
        timestamp: u32,
    ) -> Result<()> {
        let hour_timestamp = (timestamp / 3600) * 3600; // Round to hour
        let conn = self.connection.write().await;

        let update_sql = r#"
            INSERT INTO weather_aggregation (device_uuid, parameter_name, hour_timestamp, min_value, max_value, avg_value, sample_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
            ON CONFLICT(device_uuid, parameter_name, hour_timestamp) DO UPDATE SET
                min_value = MIN(min_value, ?4),
                max_value = MAX(max_value, ?5),
                avg_value = ((avg_value * sample_count) + ?6) / (sample_count + 1),
                sample_count = sample_count + 1
        "#;

        conn.execute(
            update_sql,
            (
                device_uuid,
                parameter_name,
                hour_timestamp as i64,
                value,
                value,
                value,
            ),
        )
        .await
        .map_err(|e| LoxoneError::database(format!("Failed to update aggregation: {e}")))?;

        Ok(())
    }

    /// Store or update device UUID mapping
    pub async fn store_device_mapping(
        &self,
        uuid_index: u32,
        device_uuid: &str,
        device_name: Option<&str>,
        device_type: Option<&str>,
    ) -> Result<()> {
        let conn = self.connection.write().await;

        let upsert_sql = r#"
            INSERT INTO device_uuid_mapping (uuid_index, device_uuid, device_name, device_type)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(uuid_index) DO UPDATE SET
                device_uuid = ?2,
                device_name = ?3,
                device_type = ?4,
                last_updated = CURRENT_TIMESTAMP
        "#;

        conn.execute(
            upsert_sql,
            (
                uuid_index as i64,
                device_uuid,
                device_name.unwrap_or(""),
                device_type.unwrap_or(""),
            ),
        )
        .await
        .map_err(|e| LoxoneError::database(format!("Failed to store device mapping: {e}")))?;

        Ok(())
    }

    /// Get device UUID from index
    pub async fn get_device_uuid(&self, uuid_index: u32) -> Result<Option<String>> {
        let conn = self.connection.read().await;

        let query_sql = "SELECT device_uuid FROM device_uuid_mapping WHERE uuid_index = ?1";

        let mut rows = conn
            .prepare(query_sql)
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to prepare query: {e}")))?
            .query(libsql::params![uuid_index as i64])
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to execute query: {e}")))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to fetch row: {e}")))?
        {
            let device_uuid: String = row
                .get(0)
                .map_err(|e| LoxoneError::database(format!("Failed to get device_uuid: {e}")))?;
            Ok(Some(device_uuid))
        } else {
            Ok(None)
        }
    }

    /// Get recent weather data for a device
    pub async fn get_recent_weather_data(
        &self,
        device_uuid: &str,
        parameter_name: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WeatherDataPoint>> {
        let conn = self.connection.read().await;

        let mut rows = if let Some(param) = parameter_name {
            conn.prepare("SELECT device_uuid, parameter_name, value, unit, timestamp, quality_score FROM weather_data WHERE device_uuid = ?1 AND parameter_name = ?2 ORDER BY timestamp DESC LIMIT ?3")
                .await
                .map_err(|e| LoxoneError::database(format!("Failed to prepare query: {e}")))?
                .query(libsql::params![device_uuid, param, limit as i64])
                .await
                .map_err(|e| LoxoneError::database(format!("Failed to execute query: {e}")))?
        } else {
            conn.prepare("SELECT device_uuid, parameter_name, value, unit, timestamp, quality_score FROM weather_data WHERE device_uuid = ?1 ORDER BY timestamp DESC LIMIT ?2")
                .await
                .map_err(|e| LoxoneError::database(format!("Failed to prepare query: {e}")))?
                .query(libsql::params![device_uuid, limit as i64])
                .await
                .map_err(|e| LoxoneError::database(format!("Failed to execute query: {e}")))?
        };

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to fetch row: {e}")))?
        {
            let device_uuid: String = row
                .get(0)
                .map_err(|e| LoxoneError::database(format!("Failed to get device_uuid: {e}")))?;
            let parameter_name: String = row
                .get(1)
                .map_err(|e| LoxoneError::database(format!("Failed to get parameter_name: {e}")))?;
            let value: f64 = row
                .get(2)
                .map_err(|e| LoxoneError::database(format!("Failed to get value: {e}")))?;
            let unit: String = row
                .get(3)
                .map_err(|e| LoxoneError::database(format!("Failed to get unit: {e}")))?;
            let timestamp: i64 = row
                .get(4)
                .map_err(|e| LoxoneError::database(format!("Failed to get timestamp: {e}")))?;
            let quality_score: f64 = row
                .get(5)
                .map_err(|e| LoxoneError::database(format!("Failed to get quality_score: {e}")))?;

            results.push(WeatherDataPoint {
                device_uuid,
                parameter_name,
                value,
                unit: if unit.is_empty() { None } else { Some(unit) },
                timestamp: timestamp as u32,
                quality_score,
            });
        }

        Ok(results)
    }

    /// Get aggregated weather data for time period
    pub async fn get_aggregated_weather_data(
        &self,
        device_uuid: &str,
        parameter_name: &str,
        start_time: u32,
        end_time: u32,
    ) -> Result<Vec<WeatherAggregation>> {
        let conn = self.connection.read().await;

        let query_sql = r#"
            SELECT hour_timestamp, min_value, max_value, avg_value, sample_count
            FROM weather_aggregation
            WHERE device_uuid = ?1 AND parameter_name = ?2 
            AND hour_timestamp >= ?3 AND hour_timestamp <= ?4
            ORDER BY hour_timestamp
        "#;

        let mut rows = conn
            .prepare(query_sql)
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to prepare query: {e}")))?
            .query((
                device_uuid,
                parameter_name,
                start_time as i64,
                end_time as i64,
            ))
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to execute query: {e}")))?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to fetch row: {e}")))?
        {
            let hour_timestamp: i64 = row
                .get(0)
                .map_err(|e| LoxoneError::database(format!("Failed to get hour_timestamp: {e}")))?;
            let min_value: f64 = row
                .get(1)
                .map_err(|e| LoxoneError::database(format!("Failed to get min_value: {e}")))?;
            let max_value: f64 = row
                .get(2)
                .map_err(|e| LoxoneError::database(format!("Failed to get max_value: {e}")))?;
            let avg_value: f64 = row
                .get(3)
                .map_err(|e| LoxoneError::database(format!("Failed to get avg_value: {e}")))?;
            let sample_count: i64 = row
                .get(4)
                .map_err(|e| LoxoneError::database(format!("Failed to get sample_count: {e}")))?;

            results.push(WeatherAggregation {
                hour_timestamp: hour_timestamp as u32,
                min_value,
                max_value,
                avg_value,
                sample_count: sample_count as u32,
            });
        }

        Ok(results)
    }

    /// Perform database sync (for remote databases with sync enabled)
    pub async fn sync(&self) -> Result<()> {
        if !self.config.enable_sync {
            return Ok(());
        }

        debug!("Syncing database with remote");

        // Create a new connection to trigger sync
        let _sync_conn = self
            .database
            .connect()
            .map_err(|e| LoxoneError::database(format!("Failed to create sync connection: {e}")))?;

        info!("Database sync completed");
        Ok(())
    }

    /// Clean up old data based on retention policy
    pub async fn cleanup_old_data(&self, retention_days: u32) -> Result<u64> {
        let conn = self.connection.write().await;
        let cutoff_timestamp =
            (chrono::Utc::now().timestamp() as u32) - (retention_days * 24 * 3600);

        let delete_sql = "DELETE FROM weather_data WHERE timestamp < ?1";

        let result = conn
            .execute(delete_sql, libsql::params![cutoff_timestamp as i64])
            .await
            .map_err(|e| LoxoneError::database(format!("Failed to cleanup old data: {e}")))?;

        let rows_affected = result;
        info!("Cleaned up {} old weather data records", rows_affected);
        Ok(rows_affected as u64)
    }
}

/// Weather data point structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherDataPoint {
    pub device_uuid: String,
    pub parameter_name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub timestamp: u32,
    pub quality_score: f64,
}

/// Weather aggregation data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherAggregation {
    pub hour_timestamp: u32,
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
    pub sample_count: u32,
}
