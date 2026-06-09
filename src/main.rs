mod db;
mod api;
mod services;
mod models;
mod infrastructure;
mod metadata_providers;

// Re-export for backward compatibility
pub use infrastructure::error;

use std::net::SocketAddr;
use crate::db::init_db;
use crate::api::routes::app;
use crate::infrastructure::init_config;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;


#[tokio::main]
async fn main() {
    // Handle command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-v" || arg == "-V") {
        println!("Vortex Server version {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|arg| arg == "--reset-db") {
        println!("Resetting database by deleting files...");
        
        let _ = std::fs::remove_file("vortex_server.db");
        let _ = std::fs::remove_file("vortex_server.db-shm");
        let _ = std::fs::remove_file("vortex_server.db-wal");

        let cfg = init_config();
        if cfg.transcode_dir.exists() {
            let _ = std::fs::remove_dir_all(&cfg.transcode_dir);
        }
        
        let thumb_dir = std::path::Path::new("thumbnails");
        if thumb_dir.exists() {
            let _ = std::fs::remove_dir_all(thumb_dir);
        }
        
        println!("Database successfully reset. Media files are untouched.");
        return;
    }

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize configuration from environment
    let cfg = init_config();
    tracing::info!("Loaded configuration: {:?}", cfg);

    let pool = init_db().await;

    // Initialize Users Table
    let user_service = crate::services::user_service::UserService::new(pool.clone());
    user_service.create_table().await.expect("Failed to create users table");
    
    // Clear transcode cache on startup to prevent disk bloat
    if cfg.clear_cache_on_startup && cfg.transcode_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&cfg.transcode_dir) {
            tracing::warn!("Failed to clear transcode cache: {}", e);
        } else {
            tracing::info!("Cleared transcode cache on startup");
        }
    }

    // Router with static file serving and request logging
    // SPA Fallback: If file not found in static/, serve index.html
    let serve_dir = ServeDir::new("static")
        .not_found_service(ServeFile::new("static/index.html"));

    let app = app(pool)
        .nest_service("/", serve_dir)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.server_port));
    println!("Vortex Server listening on http://{}", addr);
    println!("To connect from other devices, use your machine's local IP address (e.g., http://192.168.x.x:{})", cfg.server_port);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
