//! Discovery module for network device discovery and caching
//!
//! This module provides network discovery capabilities with intelligent caching
//! to reduce redundant network scans and improve performance.

pub mod device_discovery;
pub mod discovery_cache;

#[cfg(feature = "discovery")]
pub mod mdns;
#[cfg(feature = "discovery")]
pub mod network;

// Re-export main types for convenience
pub use device_discovery::DeviceDiscovery;
pub use discovery_cache::{
    CacheEntry, DiscoveredDevice, DiscoveryCache, DiscoveryCacheConfig, DiscoveryCacheStats,
    NetworkContext,
};
