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

/// The authenticated caller, derived from the JWT and injected into request
/// extensions by [`auth_middleware`]. Handlers extract it via `Extension<AuthUser>`.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub role: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    /// Returns `Ok(())` only for admins; otherwise a 403.
    pub fn require_admin(&self) -> Result<(), crate::error::AppError> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(crate::error::AppError::Forbidden("Admin privileges required".to_string()))
        }
    }
}

pub async fn auth_middleware(
    cookies: Cookies,
    mut request: Request<Body>,
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

    let claims = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(config::config().jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;

    // Make the caller available to downstream handlers via `Extension<AuthUser>`.
    request.extensions_mut().insert(AuthUser { id: claims.uid, role: claims.role });

    Ok(next.run(request).await)
}
