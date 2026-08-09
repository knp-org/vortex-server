use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use axum::http::HeaderMap;
use tower_cookies::Cookies;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
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

// ---------------------------------------------------------------------------
// Hidden-content unlock
// ---------------------------------------------------------------------------

/// Header carrying the short-lived token that reveals hidden (`other`) playlists.
pub const UNLOCK_HEADER: &str = "x-vortex-unlock";

/// How long an unlock lasts before the PIN must be entered again.
pub const UNLOCK_TTL_MINUTES: i64 = 30;

/// Claims for the unlock token. Deliberately separate from [`Claims`] — it is
/// not a session and grants nothing beyond visibility of hidden playlists, and
/// the `scope` check stops an ordinary auth token from being replayed here.
#[derive(Debug, Serialize, Deserialize)]
pub struct UnlockClaims {
    pub uid: i64,
    pub scope: String,
    pub exp: usize,
}

/// Mint an unlock token for a user who has just proved their PIN.
/// Returns the token and its lifetime in seconds.
pub fn make_unlock_token(user_id: i64) -> Result<(String, i64), crate::error::AppError> {
    let ttl = chrono::Duration::minutes(UNLOCK_TTL_MINUTES);
    let claims = UnlockClaims {
        uid: user_id,
        scope: "hidden".to_string(),
        exp: (chrono::Utc::now() + ttl).timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config::config().jwt_secret.as_bytes()),
    )
    .map_err(|e| crate::error::AppError::Internal(format!("Unlock token creation failed: {}", e)))?;

    Ok((token, ttl.num_seconds()))
}

/// Whether this request carries a valid, unexpired unlock token for `user_id`.
///
/// Never errors: a missing, malformed, expired or mismatched token simply means
/// "still locked", and hidden playlists stay invisible.
pub fn hidden_unlocked(headers: &HeaderMap, user_id: i64) -> bool {
    headers
        .get(UNLOCK_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|token| {
            decode::<UnlockClaims>(
                token.trim_start_matches("Bearer "),
                &DecodingKey::from_secret(config::config().jwt_secret.as_bytes()),
                &Validation::default(),
            )
            .ok()
        })
        .map(|data| data.claims.uid == user_id && data.claims.scope == "hidden")
        .unwrap_or(false)
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
