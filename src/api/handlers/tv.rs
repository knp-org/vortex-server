use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::core::media_service;

use crate::dtos::requests::IdentifyRequest;
use crate::models::tv::{SeriesDto, SeasonDto, EpisodeDto, SeriesDetailDto};

pub async fn get_all_series(State(pool): State<SqlitePool>) -> Result<Json<Vec<SeriesDto>>, AppError> {
    let series_rows: Vec<(String, i32, Option<String>)> = sqlx::query_as(
        "SELECT media.series_name, COUNT(DISTINCT media.season_number) as season_count, 
                (SELECT poster_url FROM media m2 WHERE m2.series_name = media.series_name AND m2.poster_url IS NOT NULL LIMIT 1)
         FROM media 
         JOIN libraries l ON media.library_id = l.id
         WHERE media.series_name IS NOT NULL 
         GROUP BY media.series_name 
         ORDER BY media.series_name ASC"
    )
    .fetch_all(&pool)
    .await?;

    let series: Vec<SeriesDto> = series_rows
        .into_iter()
        .map(|(name, season_count, poster_url)| SeriesDto {
            name,
            season_count,
            poster_url,
        })
        .collect();

    Ok(Json(series))
}

pub async fn get_series_seasons(
    Path(encoded_name): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<SeasonDto>>, AppError> {
    let series_name = urlencoding::decode(&encoded_name)
        .unwrap_or(std::borrow::Cow::Borrowed(&encoded_name))
        .into_owned();

    let season_rows: Vec<(i32, i32, Option<String>)> = sqlx::query_as(
        "SELECT season_number, COUNT(*) as episode_count,
                (SELECT poster_url FROM media m2 WHERE m2.series_name = ? AND m2.season_number = media.season_number AND m2.poster_url IS NOT NULL LIMIT 1)
         FROM media 
         WHERE series_name = ? AND season_number IS NOT NULL
         GROUP BY season_number 
         ORDER BY season_number ASC"
    )
    .bind(&series_name)
    .bind(&series_name)
    .fetch_all(&pool)
    .await?;

    let seasons: Vec<SeasonDto> = season_rows
        .into_iter()
        .map(|(season_number, episode_count, poster_url)| SeasonDto {
            season_number,
            episode_count,
            poster_url,
        })
        .collect();

    Ok(Json(seasons))
}

pub async fn get_season_episodes(
    Path((encoded_name, season_number)): Path<(String, i32)>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<EpisodeDto>>, AppError> {
    let series_name = urlencoding::decode(&encoded_name)
        .unwrap_or(std::borrow::Cow::Borrowed(&encoded_name))
        .into_owned();

    let episode_rows: Vec<(i64, Option<String>, Option<i32>, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, title, episode_number, still_url, file_path, plot 
        FROM media 
        WHERE series_name = ? AND season_number = ?
        ORDER BY episode_number ASC"
    )
    .bind(&series_name)
    .bind(season_number)
    .fetch_all(&pool)
    .await?;

    let episodes: Vec<EpisodeDto> = episode_rows
        .into_iter()
        .map(|(id, title, episode_number, still_url, file_path, plot)| EpisodeDto {
            id,
            title,
            episode_number: episode_number.unwrap_or(0),
            poster_url: still_url,
            file_path,
            plot,
        })
        .collect();

    Ok(Json(episodes))
}


pub async fn get_series_detail(
    Path(encoded_name): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetailDto>, AppError> {
    let series_name = urlencoding::decode(&encoded_name)
        .unwrap_or(std::borrow::Cow::Borrowed(&encoded_name))
        .into_owned();
    
    tracing::info!("Fetching details for series: '{}'", series_name);

    let series_info: Option<(Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT poster_url, backdrop_url, plot, year, genres FROM media WHERE series_name = ? AND poster_url IS NOT NULL LIMIT 1"
    )
    .bind(&series_name)
    .fetch_optional(&pool)
    .await?;

    let season_rows: Vec<(i32, i32, Option<String>)> = sqlx::query_as(
        "SELECT season_number, COUNT(*) as episode_count,
                (SELECT poster_url FROM media m2 WHERE m2.series_name = ? AND m2.season_number = media.season_number AND m2.poster_url IS NOT NULL LIMIT 1)
         FROM media 
         WHERE series_name = ? AND season_number IS NOT NULL
         GROUP BY season_number 
         ORDER BY season_number ASC"
    )
    .bind(&series_name)
    .bind(&series_name)
    .fetch_all(&pool)
    .await?;

    let seasons: Vec<SeasonDto> = season_rows
        .into_iter()
        .map(|(season_number, episode_count, poster_url)| SeasonDto {
            season_number,
            episode_count,
            poster_url,
        })
        .collect();

    let (poster_url, backdrop_url, plot, year, genres) = series_info.unwrap_or((None, None, None, None, None));

    Ok(Json(SeriesDetailDto {
        name: series_name,
        poster_url,
        backdrop_url,
        plot,
        year,
        genres,
        seasons,
    }))
}

pub async fn refresh_series_metadata(
    Path(encoded_name): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetailDto>, AppError> {
    use crate::core::metadata::{fetch_metadata, fetch_episodes, get_default_provider};
    
    let series_name = urlencoding::decode(&encoded_name)
        .unwrap_or(std::borrow::Cow::Borrowed(&encoded_name))
        .into_owned();

    let meta = fetch_metadata(&series_name, Some("series"), &pool).await
        .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?;

    media_service::update_series_metadata(&pool, &series_name, &meta).await?;
    
    // Get the provider name to look up the correct ID
    let provider_name = get_default_provider(&pool).await;
    let provider_id = meta.provider_ids.as_ref()
        .and_then(|ids| ids.get(&provider_name))
        .and_then(|v| {
            // Handle both string and number types
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if let Some(i) = v.as_i64() {
                Some(i.to_string())
            } else {
                None
            }
        });

    if let Some(id_str) = provider_id {
        let seasons = media_service::get_series_seasons(&pool, &series_name).await?;
        
        for season_num in seasons {
            if let Ok(episodes) = fetch_episodes(&id_str, season_num, &pool).await {
                for ep in episodes {
                    let still_url = ep.still_path.clone();
                    let _ = media_service::update_episode_details(
                        &pool,
                        &series_name,
                        season_num,
                        ep.episode_number,
                        &ep.name,
                        &ep.overview,
                        still_url,
                    ).await;
                }
            }
        }
    }
    
    get_series_detail(Path(encoded_name), State(pool)).await
}


pub async fn identify_series(
    State(pool): State<SqlitePool>,
    Path(encoded_name): Path<String>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<SeriesDetailDto>, AppError> {
    use crate::core::metadata::{fetch_by_id, fetch_episodes};

    let series_name = urlencoding::decode(&encoded_name)
        .unwrap_or(std::borrow::Cow::Borrowed(&encoded_name))
        .into_owned();

    let media_type = payload.media_type.as_deref().or(Some("series"));
    let meta = fetch_by_id(&payload.provider_id, media_type, &pool).await
        .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?;

    media_service::update_series_metadata(&pool, &series_name, &meta).await?;
    
    // Use the provider ID from the payload directly
    let provider_id = payload.provider_id;
    let seasons = media_service::get_series_seasons(&pool, &series_name).await?;

    for season_num in seasons {
        if let Ok(episodes) = fetch_episodes(&provider_id, season_num, &pool).await {
            for ep in episodes {
                let still_url = ep.still_path.clone();
                let _ = media_service::update_episode_details(
                    &pool,
                    &series_name,
                    season_num,
                    ep.episode_number,
                    &ep.name,
                    &ep.overview,
                    still_url,
                ).await;
            }
        }
    }

    get_series_detail(Path(encoded_name), State(pool)).await
}
