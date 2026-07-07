use axum::{
    routing::get,
    Router,
    middleware,
    extract::FromRef,
};
use tower_http::cors::CorsLayer;
use sqlx::SqlitePool;
use crate::api::handlers::{
    library::{get_libraries, create_library, delete_library, scan_all_libraries, list_directories, browse_library, scan_library, refresh_library, get_library_providers, update_library_providers},
    media::{get_recently_added, get_library_media, get_media_details, refresh_media_metadata, search_handler, identify_media, search_library, get_track_lyrics},
    playback::{stream_video, update_progress, get_continue_watching, get_media_progress, get_subtitles, stream_subtitle, stream_embedded_subtitle, get_audio_tracks, get_thumbnail},
    transcode::{get_stream_info, get_hls_playlist, get_hls_segment},
    images::{get_image, get_image_file, update_image},
    galleries::{list_galleries, get_gallery, create_gallery, update_gallery, delete_gallery, add_images, remove_image, list_library_images, list_trash, restore_image, purge_image, empty_trash},
    settings::{get_settings, update_setting},
    series::{get_all_series, get_series_seasons, get_season_episodes, get_series_detail, refresh_series_metadata, identify_series},
    book_series::{get_book_series_detail, get_book_series_chapters},
    providers::{list_providers, get_provider_config, update_provider_config, toggle_provider, reorder_providers, test_provider},
};
use crate::api::middleware::auth_middleware;
use crate::infrastructure::logging::request_logging;
use crate::services::transcode::TranscodeService;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub transcode: TranscodeService,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

