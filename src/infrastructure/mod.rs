//! Infrastructure module - cross-cutting concerns
//!
//! Contains configuration, error handling, caching, and logging utilities.

pub mod config;
pub mod error;
pub mod cache;
pub mod logging;

// Re-exports for convenience
pub use config::{config, init_config};
