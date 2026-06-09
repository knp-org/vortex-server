pub mod models;

use sqlx::sqlite::{Sqlite, SqlitePool, SqlitePoolOptions, SqliteConnectOptions};
use sqlx::migrate::MigrateDatabase;
use std::str::FromStr;

pub async fn init_db() -> SqlitePool {
    let database_url = "sqlite:vortex_server.db";

    if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
        tracing::info!(database = %database_url, "Creating database");
        Sqlite::create_database(database_url).await.unwrap();
    }

    let options = SqliteConnectOptions::from_str(database_url)
        .expect("Failed to parse DATABASE_URL")
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(30))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(15)
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    // Run Migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}