pub fn app(pool: SqlitePool) -> Router {
    let transcode = TranscodeService::new(pool.clone());
    transcode.spawn_maintenance_task();

    let state = AppState {
        transcode,
        pool,
    };
    let public_routes = Router::new()
        .route("/api/v1/auth/login", axum::routing::post(crate::api::handlers::auth::login))
        // First-run bootstrap: create the initial admin (only valid when no users exist).
        .route("/api/v1/auth/setup-status", get(crate::api::handlers::auth::setup_status))
        .route("/api/v1/auth/setup", axum::routing::post(crate::api::handlers::auth::setup))
        .route("/api/v1/auth/logout", axum::routing::post(crate::api::handlers::auth::logout))
        .route("/api/v1/images/:filename", get(get_image))
        .route("/api/v1/media/:id/thumbnail", get(get_thumbnail));

    let protected_routes = Router::new()
        .route("/api/v1/recent", get(get_recently_added))
        .route("/api/v1/directories", axum::routing::post(list_directories))
        .route("/api/v1/stream/:id", get(stream_video).head(stream_video))
        .route("/api/v1/stream/:id/subtitles", get(get_subtitles))
        .route("/api/v1/stream/:id/audio_tracks", get(get_audio_tracks))
        .route("/api/v1/stream/:id/mediainfo", get(crate::api::handlers::playback::get_media_info))
        .route("/api/v1/stream/:id/subtitle/embedded/:index", get(stream_embedded_subtitle))
        .route("/api/v1/stream/:id/subtitle/:filename", get(stream_subtitle))
        .route("/api/v1/stream/:id/info", axum::routing::post(get_stream_info))
        .route("/api/v1/stream/:id/hls/master.m3u8", get(get_hls_playlist))
        .route("/api/v1/stream/:id/hls/:segment", get(get_hls_segment))
        // .route("/api/v1/images/:filename", get(get_image)) - Moved to public
        .route("/api/v1/libraries", get(get_libraries).post(create_library))
        .route("/api/v1/libraries/:id", axum::routing::delete(delete_library).put(crate::api::handlers::library::update_library))
        .route("/api/v1/libraries/:id/media", get(get_library_media))
        .route("/api/v1/libraries/:id/tracks", get(crate::api::handlers::media::get_library_tracks))
        .route("/api/v1/libraries/:id/browse", get(browse_library))
        .route("/api/v1/libraries/:id/providers", get(get_library_providers).put(update_library_providers))
        .route("/api/v1/media/:id", get(get_media_details))
        // .route("/api/v1/media/:id/thumbnail", get(get_thumbnail)) - Moved to public
        .route("/api/v1/media/:id/refresh", axum::routing::post(refresh_media_metadata))
        .route("/api/v1/media/:id/identify", axum::routing::post(identify_media))
        .route("/api/v1/metadata/search", get(search_handler))
        .route("/api/v1/library/search", get(search_library))
        // Images (photo galleries)
        .route("/api/v1/libraries/:id/images", get(list_library_images))
        // Recycle bin: list / empty trashed photos for an Images library.
        .route("/api/v1/libraries/:id/trash", get(list_trash).delete(empty_trash))
        .route("/api/v1/galleries", get(list_galleries).post(create_gallery))
        .route("/api/v1/galleries/:id", get(get_gallery).put(update_gallery).delete(delete_gallery))
        .route("/api/v1/galleries/:id/images", axum::routing::post(add_images))
        .route("/api/v1/galleries/:id/images/:item_id", axum::routing::delete(remove_image))
        .route("/api/v1/images/item/:id", get(get_image_file).put(update_image))
        .route("/api/v1/images/item/:id/restore", axum::routing::post(restore_image))
        .route("/api/v1/images/item/:id/trash", axum::routing::delete(purge_image))
        // Music browse
        .route("/api/v1/artists", get(crate::api::handlers::media::get_artists))
        .route("/api/v1/artists/:id", get(crate::api::handlers::media::get_artist_detail))
        .route("/api/v1/albums/:id", get(crate::api::handlers::media::get_album_detail))
        // Per-user playlists
        .route("/api/v1/me/playlists", get(crate::api::handlers::playlists::list_playlists).post(crate::api::handlers::playlists::create_playlist))
        .route("/api/v1/me/playlists/:id", get(crate::api::handlers::playlists::get_playlist).delete(crate::api::handlers::playlists::delete_playlist))
        .route("/api/v1/me/playlists/:id/tracks", axum::routing::post(crate::api::handlers::playlists::add_track))
        .route("/api/v1/me/playlists/:id/tracks/:item_id", axum::routing::delete(crate::api::handlers::playlists::remove_track))

        .route("/api/v1/settings", get(get_settings).post(update_setting))
        .route("/api/v1/settings/transcode", get(crate::api::handlers::transcode::get_transcode_settings).post(crate::api::handlers::transcode::update_transcode_settings))
        .route("/api/v1/system/shutdown", axum::routing::post(crate::api::handlers::system::shutdown))
        .route("/api/v1/system/restart", axum::routing::post(crate::api::handlers::system::restart))
        .route("/api/v1/system/clear", axum::routing::post(crate::api::handlers::system::clear_database))
        .route("/api/v1/scan", axum::routing::post(scan_all_libraries))
        .route("/api/v1/libraries/:id/scan", axum::routing::post(scan_library))
        .route("/api/v1/libraries/:id/refresh", axum::routing::post(refresh_library))
        .route("/api/v1/media/:id/lyrics", get(get_track_lyrics))
        .route("/api/v1/media/:id/progress", get(get_media_progress).post(update_progress))
        .route("/api/v1/continue", get(get_continue_watching))
        // Book reader routes
        .route("/api/v1/books/:id/info", get(crate::api::handlers::books::get_book_info))
        .route("/api/v1/books/:id/page/:index", get(crate::api::handlers::books::get_book_page))
        .route("/api/v1/books/:id/file", get(crate::api::handlers::books::stream_book_file).head(crate::api::handlers::books::stream_book_file))
        .route("/api/v1/books/:id/reading-mode", axum::routing::post(crate::api::handlers::books::set_reading_mode))
        // Book Series routes
        .route("/api/v1/book-series/:id/detail", get(get_book_series_detail))
        .route("/api/v1/book-series/:id/books", get(get_book_series_chapters))
        // TV Show routes (keyed by series id)
        .route("/api/v1/series", get(get_all_series))
        .route("/api/v1/series/:id/seasons", get(get_series_seasons))
        .route("/api/v1/series/:id/detail", get(get_series_detail))
        .route("/api/v1/series/:id/refresh", axum::routing::post(refresh_series_metadata))
        .route("/api/v1/series/:id/identify", axum::routing::post(identify_series))
        .route("/api/v1/series/:id/season/:num", get(get_season_episodes))
        // Per-user settings and favorites
        .route("/api/v1/me/settings", get(crate::api::handlers::settings::get_user_settings).post(crate::api::handlers::settings::update_user_setting))
        .route("/api/v1/favorites", get(crate::api::handlers::favorites::list_favorites))
        .route("/api/v1/favorites/:id", axum::routing::post(crate::api::handlers::favorites::add_favorite).delete(crate::api::handlers::favorites::remove_favorite))
        .route("/api/v1/auth/me", get(crate::api::handlers::auth::me))
        .route("/api/v1/auth/change_password", axum::routing::post(crate::api::handlers::auth::change_password))
        // Admin-only: list / create / delete users.
        .route("/api/v1/users", get(crate::api::handlers::auth::list_users).post(crate::api::handlers::auth::create_user))
        .route("/api/v1/users/:id", axum::routing::delete(crate::api::handlers::auth::delete_user))
        // Metadata Provider management routes
        .route("/api/v1/providers", get(list_providers))
        .route("/api/v1/providers/order", axum::routing::put(reorder_providers))
        .route("/api/v1/providers/:id/config", get(get_provider_config).put(update_provider_config))
        .route("/api/v1/providers/:id/toggle", axum::routing::post(toggle_provider))
        .route("/api/v1/providers/:id/test", axum::routing::post(test_provider))
        .route_layer(middleware::from_fn(auth_middleware));

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
            axum::http::Method::HEAD,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::COOKIE,
        ]);

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors)
        .layer(tower_cookies::CookieManagerLayer::new())
        // Request logging middleware
        .layer(middleware::from_fn(request_logging))
        .with_state(state)
}

