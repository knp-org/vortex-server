pub mod models;

use sqlx::sqlite::{Sqlite, SqlitePool, SqlitePoolOptions, SqliteConnectOptions};
use sqlx::migrate::MigrateDatabase;
use std::str::FromStr;

pub async fn init_db() -> SqlitePool {
    let database_url = "sqlite:vortex_server.db";

    if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
        println!("Creating database {}", database_url);
        Sqlite::create_database(database_url).await.unwrap();
    }

    let options = SqliteConnectOptions::from_str(database_url)
        .expect("Failed to parse DATABASE_URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    // Create Libraries table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS libraries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            library_type TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create libraries table");

    // Create Settings table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create settings table");

    // Create Playback Progress table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS playback_progress (
            media_id INTEGER PRIMARY KEY,
            position INTEGER NOT NULL,
            total_duration INTEGER NOT NULL,
            last_watched DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(media_id) REFERENCES media(id)
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create playback_progress table");

    // Create Media table (initial creation)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            title TEXT,
            year INTEGER,
            poster_url TEXT,
            plot TEXT,
            media_type TEXT,
            added_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize database");

    // Migration to add library_id if it doesn't exist
    // This is a naive migration check. In production, use sqlx-cli or a migration table.
    let has_library_id: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'library_id'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_library_id.is_none() {
        println!("Migrating database: Adding library_id to media table");
        // We need a default library or allow null. For now, let's create a default library if none exists
        // and assign it. Or simply add the column with default 0/1 if we assume one exists.
        // Better: Add nullable first or default 0. 
        // Let's Add it as INTEGER DEFAULT 0 for now.
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN library_id INTEGER DEFAULT 0").execute(&pool).await;
    }

    // Migration: Add TV show columns if they don't exist
    let has_series_name: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'series_name'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_series_name.is_none() {
        println!("Migrating database: Adding TV show columns to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN series_name TEXT").execute(&pool).await;
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN season_number INTEGER").execute(&pool).await;
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN episode_number INTEGER").execute(&pool).await;
    }

    // Migration: Add provider_ids if it doesn't exist
    let has_provider_ids: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'provider_ids'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_provider_ids.is_none() {
        println!("Migrating database: Adding provider_ids to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN provider_ids TEXT").execute(&pool).await;
        
        // Migrate existing tmdb_id to provider_ids
        println!("Migrating database: Moving tmdb_id data to provider_ids");
        let _ = sqlx::query(
            "UPDATE media SET provider_ids = '{\"tmdb\":' || tmdb_id || '}' WHERE tmdb_id IS NOT NULL AND provider_ids IS NULL"
        ).execute(&pool).await;
    }

    // Migration: Add backdrop_url if it doesn't exist
    let has_backdrop_url: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'backdrop_url'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_backdrop_url.is_none() {
        println!("Migrating database: Adding backdrop_url to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN backdrop_url TEXT").execute(&pool).await;
    }

    // Migration: Add still_url if it doesn't exist
    let has_still_url: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'still_url'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_still_url.is_none() {
        println!("Migrating database: Adding still_url to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN still_url TEXT").execute(&pool).await;
    }

    // Migration: Add runtime if it doesn't exist
    let has_runtime: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'runtime'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_runtime.is_none() {
        println!("Migrating database: Adding runtime to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN runtime INTEGER").execute(&pool).await;
    }

    // Migration: Add genres if it doesn't exist
    let has_genres: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('media') WHERE name = 'genres'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if has_genres.is_none() {
        println!("Migrating database: Adding genres to media table");
        let _ = sqlx::query("ALTER TABLE media ADD COLUMN genres TEXT").execute(&pool).await;
    }

    pool
}
