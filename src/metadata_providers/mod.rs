//! Metadata Providers Module
//!
//! External metadata providers (TMDB, TVDB, etc.) for fetching movie/TV show information.
//!
//! This module contains:
//! - `traits` - The `MetadataProvider` trait that all providers implement
//! - `tmdb` - TMDB (The Movie Database) provider implementation

pub mod traits;
pub mod tmdb;

