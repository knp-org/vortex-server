//! Metadata provider trait
//!
//! Defines the interface that all metadata providers must implement.

use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};
use crate::error::AppError;
use async_trait::async_trait;

/// Trait for metadata providers (TMDB, TVDB, etc.)
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    /// Search for media by query string
    async fn search(&self, query: &str) -> Result<Vec<NormalizedMetadata>, AppError>;
    
    /// Get detailed information by provider-specific ID
    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError>;
    
    /// Get episode list for a TV series season
    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<EpisodeMetadata>, AppError>;
}
