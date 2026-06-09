//! Metadata Providers Module
//!
//! External metadata providers (TMDB, TVDB, etc.) for fetching movie/TV show information.
//!
//! This module contains:
//! - `traits` - The `MetadataProvider` trait that all providers implement
//! - `manifest` - Provider manifest and config schema types
//! - `registry` - Central provider registry (single source of truth)
//! - `tmdb` - TMDB (The Movie Database) provider implementation

pub mod traits;
pub mod manifest;
pub mod registry;
pub mod tmdb;
pub mod tvdb;

