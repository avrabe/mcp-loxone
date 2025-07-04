//! Storage layer for weather data and system metrics
//!
//! This module provides storage implementations for:
//! - Weather data from WebSocket streams
//! - Device state history
//! - System metrics and analytics
//!
//! Available implementations:
//! - Simple in-memory storage (default)
//! - Turso database storage (with "turso" feature)

pub mod simple_storage;

#[cfg(feature = "turso")]
pub mod turso_client;
#[cfg(feature = "turso")]
pub mod weather_storage;

// Default to simple storage
pub use simple_storage::{
    SimpleWeatherStorage as WeatherStorage, SimpleWeatherStorageConfig as WeatherStorageConfig,
};

#[cfg(feature = "turso")]
pub use turso_client::TursoClient;
#[cfg(feature = "turso")]
pub use weather_storage::WeatherStorage as TursoWeatherStorage;
