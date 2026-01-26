mod db;
mod api;
mod core;
mod models;
mod dtos;
mod providers;
pub mod error;


use std::net::SocketAddr;
use crate::db::init_db;
use crate::api::routes::app;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    
    // Logging w/ strict type requirement fix if needed, but for now basic:
    tracing_subscriber::fmt::init();

    let pool = init_db().await;
    
    // Migration: Add backdrop_url column if not exists
    let _ = sqlx::query("ALTER TABLE media ADD COLUMN backdrop_url TEXT").execute(&pool).await;
    
    // Background scan removed to prevent load on startup
    // Scan is now triggered manually via API or on library creation

    // Router with static file serving and request logging
    let app = app(pool)
        .nest_service("/", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Vortex Server listening on http://{}", addr);
    println!("To connect from other devices, use your machine's local IP address (e.g., http://192.168.x.x:3000)");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
