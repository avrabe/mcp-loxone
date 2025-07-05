//! Utility modules for common functionality

pub mod error_helpers;

// Re-export commonly used helpers
pub use error_helpers::{
    parse_socket_addr_safe, parse_with_context, safe_header_pair, safe_mutex_lock,
};
