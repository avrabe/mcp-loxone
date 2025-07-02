//! Unified services for the Loxone MCP server
//!
//! This module contains centralized services that provide a single source
//! of truth for device values, sensor detection, and state management.

pub mod cache_manager;
pub mod connection_pool;
pub mod sensor_registry;
pub mod state_manager;
pub mod unified_models;
pub mod value_parsers;
pub mod value_resolution;

pub use sensor_registry::{SensorInventory, SensorType, SensorTypeRegistry};
pub use state_manager::{
    ChangeSignificance, ChangeType, DeviceState, StateChangeEvent, StateManager, StateQuality,
};
pub use unified_models::{
    DataQuality, DataSource, SemanticValue, UnifiedDeviceValue, UnifiedDeviceValueBatch,
    UnifiedValue, ValueMetadata,
};
pub use value_parsers::{ParsedValue, ValueParser, ValueParserRegistry};
pub use value_resolution::{ResolvedValue, UnifiedValueResolver, ValidationStatus, ValueSource};
