use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};
use crate::error::AppError;
use async_trait::async_trait;

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedMetadata>, AppError>;
    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError>;
    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<EpisodeMetadata>, AppError>;
}
