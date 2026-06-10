//! TMDB Provider Implementation
//!
//! Fetches metadata from The Movie Database (TMDB) API.

use crate::metadata_providers::manifest::{ProviderManifest, ConfigField, FieldType};
use crate::metadata_providers::traits::MetadataProvider;
use crate::models::metadata::{NormalizedMetadata, CastMember, EpisodeMetadata};
use super::types::{TmdbResponse, TmdbFullResponse, TmdbSeasonResponse};
use crate::error::AppError;
use async_trait::async_trait;
use serde_json::json;

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

/// TMDB metadata provider
pub struct TmdbProvider {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbProvider {
    /// Create a new TMDB provider with API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch API key from database settings (back-compat path)
    pub async fn fetch_api_key(pool: &sqlx::SqlitePool) -> Result<String, AppError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'tmdb_api_key'")
            .fetch_optional(pool)
            .await?;
        
        let key = result.map(|r| r.0).ok_or_else(|| {
            AppError::BadRequest("TMDB API Key not found in settings".to_string())
        })?;

        if key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key is empty in settings".into()));
        }
        Ok(key)
    }

    /// Static manifest describing this provider's identity and config schema.
    pub fn provider_manifest() -> ProviderManifest {
        ProviderManifest {
            id: "tmdb",
            name: "The Movie Database",
            description: "Fetches movie and TV metadata, artwork, and cast information from TMDB.",
            media_types: &["movie", "series"],
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
                    key: "language",
                    label: "Language",
                    field_type: FieldType::Select,
                    required: false,
                    default: Some(json!("en")),
                    options: Some(vec![
                        ("en", "English"),
                        ("es", "Spanish"),
                        ("fr", "French"),
                        ("de", "German"),
                        ("ja", "Japanese"),
                        ("ko", "Korean"),
                        ("zh", "Chinese"),
                        ("pt", "Portuguese"),
                        ("it", "Italian"),
                        ("ru", "Russian"),
                        ("hi", "Hindi"),
                    ]),
                },
            ],
        }
    }

    /// Build a TmdbProvider from a JSON config object.
    /// Expected keys: `api_key` (required).
    pub fn from_config(config: &serde_json::Value) -> Result<Box<dyn MetadataProvider>, AppError> {
        let api_key = config.get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key is required".into()));
        }

        Ok(Box::new(TmdbProvider::new(api_key)))
    }

    /// Build TMDB API URL
    fn build_url(&self, endpoint: &str) -> String {
        format!("{}/{}", TMDB_BASE_URL, endpoint)
    }

    /// Download and cache an image, returning the local path
    async fn download_image(&self, path: &str, size: &str) -> Option<String> {
        let url = format!("{}/{}{}", TMDB_IMAGE_BASE, size, path);
        match crate::services::images::download_image(&url).await {
            Ok(Some(filename)) => Some(format!("/api/v1/images/{}", filename)),
            _ => Some(url), // Fallback to remote URL
        }
    }

    /// Parse TMDB details response into normalized metadata
    async fn parse_details(&self, json: serde_json::Value, media_type: &str, id: &str) -> Result<NormalizedMetadata, AppError> {
        let full_info: TmdbFullResponse = serde_json::from_value(json.clone()).unwrap_or(TmdbFullResponse {
            runtime: None,
            episode_run_time: None,
            genres: None,
            vote_average: None,
            tagline: None,
            status: None,
            original_language: None,
            popularity: None,
            budget: None,
            revenue: None,
            homepage: None,
            imdb_id: None,
            content_ratings: None,
            release_dates: None,
            networks: None,
            production_companies: None,
            videos: None,
            origin_country: None,
            belongs_to_collection: None,
            created_by: None,
            credits: None,
            aggregate_credits: None,
        });

        let title = json.get("title").or(json.get("name"))
            .and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
        let year = json.get("release_date").or(json.get("first_air_date"))
            .and_then(|v| v.as_str()).map(|d| d.chars().take(4).collect());
        
        let poster = match json.get("poster_path").and_then(|v| v.as_str()) {
            Some(p) => self.download_image(p, "w500").await,
            None => None,
        };

        let backdrop = match json.get("backdrop_path").and_then(|v| v.as_str()) {
            Some(p) => self.download_image(p, "original").await,
            None => None,
        };

        let plot = json.get("overview")
            .and_then(|v| v.as_str()).map(|s| s.to_string());
            
        let runtime = full_info.runtime.or_else(|| {
            full_info.episode_run_time.as_ref().and_then(|v| v.first().copied())
        });

        let genres = full_info.genres.map(|gs| gs.into_iter().map(|g| g.name).collect());

        // Process Cast
        // Process Cast
        let mut cast_members: Vec<CastMember> = Vec::new();
        
        let cast_source = full_info.aggregate_credits.as_ref().map(|c| &c.cast)
            .or(full_info.credits.as_ref().map(|c| &c.cast));
        
        if let Some(cast_list) = cast_source {
            for c in cast_list.iter().take(10) {
                let profile_url = match &c.profile_path {
                    Some(p) => self.download_image(p, "w185").await,
                    None => None,
                };

                cast_members.push(CastMember {
                    name: c.name.clone(),
                    character: c.character.clone().unwrap_or_default(),
                    role: "actor".to_string(),
                    profile_url,
                    order: c.order.unwrap_or(0),
                });
            }
        }
        
        let cast = if cast_members.is_empty() { None } else { Some(cast_members) };

        // Process Crew (Director)
        let mut directors = Vec::new();

        // 1. Check created_by (TV Series Creators) - Primary source for TV
        if let Some(creators) = &full_info.created_by {
            for creator in creators {
                directors.push(creator.name.clone());
            }
        }

        // 2. Check movie credits
        if let Some(credits) = &full_info.credits {
            for crew_member in &credits.crew {
                if crew_member.job == "Director" {
                     if !directors.contains(&crew_member.name) {
                        directors.push(crew_member.name.clone());
                     }
                }
            }
        }

        // 3. Check TV aggregate credits
        if let Some(credits) = &full_info.aggregate_credits {
            for crew_member in &credits.crew {
                for job in &crew_member.jobs {
                    if job.job == "Director" || job.job == "Series Director" {
                         if !directors.contains(&crew_member.name) {
                            directors.push(crew_member.name.clone());
                         }
                    }
                }
            }
        }
        let mut creators_list = Vec::new();
        if let Some(creators) = &full_info.created_by {
            for creator in creators {
                creators_list.push(creator.name.clone());
            }
        }
        let creator = if creators_list.is_empty() { None } else { Some(creators_list) };

        // Process Age Rating
        let mut age_rating = None;
        if let Some(content_ratings) = &full_info.content_ratings {
            for rating in &content_ratings.results {
                if rating.iso_3166_1 == "US" {
                    age_rating = Some(rating.rating.clone());
                    break;
                }
            }
        } else if let Some(release_dates) = &full_info.release_dates {
            for release in &release_dates.results {
                if release.iso_3166_1 == "US" {
                    if let Some(cert) = release.release_dates.first() {
                        age_rating = Some(cert.certification.clone());
                    }
                    break;
                }
            }
        }
        if age_rating.as_deref() == Some("") {
            age_rating = None;
        }

        // Process Studio / Network
        let mut studio = None;
        if let Some(networks) = &full_info.networks {
            if let Some(network) = networks.first() {
                studio = Some(network.name.clone());
            }
        }
        if studio.is_none() {
            if let Some(companies) = &full_info.production_companies {
                if let Some(company) = companies.first() {
                    studio = Some(company.name.clone());
                }
            }
        }

        // Process Trailer URL
        let mut trailer_url = None;
        if let Some(videos) = &full_info.videos {
            for video in &videos.results {
                if video.video_type == "Trailer" && video.site == "YouTube" {
                    trailer_url = Some(format!("https://www.youtube.com/watch?v={}", video.key));
                    break;
                }
            }
        }

        // Origin Country
        let origin_country = full_info.origin_country.as_ref().and_then(|c| c.first().cloned());

        // Collection Name
        let collection_name = full_info.belongs_to_collection.as_ref().map(|c| c.name.clone());

        let director = if directors.is_empty() { None } else { Some(directors) };

        Ok(NormalizedMetadata {
            title,
            year,
            plot,
            poster_url: poster,
            backdrop_url: backdrop,
            media_type: Some(media_type.to_string()),
            provider_ids: Some(json!({ "tmdb": id.parse::<i64>().unwrap_or(0) })),
            genres,
            runtime,
            rating: full_info.vote_average,
            cast,
            director,
            tagline: full_info.tagline,
            status: full_info.status,
            original_language: full_info.original_language,
            popularity: full_info.popularity,
            budget: full_info.budget,
            revenue: full_info.revenue,
            homepage: full_info.homepage,
            imdb_id: full_info.imdb_id,
            age_rating,
            studio,
            trailer_url,
            origin_country,
            collection_name,
            creator,
            tags: None,
        })
    }
}

