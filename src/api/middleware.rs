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
    let token = cookies.get("auth_token").map(|c| c.value().to_string());

    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;

    let _claims = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(config::config().jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(request).await)
}
