//! User Service
//!
//! Handles database operations for user management.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::user::User;

pub struct UserService {
    pool: SqlitePool,
}

impl UserService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Number of users in the system. Zero means first-run (setup needed).
    pub async fn count(&self) -> Result<i64, AppError> {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn create_with_role(&self, username: &str, password_hash: &str, role: &str) -> Result<User, AppError> {
        let id = sqlx::query(
            "INSERT INTO users (username, password_hash, role) VALUES (?, ?, ?)"
        )
        .bind(username)
        .bind(password_hash)
        .bind(role)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        self.get_by_id(id).await?
            .ok_or_else(|| AppError::Internal("Failed to fetch created user".to_string()))
    }

    /// All users, ordered by id. Password hashes are not serialized (see `User`).
    pub async fn list(&self) -> Result<Vec<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }
    pub async fn update_password(&self, username: &str, password_hash: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
            .bind(password_hash)
            .bind(username)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