#[async_trait]
impl MetadataProvider for TmdbProvider {
    fn provider_id(&self) -> &'static str {
        "tmdb"
    }

    async fn health_check(&self) -> Result<(), AppError> {
        let url = self.build_url("configuration");
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| {
                AppError::External(format!("TMDB connection failed: {}", e))
            })?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::AuthError("Invalid TMDB API Key".into()));
        }
        if !resp.status().is_success() {
            return Err(AppError::External(format!("TMDB health check failed: {}", resp.status())));
        }
        Ok(())
    }

    async fn search(&self, query: &str, year: Option<String>) -> Result<Vec<NormalizedMetadata>, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }
        
        let url = self.build_url("search/multi"); 
        
        let mut req = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str()), ("query", query)]);
            
        if let Some(y) = &year {
            req = req.query(&[("year", y.as_str()), ("first_air_date_year", y.as_str())]);
        }
        
        tracing::info!(
            provider = "tmdb",
            endpoint = "search/multi",
            search_query = query,
            year = ?year,
            "Sending TMDB search API request"
        );
        
        let resp = req.send().await.map_err(|e| {
                tracing::error!("Failed to send TMDB search request: {}", e);
                AppError::External(e.to_string())
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
            tracing::error!("TMDB API Error. Status: {}, Body: {}", status, body);

            if status == reqwest::StatusCode::UNAUTHORIZED {
                 return Err(AppError::AuthError("Invalid TMDB API Key".to_string()));
            }
            return Err(AppError::External(format!("TMDB API Error: {} - {}", status, body)));
        }

        let body = resp.text().await.map_err(|e| AppError::External(e.to_string()))?;
        let resp: TmdbResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to parse TMDB search response: {}. Body: {}", e, body);
                return Err(AppError::External(format!("TMDB Parse Error: {}", e)));
            }
        };

        let mut results = Vec::new();
        for r in resp.results {
            let raw_type = r.media_type.as_deref().unwrap_or("unknown");
            let media_type = if raw_type == "tv" { "series" } else { "movie" };
            
            if raw_type != "tv" && raw_type != "movie" { continue; }

            let poster_url = match r.poster_path {
                Some(p) => Some(format!("{}/w500{}", TMDB_IMAGE_BASE, p)),
                None => None,
            };
            let backdrop_url = match r.backdrop_path {
                Some(p) => Some(format!("{}/original{}", TMDB_IMAGE_BASE, p)),
                None => None,
            };

            results.push(NormalizedMetadata {
                title: r.title.clone(),
                year: r.date.clone().map(|d| d.chars().take(4).collect()),
                plot: r.overview.clone(),
                poster_url,
                backdrop_url,
                media_type: Some(media_type.to_string()),
                provider_ids: Some(json!({ "tmdb": r.id })),
                genres: None,
                runtime: None,
                rating: r.vote_average,
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
                age_rating: None,
                studio: None,
                trailer_url: None,
                origin_country: None,
                collection_name: None,
                creator: None,
                tags: None,
            });
        }
        Ok(results)
    }

    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }

        if let Some(t) = media_type {
            let endpoint = if t == "movie" { "movie" } else { "tv" };
            let append = if t == "movie" { "credits" } else { "aggregate_credits" };
            let url = self.build_url(&format!("{}/{}?append_to_response={}", endpoint, id, append));
            let resp = self.client.get(&url)
                .query(&[("api_key", self.api_key.as_str())])
                .send().await.map_err(|e| {
                    tracing::error!("Failed to send TMDB details request: {}", e);
                    AppError::External(e.to_string())
                })?;
            
            if !resp.status().is_success() {
                let status = resp.status();
                if status == reqwest::StatusCode::NOT_FOUND {
                    return Err(AppError::NotFound(format!("TMDB ID {} not found", id)));
                }
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(AppError::AuthError("Invalid TMDB API Key".to_string()));
                }
                let body = resp.text().await.unwrap_or_default();
                tracing::error!("TMDB fetch details error: {} - {}", status, body);
                return Err(AppError::External(format!("TMDB API Error: {}", status)));
            }
            
            let body = resp.text().await.map_err(|e| AppError::External(e.to_string()))?;
            let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                tracing::error!("Failed to parse details JSON: {}. Body: {}", e, body);
                AppError::External(format!("TMDB Parse Error: {}", e))
            })?;
            return self.parse_details(json, if endpoint == "movie" { "movie" } else { "series" }, id).await;
        }

        // Fallback: Try Movie, then TV
        let url = self.build_url(&format!("movie/{}?append_to_response=credits", id));
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(|e| AppError::External(e.to_string()))?;
            return self.parse_details(json, "movie", id).await;
        }

        // Try TV
        let url_tv = self.build_url(&format!("tv/{}?append_to_response=credits", id));
        let resp_tv = self.client.get(&url_tv)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?;
            
        if resp_tv.status().is_success() {
            let json: serde_json::Value = resp_tv.json().await.map_err(|e| AppError::External(e.to_string()))?;
            return self.parse_details(json, "series", id).await;
        }

        Err(AppError::NotFound("TMDB ID not found".to_string()))
    }

    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<EpisodeMetadata>, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }
        
        let url = self.build_url(&format!("tv/{}/season/{}", series_id, season_number));
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?;

        if !resp.status().is_success() {
             let status = resp.status();
             if status == reqwest::StatusCode::NOT_FOUND {
                 // Season not found, return empty list instead of error? OR specific error.
                 return Err(AppError::NotFound(format!("Season {} not found", season_number)));
             }
             if status == reqwest::StatusCode::UNAUTHORIZED {
                 return Err(AppError::AuthError("Invalid TMDB API Key".to_string()));
             }
             return Err(AppError::External(format!("TMDB API Error: {}", status)));
        }

        let resp = resp.json::<TmdbSeasonResponse>().await.map_err(|e| AppError::External(e.to_string()))?;
            
        let mut episodes = Vec::new();
        for ep in resp.episodes {
            let still_path = match ep.still_path {
                Some(p) => self.download_image(&p, "w500").await,
                None => None,
            };

            episodes.push(EpisodeMetadata {
                id: ep.episode_number.to_string(),
                episode_number: ep.episode_number,
                season_number,
                name: ep.name,
                overview: ep.overview,
                still_path,
                air_date: None,
            });
        }
            
        Ok(episodes)
    }
}
