use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use serde_json::json;

use crate::error::AppError;
use crate::metadata_providers::manifest::{ConfigField, FieldType, ProviderManifest};
use crate::metadata_providers::traits::MetadataProvider;
use crate::models::metadata::{CastMember, EpisodeMetadata, NormalizedMetadata};

use super::types::{TvdbEpisodesResponse, TvdbLoginResponse, TvdbSearchResponse,
                   TvdbSeriesResponse, TvdbTranslationResponse};

const TVDB_API_URL: &str = "https://api4.thetvdb.com/v4";
const TVDB_ARTWORK_URL: &str = "https://artworks.thetvdb.com";

pub struct TvdbProvider {
    client: reqwest::Client,
    api_key: String,
    pin: String,
    language: String,
    token: Arc<RwLock<Option<String>>>,
}

impl TvdbProvider {
    pub fn new(api_key: String, pin: String, language: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            pin,
            language,
            token: Arc::new(RwLock::new(None)),
        }
    }

    pub fn provider_manifest() -> ProviderManifest {
        ProviderManifest {
            id: "tvdb",
            name: "TheTVDB",
            description: "Fetches TV show metadata and episodes from TheTVDB (API v4).",
            media_types: &["series", "movie"],
            requires_api_key: true,
            config_schema: vec![
                ConfigField {
                    key: "api_key",
                    label: "API Key",
                    field_type: FieldType::Secret,
                    required: true,
                    default: None,
                    options: None,
                },
                ConfigField {
                    key: "pin",
                    label: "PIN (Optional)",
                    field_type: FieldType::Secret,
                    required: false,
                    default: None,
                    options: None,
                },
                ConfigField {
                    key: "language",
                    label: "Language",
                    field_type: FieldType::Select,
                    required: false,
                    default: Some(json!("eng")),
                    options: Some(vec![
                        ("eng", "English"),
                        ("spa", "Spanish"),
                        ("fra", "French"),
                        ("deu", "German"),
                        ("ita", "Italian"),
                        ("jpn", "Japanese"),
                        ("kor", "Korean"),
                        ("zho", "Chinese"),
                        ("por", "Portuguese"),
                        ("rus", "Russian"),
                        ("hin", "Hindi"),
                        ("ara", "Arabic"),
                        ("tha", "Thai"),
                        ("tur", "Turkish"),
                    ]),
                },
            ],
        }
    }

    pub fn from_config(config: &serde_json::Value) -> Result<Box<dyn MetadataProvider>, AppError> {
        let api_key = config.get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TVDB API Key is required".into()));
        }

        let pin = config.get("pin")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let language = config.get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("eng")
            .to_string();

        Ok(Box::new(TvdbProvider::new(api_key, pin, language)))
    }

    async fn get_token(&self) -> Result<String, AppError> {
        // Check cache first
        {
            let cache = self.token.read().await;
            if let Some(t) = cache.as_ref() {
                return Ok(t.clone());
            }
        }

        // Fetch new token
        let body = if self.pin.is_empty() {
            json!({ "apikey": self.api_key })
        } else {
            json!({ "apikey": self.api_key, "pin": self.pin })
        };

        let resp = self.client.post(format!("{}/login", TVDB_API_URL))
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::External(format!("TVDB login failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::AuthError(format!("TVDB authentication failed: {}", resp.status())));
        }

        let data: TvdbLoginResponse = resp.json().await
            .map_err(|e| AppError::External(format!("Failed to parse TVDB login response: {}", e)))?;

        // Cache it
        let mut cache = self.token.write().await;
        *cache = Some(data.data.token.clone());

        Ok(data.data.token)
    }

    async fn get_request(&self, endpoint: &str, query: Option<&[(&str, &str)]>, year: Option<&str>) -> Result<reqwest::Response, AppError> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", TVDB_API_URL, endpoint);
        
        let mut req = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept-Language", &self.language);
            
        if let Some(q) = query {
            req = req.query(q);
        }
        if let Some(y) = year {
            req = req.query(&[("year", y)]);
        }

        tracing::info!(
            provider = "tvdb",
            endpoint = endpoint,
            query_params = ?query,
            year = ?year,
            "Sending TVDB API request"
        );

        let resp = req.send().await
            .map_err(|e| AppError::External(format!("TVDB request failed: {}", e)))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            // Token might have expired, clear it
            let mut cache = self.token.write().await;
            *cache = None;
            return Err(AppError::AuthError("TVDB token expired".into()));
        }

        if !resp.status().is_success() {
            return Err(AppError::External(format!("TVDB API error: {}", resp.status())));
        }

        Ok(resp)
    }

    /// Fetch translation for a series or movie from the dedicated translations endpoint.
    /// TVDB v4 API: GET /series/{id}/translations/{language}
    ///              GET /movies/{id}/translations/{language}
    async fn fetch_translation(&self, id: &str, media_type: Option<&str>) -> (Option<String>, Option<String>) {
        let type_prefix = match media_type {
            Some("movie") => "movies",
            _ => "series",
        };

        let endpoint = format!("{}/{}/translations/{}", type_prefix, id, self.language);
        
        match self.get_request(&endpoint, None, None).await {
            Ok(resp) => {
                match resp.json::<TvdbTranslationResponse>().await {
                    Ok(trans) => {
                        if let Some(data) = trans.data {
                            tracing::debug!(
                                language = %self.language,
                                name = ?data.name,
                                "TVDB translation fetched"
                            );
                            return (data.name, data.overview);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse TVDB translation response: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch TVDB translation for {}/{}: {}", type_prefix, id, e);
            }
        }
        
        (None, None)
    }

    async fn download_image(&self, path: Option<String>) -> Option<String> {
        let mut url = path?;
        if url.starts_with('/') {
            url = format!("{}{}", TVDB_ARTWORK_URL, url);
        }
        match crate::services::images::download_image(&url).await {
            Ok(Some(filename)) => Some(format!("/api/v1/images/{}", filename)),
            Ok(None) => Some(url),
            Err(e) => {
                tracing::warn!("Failed to download TVDB image {}: {}", url, e);
                Some(url)
            }
        }
    }
}

#[async_trait]
impl MetadataProvider for TvdbProvider {
    fn provider_id(&self) -> &'static str {
        "tvdb"
    }

    async fn health_check(&self) -> Result<(), AppError> {
        self.get_token().await.map(|_| ())
    }

    async fn search(&self, query: &str, year: Option<String>) -> Result<Vec<NormalizedMetadata>, AppError> {
        let resp = self.get_request("search", Some(&[("query", query)]), year.as_deref()).await?;
        let result: TvdbSearchResponse = resp.json().await
            .map_err(|e| AppError::External(format!("Failed to parse TVDB search response: {}", e)))?;

        let mut out = Vec::new();
        if let Some(data) = result.data {
            for item in data {
                let mut provider_ids = serde_json::Map::new();
                provider_ids.insert("tvdb".to_string(), json!(item.tvdb_id));

                // Search results use Accept-Language header which is already set,
                // so the name/overview should come back translated if available.
                out.push(NormalizedMetadata {
                    title: item.name.unwrap_or_default(),
                    year: item.year,
                    plot: item.overview,
                    poster_url: self.download_image(item.image_url).await,
                    backdrop_url: None,
                    media_type: item.item_type,
                    provider_ids: Some(serde_json::Value::Object(provider_ids)),
                    genres: None,
                    runtime: None,
                    rating: None,
                    cast: None,
                    director: None,
                    tagline: None,
                    status: None,
                    original_language: None,
                    popularity: None,
                    budget: None,
                    revenue: None,
                    homepage: None,
                    imdb_id: None,
                });
            }
        }
        
        Ok(out)
    }

    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError> {
        let endpoint = match media_type {
            Some("movie") => format!("movies/{}/extended", id),
            _ => format!("series/{}/extended", id),
        };

        let resp = self.get_request(&endpoint, None, None).await?;
        let result: TvdbSeriesResponse = resp.json().await
            .map_err(|e| AppError::External(format!("Failed to parse TVDB details response: {}", e)))?;

        let mut provider_ids = serde_json::Map::new();
        provider_ids.insert("tvdb".to_string(), json!(result.data.id));

        let mut cast_members = Vec::new();
        if let Some(chars) = result.data.characters {
            for c in chars.into_iter().take(15) {
                let profile_url = if let Some(img) = &c.image {
                    self.download_image(Some(img.clone())).await
                } else {
                    None
                };

                cast_members.push(CastMember {
                    name: c.people_name.unwrap_or_default(),
                    character: c.name.unwrap_or_default(),
                    role: "actor".to_string(),
                    profile_url,
                    order: 0,
                });
            }
        }
        let cast = if cast_members.is_empty() { None } else { Some(cast_members) };

        // Fetch translated name and overview from the dedicated translations endpoint
        let (trans_name, trans_overview) = self.fetch_translation(id, media_type).await;

        // Use translated values if available, otherwise fall back to original
        let title = trans_name
            .filter(|s| !s.is_empty())
            .or(result.data.name)
            .unwrap_or_default();
        let plot = trans_overview
            .filter(|s| !s.is_empty())
            .or(result.data.overview);

        // TVDB API v4: Type 3 is background/backdrop for Series. Type 15 is background for Movies.
        let backdrop_url = result.data.artworks
            .unwrap_or_default()
            .into_iter()
            .find(|a| (a.artwork_type == 3 || a.artwork_type == 15) && a.image.is_some())
            .and_then(|a| a.image);

        let poster_url = self.download_image(result.data.image).await;
        let backdrop_url = self.download_image(backdrop_url).await;

        Ok(NormalizedMetadata {
            title,
            year: result.data.first_aired.and_then(|s| s.split('-').next().map(|y| y.to_string())),
            plot,
            poster_url,
            backdrop_url,
            media_type: media_type.map(|s| s.to_string()),
            provider_ids: Some(serde_json::Value::Object(provider_ids)),
            genres: None,
            runtime: None,
            rating: None,
            cast,
            director: None,
            tagline: None,
            status: None,
            original_language: None,
            popularity: None,
            budget: None,
            revenue: None,
            homepage: None,
            imdb_id: None,
        })
    }

    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<EpisodeMetadata>, AppError> {
        // Use the language-aware episodes endpoint: /series/{id}/episodes/{season-type}/{lang}
        let endpoint = format!("series/{}/episodes/default/{}", series_id, self.language);
        let season_num_str = season_number.to_string();
        let resp = self.get_request(&endpoint, Some(&[("season", &season_num_str)]), None).await?;
        let json_value: serde_json::Value = resp.json().await
            .map_err(|e| AppError::External(format!("Failed to parse TVDB episodes JSON: {}", e)))?;
        tracing::debug!(endpoint = %endpoint, "TVDB episodes response: {}", json_value);
        let result: TvdbEpisodesResponse = serde_json::from_value(json_value)
            .map_err(|e| AppError::External(format!("Failed to parse TVDB episodes response: {}", e)))?;

        let mut out = Vec::new();
        if let Some(episodes) = result.data.episodes {
            for ep in episodes {
                if ep.season_number == season_number {
                    let still_path = self.download_image(ep.image).await;
                    out.push(EpisodeMetadata {
                        id: ep.id.to_string(),
                        episode_number: ep.number,
                        season_number: ep.season_number,
                        name: ep.name.unwrap_or_default(),
                        overview: ep.overview.unwrap_or_default(),
                        still_path,
                        air_date: ep.aired,
                    });
                }
            }
        }

        Ok(out)
    }
}
