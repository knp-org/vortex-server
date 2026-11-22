//! Shared helpers for the inline `#[cfg(test)]` test modules.
//!
//! Tests run against a real, migrated SQLite database held entirely in memory.
//! `max_connections(1)` keeps the single `:memory:` database alive for the whole
//! pool (each new connection would otherwise get its own empty database), and
//! foreign keys are enabled to match the production pool.
#![allow(dead_code)]

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteConnectOptions};
use std::str::FromStr;

/// A fresh, migrated in-memory pool.
pub async fn test_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("connect in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

/// Insert a user, returning its id.
pub async fn seed_user(pool: &SqlitePool, username: &str) -> i64 {
    sqlx::query("INSERT INTO users (username, password_hash, role) VALUES (?, 'x', 'admin')")
        .bind(username)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

/// Insert a library (with a `library_paths` row), returning its id.
pub async fn seed_library(pool: &SqlitePool, name: &str, library_type: &str) -> i64 {
    let id = sqlx::query("INSERT INTO libraries (name, path, library_type) VALUES (?, '/data', ?)")
        .bind(name)
        .bind(library_type)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid();
    sqlx::query("INSERT INTO library_paths (library_id, path) VALUES (?, '/data')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

/// Insert a `media_items` spine row, returning its id.
pub async fn seed_item(pool: &SqlitePool, library_id: i64, item_type: &str, file_path: &str) -> i64 {
    sqlx::query("INSERT INTO media_items (library_id, item_type, file_path) VALUES (?, ?, ?)")
        .bind(library_id)
        .bind(item_type)
        .bind(file_path)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

/// Insert a book spine row plus its `books` detail row, returning the item id.
pub async fn seed_book(pool: &SqlitePool, library_id: i64, file_path: &str, title: &str) -> i64 {
    let id = seed_item(pool, library_id, "book", file_path).await;
    sqlx::query("INSERT INTO books (item_id, title) VALUES (?, ?)")
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    id
}

/// Insert a movie spine row plus its `movies` detail row, returning the item id.
pub async fn seed_movie(pool: &SqlitePool, library_id: i64, file_path: &str, title: &str) -> i64 {
    let id = seed_item(pool, library_id, "movie", file_path).await;
    sqlx::query("INSERT INTO movies (item_id, title) VALUES (?, ?)")
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    id
}
