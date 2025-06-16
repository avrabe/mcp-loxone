//! Proper mDNS/Zeroconf discovery implementation for Loxone Miniservers
//!
//! This module implements real mDNS discovery using the mdns crate
//! to find Loxone Miniservers on the local network.

#![cfg(feature = "mdns")]

use super::network::DiscoveredServer;
use crate::error::Result;
use std::time::Duration;
use tracing::debug;

/// Perform mDNS discovery for Loxone Miniservers
pub async fn discover_via_mdns(_timeout: Duration) -> Result<Vec<DiscoveredServer>> {
    // mDNS discovery is currently disabled due to unmaintained dependencies
    // The mdns crate depends on unmaintained crates:
    // - net2 (last updated 2017)
    // - proc-macro-error (unmaintained)
    //
    // When these dependencies are updated or replaced with maintained alternatives,
    // the full mDNS implementation can be restored from git history.
    debug!("mDNS discovery is disabled due to unmaintained dependencies");
    Ok(Vec::new())
}

// The full mDNS implementation has been preserved in git history
// and can be restored when the mdns crate dependencies are updated.
