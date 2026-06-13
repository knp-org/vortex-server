use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
    Json,
};
use sqlx::SqlitePool;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use crate::error::AppError;
use crate::services::user_service::UserService;
use crate::models::user::{CreateUser, User, ChangePasswordRequest};
use crate::api::middleware::AuthUser;
use crate::infrastructure::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // username
    pub uid: i64,    // user id
    pub role: String,
    pub exp: usize,
}

/// Admin-facing request to create a user; `role` defaults to "user".
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Hashing failed: {}", e)))
}

/// Build a signed JWT for a user.
fn make_token(user: &User) -> Result<String, AppError> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.username.clone(),
        uid: user.id,
        role: user.role.clone(),
        exp: expiration,
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(config().jwt_secret.as_bytes()))
        .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))
}

fn set_auth_cookie(cookies: &Cookies, token: &str) {
    let mut cookie = tower_cookies::Cookie::new("auth_token", token.to_string());
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_max_age(tower_cookies::cookie::time::Duration::days(1));
    cookies.add(cookie);
}

/// Issue a session (token + cookie) and return the user with its token attached.
fn issue_session(mut user: User, cookies: &Cookies) -> Result<Json<User>, AppError> {
    let token = make_token(&user)?;
    set_auth_cookie(cookies, &token);
    user.token = Some(token);
    Ok(Json(user))
}

// ---------------------------------------------------------------------------
// First-run setup
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SetupStatus {
    pub needs_setup: bool,
}

/// Whether the server still needs its first (admin) user created.
pub async fn setup_status(State(pool): State<SqlitePool>) -> Result<Json<SetupStatus>, AppError> {
    let count = UserService::new(pool).count().await?;
    Ok(Json(SetupStatus { needs_setup: count == 0 }))
}

/// Create the first user as an admin. Only valid on a fresh install (no users yet);
/// once any user exists, further accounts are created by an admin via `create_user`.
pub async fn setup(
    State(pool): State<SqlitePool>,
    cookies: Cookies,
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, AppError> {
    let service = UserService::new(pool);

    if service.count().await? > 0 {
        return Err(AppError::Forbidden("Setup already completed".to_string()));
    }
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest("Username and password are required".to_string()));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = service.create_with_role(&payload.username, &password_hash, "admin").await?;

    // Log the new admin straight in.
    issue_session(user, &cookies)
}

// ---------------------------------------------------------------------------
// Admin-gated user creation
// ---------------------------------------------------------------------------

/// Create a new user. Admin-only (after first-run setup).
pub async fn create_user(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    auth_user.require_admin()?;

    let role = match payload.role.as_deref() {
        None | Some("user") => "user",
        Some("admin") => "admin",
        Some(other) => return Err(AppError::BadRequest(format!("Invalid role: {}", other))),
    };

    let service = UserService::new(pool);
    if service.get_by_username(&payload.username).await?.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    let password_hash = hash_password(&payload.password)?;
    service.create_with_role(&payload.username, &password_hash, role).await?;

    Ok(StatusCode::CREATED)
}

/// List all users. Admin-only.
pub async fn list_users(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<User>>, AppError> {
    auth_user.require_admin()?;
    Ok(Json(UserService::new(pool).list().await?))
}

/// Delete a user. Admin-only; an admin cannot delete their own account.
pub async fn delete_user(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    auth_user.require_admin()?;
    if auth_user.id == id {
        return Err(AppError::BadRequest("You cannot delete your own account".to_string()));
    }
    UserService::new(pool).delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

pub async fn login(
    State(pool): State<SqlitePool>,
    cookies: Cookies,
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, AppError> {
    let service = UserService::new(pool);

    let user = service.get_by_username(&payload.username).await?
        .ok_or(AppError::AuthError("Invalid credentials".to_string()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("Invalid hash: {}", e)))?;

    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::AuthError("Invalid credentials".to_string()))?;

    issue_session(user, &cookies)
}

pub async fn logout(cookies: Cookies) -> Result<impl IntoResponse, AppError> {
    let mut cookie = tower_cookies::Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.set_max_age(tower_cookies::cookie::time::Duration::seconds(0));
    cookies.add(cookie);
    Ok(StatusCode::OK)
}

pub async fn me(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<User>, AppError> {
    let service = UserService::new(pool);
    let user = service.get_by_id(auth_user.id).await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;
    Ok(Json(user))
}

pub async fn change_password(
    State(pool): State<SqlitePool>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    let service = UserService::new(pool);
    let user = service.get_by_id(auth_user.id).await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("Invalid hash: {}", e)))?;

    Argon2::default()
        .verify_password(payload.current_password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::AuthError("Invalid current password".to_string()))?;

    let new_password_hash = hash_password(&payload.new_password)?;
    service.update_password(&user.username, &new_password_hash).await?;

    Ok(StatusCode::OK)
}
