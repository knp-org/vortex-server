use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};
use crate::providers::traits::MetadataProvider;
use crate::providers::tmdb::TmdbProvider;
use sqlx::SqlitePool;
use crate::error::AppError;

/// Default provider if not configured in settings
const DEFAULT_PROVIDER: &str = "tmdb";

/// Get the configured default provider from settings, or use DEFAULT_PROVIDER
pub async fn get_default_provider(pool: &SqlitePool) -> String {
    let result: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'metadata_provider'")
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    result.map(|r| r.0).unwrap_or_else(|| DEFAULT_PROVIDER.to_string())
}

/// Get a provider instance by name
pub async fn get_provider(pool: &SqlitePool, provider: &str) -> Result<Box<dyn MetadataProvider>, AppError> {
    match provider {
        "tmdb" => {
            let api_key = TmdbProvider::fetch_api_key(pool).await?;
            Ok(Box::new(TmdbProvider::new(api_key)))
        },
        // Future: "tvdb" => { ... },
        _ => Err(AppError::BadRequest(format!("Unknown provider: {}", provider)))
    }
}

/// Fetch metadata using the default configured provider
pub async fn fetch_metadata(
    query: &str,
    _media_type_hint: Option<&str>,
    pool: &SqlitePool
) -> Result<NormalizedMetadata, AppError> {
    let provider_name = get_default_provider(pool).await;
    let provider = get_provider(pool, &provider_name).await?;
    let results = provider.search(query).await?;
    
    if let Some(first) = results.first() {
        if let Some(ids) = &first.provider_ids {
            if let Some(id) = ids.get(&provider_name).and_then(|v| v.as_i64()) {
                return Ok(provider.get_details(&id.to_string(), first.media_type.as_deref()).await?);
            }
        }
        Ok(first.clone())
    } else {
        Err(AppError::NotFound("No results found".into()))
    }
}

/// Search using the default configured provider
pub async fn search(
    query: &str,
    _media_type: Option<&str>,
    pool: &SqlitePool
) -> Result<Vec<NormalizedMetadata>, AppError> {
    let provider_name = get_default_provider(pool).await;
    let provider = get_provider(pool, &provider_name).await?;
    Ok(provider.search(query).await?)
}

/// Fetch by ID using the default configured provider
pub async fn fetch_by_id(
    provider_id: &str,
    _media_type: Option<&str>,
    pool: &SqlitePool
) -> Result<NormalizedMetadata, AppError> {
    let provider_name = get_default_provider(pool).await;
    let provider = get_provider(pool, &provider_name).await?;
    Ok(provider.get_details(provider_id, _media_type).await?)
}

/// Fetch episodes using the default configured provider
pub async fn fetch_episodes(
    series_provider_id: &str,
    season_number: i32,
    pool: &SqlitePool
) -> Result<Vec<EpisodeMetadata>, AppError> {
    let provider_name = get_default_provider(pool).await;
    let provider = get_provider(pool, &provider_name).await?;
    Ok(provider.get_season_episodes(series_provider_id, season_number).await?)
}
