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
    async fn search(&self, query: &str, year: Option<String>) -> Result<Vec<NormalizedMetadata>, AppError>;
    
    /// Get detailed information by provider-specific ID
    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError>;
    
    /// Get episode list for a TV series season
    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<EpisodeMetadata>, AppError>;

    /// Optional: lightweight connectivity/key check for the "Test" button.
    /// Providers should override this to verify their API key or connectivity.
    async fn health_check(&self) -> Result<(), AppError> {
        Ok(())
    }

    /// Returns the unique provider id (e.g. "tmdb").
    /// Used by MetadataService to identify which provider produced a result.
    fn provider_id(&self) -> &'static str;
}
