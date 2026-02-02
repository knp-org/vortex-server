//! Request logging middleware with structured tracing
//!
//! Adds request IDs and logs request/response details.

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};
use std::time::Instant;
use tracing::{info_span, Instrument};

/// Middleware that logs request details with timing and request IDs
pub async fn request_logging(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let request_id = uuid_v4_simple();
    let method = request.method().clone();
    let uri = request.uri().path().to_string();
    let start = Instant::now();

    // Create a span for this request
    let span = info_span!(
        "request",
        id = %request_id,
        method = %method,
        path = %uri,
    );

    async move {
        tracing::info!(target: "http", "→ {} {}", method, uri);

        let response = next.run(request).await;
        let duration = start.elapsed();
        let status = response.status();

        if status.is_success() {
            tracing::info!(
                target: "http",
                status = %status.as_u16(),
                duration_ms = %duration.as_millis(),
                "← {} {} - {}ms",
                method, uri, duration.as_millis()
            );
        } else if status.is_client_error() {
            tracing::warn!(
                target: "http",
                status = %status.as_u16(),
                duration_ms = %duration.as_millis(),
                "← {} {} - {} ({}ms)",
                method, uri, status, duration.as_millis()
            );
        } else {
            tracing::error!(
                target: "http",
                status = %status.as_u16(),
                duration_ms = %duration.as_millis(),
                "← {} {} - {} ({}ms)",
                method, uri, status, duration.as_millis()
            );
        }

        response
    }
    .instrument(span)
    .await
}

/// Generate a simple UUID v4 (8 chars for brevity)
fn uuid_v4_simple() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:08x}", rng.gen::<u32>())
}
