//! Compatibility adapters for existing implementations
//!
//! These adapters allow existing code to continue working while gradually
//! migrating to the unified history system.

pub mod sensor_history;

pub use sensor_history::SensorHistoryAdapter;
