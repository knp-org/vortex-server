use axum::{http::StatusCode, Extension};
use std::{process, time::Duration};
use crate::api::middleware::AuthUser;

pub async fn shutdown(Extension(auth_user): Extension<AuthUser>) -> Result<StatusCode, AppError> {
    auth_user.require_admin()?;
    tracing::info!("Received shutdown request. Terminating in 500ms...");
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        process::exit(0);
    });
    Ok(StatusCode::OK)
}

pub async fn restart(Extension(auth_user): Extension<AuthUser>) -> Result<StatusCode, AppError> {
    auth_user.require_admin()?;
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
    Ok(StatusCode::OK)
}

use axum::extract::State;
use sqlx::SqlitePool;
use crate::error::AppError;

pub async fn clear_database(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    auth_user.require_admin()?;
    tracing::warn!("Clearing database content...");
    
    // Cleanup filesystem caches first
    let cfg = crate::infrastructure::config::config();
    if cfg.transcode_dir.exists() {
        let _ = std::fs::remove_dir_all(&cfg.transcode_dir); // Ignore error if fails
    }
    
    let thumb_dir = cfg.data_dir.join("thumbnails");
    if thumb_dir.exists() {
        let _ = std::fs::remove_dir_all(thumb_dir);
    }

    // Close the database pool so we release file locks
    pool.close().await;

    // Delete the database files directly
    let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db"));
    let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db-shm"));
    let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db-wal"));

    tracing::warn!("Database files deleted. Shutting down server for clean restart...");
    
    // Spawn a thread to exit after the HTTP response is sent
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    Ok(StatusCode::OK)
}
