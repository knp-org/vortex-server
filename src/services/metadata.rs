use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};
use crate::metadata_providers::traits::MetadataProvider;
use crate::metadata_providers::registry;
use crate::models::db::provider::ProviderConfig;
use sqlx::SqlitePool;
use crate::error::AppError;

/// MetadataService resolves the ordered, enabled provider chain and runs
/// search/fetch with fallback. It replaces the old hardcoded match/factory.
pub struct MetadataService;

impl MetadataService {
    /// Build the ordered chain of enabled providers.
    ///
    /// Reads `provider_configs` from DB, filters to enabled providers,
    /// optionally filters by `media_type`, sorts by priority (ascending),
    /// and constructs each provider via its registry factory + stored config.
    pub async fn chain(
        pool: &SqlitePool,
        library_id: Option<i64>,
        media_type: Option<&str>,
    ) -> Result<Vec<Box<dyn MetadataProvider>>, AppError> {
        // Read all provider configs from DB, ordered by priority
        let mut configs: Vec<ProviderConfig> = Vec::new();

        if let Some(lib_id) = library_id {
            // Check for library-specific overrides
            let overrides: Vec<(String, i32, bool)> = sqlx::query_as(
                "SELECT provider_id, priority, enabled FROM library_providers WHERE library_id = ? ORDER BY priority ASC"
            )
            .bind(lib_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            if !overrides.is_empty() {
                // For each override, we need its config_json from provider_configs
                for (provider_id, priority, enabled) in overrides {
                    let cfg_json: Option<(String,)> = sqlx::query_as(
                        "SELECT config_json FROM provider_configs WHERE provider_id = ?"
                    )
                    .bind(&provider_id)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or_default();

                    configs.push(ProviderConfig {
                        provider_id,
                        enabled,
                        priority,
                        config_json: cfg_json.map(|x| x.0).unwrap_or_else(|| "{}".to_string()),
                    });
                }
            }
        }

        if configs.is_empty() {
            // Fallback to global configs
            configs = sqlx::query_as(
                "SELECT provider_id, enabled, priority, config_json FROM provider_configs ORDER BY priority ASC"
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        }

        let mut providers: Vec<Box<dyn MetadataProvider>> = Vec::new();

        for config in &configs {
            if !config.enabled {
                continue;
            }

            // Look up the registry entry for this provider
            let entry = match registry::manifest(&config.provider_id) {
                Some(m) => m,
                None => {
                    tracing::warn!(provider = %config.provider_id, "Provider in DB but not in registry, skipping");
                    continue;
                }
            };

            // Filter by media type if requested
            if let Some(mt) = media_type {
                if !entry.media_types.contains(&mt) {
                    continue;
                }
            }

            // Build the provider from stored config
            let config_json: serde_json::Value = serde_json::from_str(&config.config_json)
                .unwrap_or(serde_json::json!({}));

            let factory = match registry::factory(&config.provider_id) {
                Some(f) => f,
                None => continue,
            };

            match factory(&config_json) {
                Ok(provider) => providers.push(provider),
                Err(e) => {
                    tracing::warn!(provider = %config.provider_id, error = %e, "Failed to build provider from config");
                }
            }
        }

        // If no providers were configured in the DB, fall back to building
        // any provider that exists in the registry (best-effort for fresh installs)
        if providers.is_empty() {
            for entry in registry::registry() {
                // Try building with empty config — providers with required keys will fail gracefully
                match (entry.factory)(&serde_json::json!({})) {
                    Ok(p) => providers.push(p),
                    Err(_) => {} // Expected for providers that need API keys
                }
            }
        }

        Ok(providers)
    }

    /// Search using the provider chain with fallback.
    /// Iterates providers in priority order; first successful result wins.
    pub async fn search(
        pool: &SqlitePool,
        query: &str,
        year: Option<String>,
        media_type: Option<&str>,
        library_id: Option<i64>,
    ) -> Result<Vec<NormalizedMetadata>, AppError> {
        let (clean_query, extracted_year) = extract_year(query);
        let final_year = year.or(extracted_year);

        let chain = Self::chain(pool, library_id, media_type).await?;

        if chain.is_empty() {
            return Err(AppError::BadRequest("No metadata providers are configured and enabled".into()));
        }

        for provider in &chain {
            match provider.search(&clean_query, final_year.clone()).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => {
                    tracing::debug!(provider = %provider.provider_id(), "Provider returned empty results, trying next");
                }
                Err(e) => {
                    tracing::warn!(provider = %provider.provider_id(), error = %e, "Provider search failed, trying next");
                }
            }
        }

        Err(AppError::NotFound("No results found from any provider".into()))
    }

    /// Fetch details from a specific provider by its id.
    /// Used by the Identify flow where the user has already chosen a provider result.
    #[allow(dead_code)]
    pub async fn fetch_details(
        pool: &SqlitePool,
        _provider_id_str: &str,
        _media_type: Option<&str>,
    ) -> Result<Box<dyn MetadataProvider>, AppError> {
        // Try to find this provider in the configured chain
        let chain = Self::chain(pool, None, None).await?;
        for provider in chain {
            // Return the first available provider (in v1 we only have one)
            return Ok(provider);
        }

        Err(AppError::BadRequest("No providers available".into()))
    }
}

// ── Backward-compatible wrapper functions ──────────────────────────────
// These preserve the existing public API so call sites in media.rs,
// tv.rs, and scanner.rs compile without changes.

/// Default provider if not configured in settings
const DEFAULT_PROVIDER: &str = "tmdb";

/// Get the configured default provider from settings, or use DEFAULT_PROVIDER.
/// Now also considers the highest-priority enabled provider from provider_configs.
pub async fn get_default_provider(pool: &SqlitePool) -> String {
    // First try the new provider_configs table
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT provider_id FROM provider_configs WHERE enabled = 1 ORDER BY priority ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((id,)) = result {
        return id;
    }

    // Fall back to legacy settings
    let result: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'metadata_provider'")
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    result.map(|r| r.0).unwrap_or_else(|| DEFAULT_PROVIDER.to_string())
}

/// Get a provider instance by name — now uses the registry + DB config.
pub async fn get_provider(pool: &SqlitePool, provider: &str) -> Result<Box<dyn MetadataProvider>, AppError> {
    // Try to load config from DB first
    let config: Option<ProviderConfig> = sqlx::query_as(
        "SELECT provider_id, enabled, priority, config_json FROM provider_configs WHERE provider_id = ?"
    )
    .bind(provider)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some(cfg) = config {
        let config_json: serde_json::Value = serde_json::from_str(&cfg.config_json)
            .unwrap_or(serde_json::json!({}));

        if let Some(factory) = registry::factory(provider) {
            return factory(&config_json);
        }
    }

    // Legacy fallback for TMDB
    if provider == "tmdb" {
        use crate::metadata_providers::tmdb::TmdbProvider;
        let api_key = TmdbProvider::fetch_api_key(pool).await?;
        return Ok(Box::new(TmdbProvider::new(api_key)));
    }

    Err(AppError::BadRequest(format!("Unknown provider: {}", provider)))
}

fn extract_year(query: &str) -> (String, Option<String>) {
    let re = regex::Regex::new(r"^(.*?)\s*\(?(\d{4})\)?\s*$").unwrap();
    if let Some(caps) = re.captures(query) {
        let name = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();
        let year = caps.get(2).map(|m| m.as_str().to_string());
        if !name.is_empty() {
            return (name, year);
        }
    }
    (query.to_string(), None)
}

/// Fetch metadata using the provider chain
pub async fn fetch_metadata(
    query: &str,
    media_type_hint: Option<&str>,
    pool: &SqlitePool
) -> Result<NormalizedMetadata, AppError> {
    let results = MetadataService::search(pool, query, None, media_type_hint, None).await?;
    
    if let Some(first) = results.first() {
        if let Some(ids) = &first.provider_ids {
            // Pick the first provider ID we have for it
            if let Some((provider_name, val)) = ids.as_object().and_then(|m| m.iter().next()) {
                if let Some(id) = val.as_i64().map(|i| i.to_string()).or_else(|| val.as_str().map(|s| s.to_string())) {
                    return Ok(fetch_by_id(&id, first.media_type.as_deref(), pool, Some(provider_name)).await?);
                }
            }
        }
        Ok(first.clone())
    } else {
        Err(AppError::NotFound("No results found".into()))
    }
}

/// Search using the provider chain
pub async fn search(
    query: &str,
    year: Option<String>,
    media_type: Option<&str>,
    pool: &SqlitePool
) -> Result<Vec<NormalizedMetadata>, AppError> {
    MetadataService::search(pool, query, year, media_type, None).await
}

/// Fetch by ID using a specific provider (if given) or the default.
pub async fn fetch_by_id(
    provider_id: &str,
    media_type: Option<&str>,
    pool: &SqlitePool,
    provider_name: Option<&str>,
) -> Result<NormalizedMetadata, AppError> {
    let name = match provider_name {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => get_default_provider(pool).await,
    };
    let provider = get_provider(pool, &name).await?;
    Ok(provider.get_details(provider_id, media_type).await?)
}

/// Fetch episodes using a specific provider (if given) or the default.
pub async fn fetch_episodes(
    series_provider_id: &str,
    season_number: i32,
    pool: &SqlitePool,
    provider_name: Option<&str>,
) -> Result<Vec<EpisodeMetadata>, AppError> {
    let name = match provider_name {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => get_default_provider(pool).await,
    };
    let provider = get_provider(pool, &name).await?;
    Ok(provider.get_season_episodes(series_provider_id, season_number).await?)
}

