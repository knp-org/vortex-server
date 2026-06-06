use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
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
use crate::models::user::{CreateUser, User};

// JWT Secret - In production this should be an env var
use crate::infrastructure::config;

// JWT Secret is now loaded from config

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // username
    pub role: String,
    pub exp: usize,
}

pub async fn register(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, AppError> {
    let service = UserService::new(pool);

    // Check if user exists
    if service.get_by_username(&payload.username).await?.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    // Hash password with Argon2id
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Hashing failed: {}", e)))?
        .to_string();

    service.create(&payload.username, &password_hash).await?;

    Ok(StatusCode::CREATED)
}

pub async fn login(
    State(pool): State<SqlitePool>,
    cookies: Cookies,
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, AppError> {
    let service = UserService::new(pool);

    let user = service.get_by_username(&payload.username).await?
        .ok_or(AppError::AuthError("Invalid credentials".to_string()))?;

    // Verify Password
    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("Invalid hash: {}", e)))?;
    
    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::AuthError("Invalid credentials".to_string()))?;

    // Create JWT
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.username.clone(),
        role: user.role.clone(),
        exp: expiration,
    };

    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(config().jwt_secret.as_bytes()))
        .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))?;

    // Set HTTP-only cookie
    let mut cookie = tower_cookies::Cookie::new("auth_token", token.clone());
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_max_age(tower_cookies::cookie::time::Duration::days(1));
    
    cookies.add(cookie);

    let mut user_response = user;
    user_response.token = Some(token);

    Ok(Json(user_response))
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
    cookies: Cookies,
) -> Result<Json<User>, AppError> {
     let token = cookies.get("auth_token").map(|c| c.value().to_string());
     
     if let Some(token) = token {
          // Validate token (simplified for 'me' check, fuller validation in middleware)
          let token_data = jsonwebtoken::decode::<Claims>(
            &token,
            &jsonwebtoken::DecodingKey::from_secret(config().jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        ).map_err(|_| AppError::AuthError("Invalid token".to_string()))?;
        
        let service = UserService::new(pool);
        let user = service.get_by_username(&token_data.claims.sub).await?
             .ok_or(AppError::NotFound("User not found".to_string()))?;
             
        Ok(Json(user))
     } else {
         Err(AppError::AuthError("Not logged in".to_string()))
     }
}
