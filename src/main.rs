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
        
        let cfg = init_config();
        let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db"));
        let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db-shm"));
        let _ = std::fs::remove_file(cfg.data_dir.join("vortex_server.db-wal"));

        if cfg.transcode_dir.exists() {
            let _ = std::fs::remove_dir_all(&cfg.transcode_dir);
        }
        
        let thumb_dir = cfg.data_dir.join("thumbnails");
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
    // The `users` table is created by migrations (20260613120000_users_table.sql).

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
    let static_dir = std::env::var("VORTEX_STATIC_DIR")
        .unwrap_or_else(|_| "static".to_string());
    let index_path = format!("{}/index.html", &static_dir);
    let serve_dir = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(&index_path));

    let app = app(pool)
        .nest_service("/", serve_dir)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.server_port));
    println!("Vortex Server listening on http://{}", addr);
    println!("To connect from other devices, use your machine's local IP address (e.g., http://192.168.x.x:{})", cfg.server_port);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to bind to {}: {}", addr, e);
        if e.kind() == std::io::ErrorKind::AddrInUse {
            eprintln!("Port {} is already in use. Check for duplicate services or stale processes:", cfg.server_port);
            eprintln!("  sudo fuser -k {}/tcp", cfg.server_port);
            eprintln!("  sudo systemctl list-units '*vortex*' --all");
        }
        std::process::exit(1);
    });
    axum::serve(listener, app).await.unwrap();
}
