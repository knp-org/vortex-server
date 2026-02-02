use axum::http::StatusCode;
use std::{process, time::Duration};

pub async fn shutdown() -> StatusCode {
    tracing::info!("Received shutdown request. Terminating in 500ms...");
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        process::exit(0);
    });
    StatusCode::OK
}

pub async fn restart() -> StatusCode {
    tracing::info!("Received restart request. Restarting in 500ms...");
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let args: Vec<String> = std::env::args().collect();
        if let Ok(exe) = std::env::current_exe() {
            // Unix-specific exec to replace process
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                // args[0] is the executable path, so we skip it for the arguments list
                let _ = process::Command::new(exe).args(&args[1..]).exec();
            }
            
            // Fallback: just exit with non-zero code to encourage supervisor restart
            process::exit(1); 
        }
        process::exit(1);
    });
    StatusCode::OK
}

use axum::extract::State;
use sqlx::SqlitePool;
use crate::error::AppError;

pub async fn clear_database(State(pool): State<SqlitePool>) -> Result<StatusCode, AppError> {
    tracing::warn!("Clearing database content...");
    let mut tx = pool.begin().await.map_err(|e| AppError::Internal(e.to_string()))?;
    
    // Clear content tables
    sqlx::query("DELETE FROM playback_progress").execute(&mut *tx).await.map_err(|e| AppError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM media").execute(&mut *tx).await.map_err(|e| AppError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM libraries").execute(&mut *tx).await.map_err(|e| AppError::Internal(e.to_string()))?;
    
    // Cleanup filesystem
    let cfg = crate::infrastructure::config::config();
    if cfg.transcode_dir.exists() {
        let _ = std::fs::remove_dir_all(&cfg.transcode_dir); // Ignore error if fails (might be empty/locked)
    }
    
    let thumb_dir = std::path::Path::new("thumbnails");
    if thumb_dir.exists() {
        let _ = std::fs::remove_dir_all(thumb_dir);
    }
    
    // Note: We deliberately do NOT clear 'users' or 'settings' to avoid locking out the admin.
    
    tx.commit().await.map_err(|e| AppError::Internal(e.to_string()))?;
    
    tracing::info!("Database content cleared.");
    Ok(StatusCode::OK)
}
