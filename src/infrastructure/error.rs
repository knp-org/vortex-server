use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;

/// Structured API error for consistent error responses
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    /// Machine-readable error code
    pub code: &'static str,
    /// Human-readable error message
    pub message: String,
    /// HTTP status code
    pub status: u16,
}

/// Error codes for machine-readable error handling
pub mod codes {
    pub const DATABASE_ERROR: &str = "DATABASE_ERROR";
    pub const NOT_FOUND: &str = "NOT_FOUND";
    pub const MEDIA_NOT_FOUND: &str = "MEDIA_NOT_FOUND";
    pub const LIBRARY_NOT_FOUND: &str = "LIBRARY_NOT_FOUND";
    pub const EXTERNAL_SERVICE_ERROR: &str = "EXTERNAL_SERVICE_ERROR";
    pub const BAD_REQUEST: &str = "BAD_REQUEST";
    pub const VALIDATION_ERROR: &str = "VALIDATION_ERROR";
    pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
    pub const FORBIDDEN: &str = "FORBIDDEN";
    pub const INVALID_CREDENTIALS: &str = "INVALID_CREDENTIALS";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const TRANSCODE_ERROR: &str = "TRANSCODE_ERROR";
    pub const FILE_NOT_FOUND: &str = "FILE_NOT_FOUND";
}

/// Custom error type for API handlers.
/// Implements `IntoResponse` to return consistent JSON error bodies.
#[derive(Debug)]
pub enum AppError {
    /// Database error (500 Internal Server Error)
    Database(sqlx::Error),
    /// Resource not found (404 Not Found)
    NotFound(String),
    /// Media not found (404 Not Found) - specific variant
    MediaNotFound(i64),
    /// Library not found (404 Not Found) - specific variant
    LibraryNotFound(i64),
    /// External service error, e.g., TMDB API failure (502 Bad Gateway)
    External(String),
    /// Bad request / validation error (400 Bad Request)
    BadRequest(String),
    /// Authentication error (401 Unauthorized)
    AuthError(String),
    /// Authenticated but not allowed (403 Forbidden)
    Forbidden(String),
    /// Generic internal error
    Internal(String),
    /// Transcode-specific error
    TranscodeError(String),
}

impl AppError {
    /// Get the error code for this error
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => codes::DATABASE_ERROR,
            AppError::NotFound(_) => codes::NOT_FOUND,
            AppError::MediaNotFound(_) => codes::MEDIA_NOT_FOUND,
            AppError::LibraryNotFound(_) => codes::LIBRARY_NOT_FOUND,
            AppError::External(_) => codes::EXTERNAL_SERVICE_ERROR,
            AppError::BadRequest(_) => codes::BAD_REQUEST,
            AppError::AuthError(_) => codes::AUTH_REQUIRED,
            AppError::Forbidden(_) => codes::FORBIDDEN,
            AppError::Internal(_) => codes::INTERNAL_ERROR,
            AppError::TranscodeError(_) => codes::TRANSCODE_ERROR,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Database(e) => write!(f, "Database error: {}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::MediaNotFound(id) => write!(f, "Media not found: {}", id),
            AppError::LibraryNotFound(id) => write!(f, "Library not found: {}", id),
            AppError::External(msg) => write!(f, "External service error: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AppError::TranscodeError(msg) => write!(f, "Transcode error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let code = self.code();
        
        let (status, message) = match &self {
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::MediaNotFound(id) => (StatusCode::NOT_FOUND, format!("Media with id {} not found", id)),
            AppError::LibraryNotFound(id) => (StatusCode::NOT_FOUND, format!("Library with id {} not found", id)),
            AppError::External(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::AuthError(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::TranscodeError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({
            "code": code,
            "message": message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".to_string()),
            _ => AppError::Database(err),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::External(format!("External API error: {}", err))
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AppError::External(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(format!("IO error: {}", err))
    }
}
