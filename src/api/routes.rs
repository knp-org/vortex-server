use axum::{
    routing::get,
    Router,
    middleware,
};
use sqlx::SqlitePool;
use crate::api::handlers::{
    library::{get_libraries, create_library, delete_library, scan_all_libraries, list_directories, browse_library, scan_library, refresh_library},
    media::{get_recently_added, get_library_media, get_media_details, refresh_media_metadata, search_handler, identify_media, search_library},
    playback::{stream_video, update_progress, get_continue_watching, get_media_progress, get_subtitles, stream_subtitle, get_thumbnail},
    transcode::{get_stream_info, get_hls_playlist, get_hls_segment},
    images::get_image,
    settings::{get_settings, update_setting},
    tv::{get_all_series, get_series_seasons, get_season_episodes, get_series_detail, refresh_series_metadata, identify_series},
};
use crate::api::middleware::auth_middleware;
use crate::infrastructure::logging::request_logging;

pub fn app(pool: SqlitePool) -> Router {
    let public_routes = Router::new()
        .route("/api/v1/auth/login", axum::routing::post(crate::api::handlers::auth::login))
        .route("/api/v1/auth/register", axum::routing::post(crate::api::handlers::auth::register))
        .route("/api/v1/auth/logout", axum::routing::post(crate::api::handlers::auth::logout));

    let protected_routes = Router::new()
        .route("/api/v1/recent", get(get_recently_added))
        .route("/api/v1/directories", axum::routing::post(list_directories))
        .route("/api/v1/stream/:id", get(stream_video).head(stream_video))
        .route("/api/v1/stream/:id/subtitles", get(get_subtitles))
        .route("/api/v1/stream/:id/subtitle/:filename", get(stream_subtitle))
        .route("/api/v1/stream/:id/info", axum::routing::post(get_stream_info))
        .route("/api/v1/stream/:id/hls/master.m3u8", get(get_hls_playlist))
        .route("/api/v1/stream/:id/hls/:segment", get(get_hls_segment))
        .route("/api/v1/images/:filename", get(get_image))
        .route("/api/v1/libraries", get(get_libraries).post(create_library))
        .route("/api/v1/libraries/:id", axum::routing::delete(delete_library).put(crate::api::handlers::library::update_library))
        .route("/api/v1/libraries/:id/media", get(get_library_media))
        .route("/api/v1/libraries/:id/browse", get(browse_library))
        .route("/api/v1/media/:id", get(get_media_details))
        .route("/api/v1/media/:id/thumbnail", get(get_thumbnail))
        .route("/api/v1/media/:id/refresh", axum::routing::post(refresh_media_metadata))
        .route("/api/v1/media/:id/identify", axum::routing::post(identify_media))
        .route("/api/v1/metadata/search", get(search_handler))
        .route("/api/v1/library/search", get(search_library))

        .route("/api/v1/settings", get(get_settings).post(update_setting))
        .route("/api/v1/settings/transcode", get(crate::api::handlers::transcode::get_transcode_settings).post(crate::api::handlers::transcode::update_transcode_settings))
        .route("/api/v1/system/shutdown", axum::routing::post(crate::api::handlers::system::shutdown))
        .route("/api/v1/system/restart", axum::routing::post(crate::api::handlers::system::restart))
        .route("/api/v1/system/clear", axum::routing::post(crate::api::handlers::system::clear_database))
        .route("/api/v1/scan", axum::routing::post(scan_all_libraries))
        .route("/api/v1/libraries/:id/scan", axum::routing::post(scan_library))
        .route("/api/v1/libraries/:id/refresh", axum::routing::post(refresh_library))
        .route("/api/v1/media/:id/progress", get(get_media_progress).post(update_progress))
        .route("/api/v1/continue", get(get_continue_watching))
        // TV Show routes
        .route("/api/v1/series", get(get_all_series))
        .route("/api/v1/series/:name/seasons", get(get_series_seasons))
        .route("/api/v1/series/:name/detail", get(get_series_detail))
        .route("/api/v1/series/:name/refresh", axum::routing::post(refresh_series_metadata))
        .route("/api/v1/series/:name/identify", axum::routing::post(identify_series))
        .route("/api/v1/series/:name/season/:num", get(get_season_episodes))
        .route("/api/v1/auth/me", get(crate::api::handlers::auth::me))
        .route_layer(middleware::from_fn(auth_middleware));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(tower_cookies::CookieManagerLayer::new())
        // Request logging middleware
        .layer(middleware::from_fn(request_logging))
        .with_state(pool)
}

