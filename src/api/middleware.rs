use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use tower_cookies::Cookies;
use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::api::handlers::auth::Claims;
use crate::infrastructure::config;

pub async fn auth_middleware(
    cookies: Cookies,
    request: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    let token = request.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer ").map(|s| s.to_string()))
        .or_else(|| cookies.get("auth_token").map(|c| c.value().to_string()))
        // Fall back to a `?token=` query param. Needed for media that loads via
        // <video>/native elements which cannot set an Authorization header
        // (e.g. direct stream playback in the Tauri desktop app).
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .find_map(|pair| pair.strip_prefix("token="))
                    .map(|v| urlencoding::decode(v).map(|s| s.into_owned()).unwrap_or_else(|_| v.to_string()))
            })
        });

    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;

    let _claims = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(config::config().jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(request).await)
}
