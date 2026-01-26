use crate::providers::traits::MetadataProvider;
use crate::models::metadata::NormalizedMetadata;
use crate::dtos::tmdb::{TmdbResponse, TmdbFullResponse, TmdbSeasonResponse};
use crate::error::AppError;
use async_trait::async_trait;
use serde_json::json;

pub struct TmdbProvider {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch API key from database settings
    pub async fn fetch_api_key(pool: &sqlx::SqlitePool) -> Result<String, crate::error::AppError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'tmdb_api_key'")
            .fetch_optional(pool)
            .await?;
        
        let key = result.map(|r| r.0).ok_or_else(|| {
            crate::error::AppError::BadRequest("TMDB API Key not found in settings".to_string())
        })?;

        if key.trim().is_empty() {
            return Err(crate::error::AppError::BadRequest("TMDB API Key is empty in settings".into()));
        }
        Ok(key)
    }

    fn build_url(&self, endpoint: &str) -> String {
        format!("https://api.themoviedb.org/3/{}", endpoint)
    }
    
    // Helper to keep the existing logic accessible if needed, or used by trait impl
}

#[async_trait]
impl MetadataProvider for TmdbProvider {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedMetadata>, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }
        // Let's use search/multi strictly to find everything.
        let url = self.build_url("search/multi"); 
        
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str()), ("query", query)])
            .send().await.map_err(|e| AppError::External(e.to_string()))?
            .json::<TmdbResponse>().await.map_err(|e| AppError::External(e.to_string()))?;

        let mut results = Vec::new();
        for r in resp.results {
            // Only include movie/tv
            let media_type = r.date.as_ref().map(|_| "movie").unwrap_or("series"); // specific logic or check fields? 
            // TmdbResult has title/name aliasing but not explicit media_type field in the struct I saw earlier? 
            // Actually TmdbFetchResult had type. TmdbResult (search) didn't show type in the DTO file?
            // Need to check TmdbResult DTO in dtos/tmdb.rs later. Assuming it has what we need or we infer.
            
            results.push(NormalizedMetadata {
                title: r.title.clone(),
                year: r.date.clone().map(|d| d.chars().take(4).collect()),
                plot: r.overview.clone(),
                poster_url: r.poster_path.clone().map(|p| format!("https://image.tmdb.org/t/p/w500{}", p)),
                backdrop_url: r.backdrop_path.clone().map(|p| format!("https://image.tmdb.org/t/p/original{}", p)),
                media_type: Some(media_type.to_string()),
                provider_ids: Some(json!({ "tmdb": r.id })),
                genres: None,
                runtime: None,
                rating: None,
            });
        }
        Ok(results)
    }

    async fn get_details(&self, id: &str, media_type: Option<&str>) -> Result<NormalizedMetadata, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }



        // If hint provided, try that specific one. If generic, try movie then tv?
        if let Some(t) = media_type {
            let endpoint = if t == "movie" { "movie" } else { "tv" };
            let url = self.build_url(&format!("{}/{}", endpoint, id));
            let resp = self.client.get(&url)
                .query(&[("api_key", self.api_key.as_str())])
                .send().await.map_err(|e| AppError::External(e.to_string()))?;
            
            if resp.status().is_success() {
                let json: serde_json::Value = resp.json().await.map_err(|e| AppError::External(e.to_string()))?;
                return self.parse_details(json, if endpoint == "movie" { "movie" } else { "series" }, id).await;
            } else if t == "series" || t == "tv" {
                 // return error if explicit type failed
                 return Err(AppError::NotFound("TMDB ID not found for series".to_string()));
            }
            // If movie failed but explicit, return error? Or maybe user meant series?
            // Let's stick to explicit failure if type is known.
             return Err(AppError::NotFound(format!("TMDB ID not found for {}", t)));
        }

        // Fallback: Try Movie, then TV
        let url = self.build_url(&format!("movie/{}", id));
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?;

        if resp.status().is_success() {
             let json: serde_json::Value = resp.json().await.map_err(|e| AppError::External(e.to_string()))?;
             return self.parse_details(json, "movie", id).await;
        }

        // Try TV
        let url_tv = self.build_url(&format!("tv/{}", id));
        let resp_tv = self.client.get(&url_tv)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?;
            
        if resp_tv.status().is_success() {
            let json: serde_json::Value = resp_tv.json().await.map_err(|e| AppError::External(e.to_string()))?;
            return self.parse_details(json, "series", id).await;
        }

        Err(AppError::NotFound("TMDB ID not found".to_string()))
    }

    async fn get_season_episodes(&self, series_id: &str, season_number: i32) -> Result<Vec<crate::models::metadata::EpisodeMetadata>, AppError> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::BadRequest("TMDB API Key not set".into()));
        }
        let url = self.build_url(&format!("tv/{}/season/{}", series_id, season_number));
        let resp = self.client.get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send().await.map_err(|e| AppError::External(e.to_string()))?
            .json::<TmdbSeasonResponse>().await.map_err(|e| AppError::External(e.to_string()))?;
            
        Ok(resp.episodes.into_iter().map(|ep| crate::models::metadata::EpisodeMetadata {
            id: ep.episode_number.to_string(), // TMDB doesn't usually use separate IDs for episodes in this context easily, or we can use episode_number as ID for now within the season
            episode_number: ep.episode_number,
            season_number: season_number,
            name: ep.name,
            overview: ep.overview,
            still_path: ep.still_path.map(|p| format!("https://image.tmdb.org/t/p/w500{}", p)),
            air_date: None, // DTO doesn't have it yet, we can add later if crucial
        }).collect())
    }
}

impl TmdbProvider {
    async fn parse_details(&self, json: serde_json::Value, media_type: &str, id: &str) -> Result<NormalizedMetadata, AppError> {
          let full_info: TmdbFullResponse = serde_json::from_value(json.clone()).unwrap_or(TmdbFullResponse {
            runtime: None,
            episode_run_time: None,
            genres: None,
        });

        let title = json.get("title").or(json.get("name"))
            .and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
        let year = json.get("release_date").or(json.get("first_air_date"))
            .and_then(|v| v.as_str()).map(|d| d.chars().take(4).collect());
        let poster = json.get("poster_path")
            .and_then(|v| v.as_str()).map(|p| format!("https://image.tmdb.org/t/p/w500{}", p));
        let backdrop = json.get("backdrop_path")
            .and_then(|v| v.as_str()).map(|p| format!("https://image.tmdb.org/t/p/original{}", p));
        let plot = json.get("overview")
            .and_then(|v| v.as_str()).map(|s| s.to_string());
            
        let runtime = full_info.runtime.or_else(|| {
            full_info.episode_run_time.as_ref().and_then(|v| v.first().copied())
        });

        let genres = full_info.genres.map(|gs| gs.into_iter().map(|g| g.name).collect());

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
            rating: None,
        })
    }
}
