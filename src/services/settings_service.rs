//! Settings Service
//!
//! Owns reads/writes for global server settings (`settings`) and per-user
//! settings (`user_settings`).

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::settings::Setting;

pub struct SettingsService {
    pool: SqlitePool,
}

impl SettingsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_global(&self) -> Result<Vec<Setting>, AppError> {
        Ok(sqlx::query_as::<_, Setting>("SELECT * FROM settings")
            .fetch_all(&self.pool)
            .await?)
    }

    /// A single global setting's value by key, or `None` if unset.
    pub async fn get_global(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn upsert_global(&self, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A user's preferences. Keys ending in `_hash` hold credential material
    /// (e.g. the hidden-playlist PIN) and are never handed back to the client.
    pub async fn list_for_user(&self, user_id: i64) -> Result<Vec<Setting>, AppError> {
        Ok(sqlx::query_as::<_, Setting>(
            "SELECT key, value FROM user_settings WHERE user_id = ? AND key NOT LIKE '%\\_hash' ESCAPE '\\'"
        )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Write a user preference. `_hash` keys are reserved for credential
    /// material written by the service that owns it — letting a client set one
    /// directly would let it plant a hash it knows the input for (e.g. reset the
    /// hidden-playlist PIN without proving the current one).
    pub async fn upsert_for_user(&self, user_id: i64, key: &str, value: &str) -> Result<(), AppError> {
        if key.ends_with("_hash") {
            return Err(AppError::Forbidden(format!("Setting '{}' is not client-writable", key)));
        }
        self.upsert_for_user_unchecked(user_id, key, value).await
    }

    /// As [`Self::upsert_for_user`], but permitted to write reserved keys.
    /// Only for services that own the credential they are storing.
    pub async fn upsert_for_user_unchecked(&self, user_id: i64, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?)
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value"
        )
            .bind(user_id)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_pool;

    #[tokio::test]
    async fn get_global_missing_is_none() {
        let pool = test_pool().await;
        let svc = SettingsService::new(pool);
        assert_eq!(svc.get_global("nope").await.unwrap(), None);
    }

    #[tokio::test]
    async fn upsert_global_inserts_then_updates() {
        let pool = test_pool().await;
        let svc = SettingsService::new(pool);

        svc.upsert_global("transcode_encoder", "Auto").await.unwrap();
        assert_eq!(svc.get_global("transcode_encoder").await.unwrap(), Some("Auto".to_string()));

        // Same key again -> update, not a duplicate row.
        svc.upsert_global("transcode_encoder", "libx264").await.unwrap();
        assert_eq!(svc.get_global("transcode_encoder").await.unwrap(), Some("libx264".to_string()));

        let all = svc.list_global().await.unwrap();
        assert_eq!(all.iter().filter(|s| s.key == "transcode_encoder").count(), 1);
    }

    #[tokio::test]
    async fn per_user_settings_are_scoped() {
        let pool = test_pool().await;
        let u1 = crate::test_support::seed_user(&pool, "a").await;
        let u2 = crate::test_support::seed_user(&pool, "b").await;
        let svc = SettingsService::new(pool);

        svc.upsert_for_user(u1, "theme", "dark").await.unwrap();
        svc.upsert_for_user(u2, "theme", "light").await.unwrap();

        let u1_settings = svc.list_for_user(u1).await.unwrap();
        assert_eq!(u1_settings.len(), 1);
        assert_eq!(u1_settings[0].value, "dark");
    }
}
