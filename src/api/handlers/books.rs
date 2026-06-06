//! Book reader endpoints.
//!
//! - `info`         metadata the reader needs (format, page count, reading mode).
//! - `page/:index`  individual CBZ page image (extracted on the server).
//! - `file`         raw PDF/EPUB byte stream (range-aware) for client-side rendering.
//! - `reading-mode` persist the user's chosen reading mode for a book.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

use crate::error::AppError;
use crate::services::book_service::BookService;
use crate::services::books::{self, BookFormat};

#[derive(Serialize)]
pub struct BookInfo {
    pub id: i64,
    pub title: Option<String>,
    pub format: String,
    pub page_count: Option<i64>,
    pub reading_mode: String,
}

pub async fn get_book_info(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<BookInfo>, AppError> {
    let service = BookService::new(pool);
    let book = service.get(id).await?;

    let format = books::detect(&book.file_path)
        .ok_or_else(|| AppError::BadRequest("Unsupported book format".to_string()))?;
    let reading_mode = service.resolve_reading_mode(&book).await?;

    // Backfill CBZ page count on demand if the scan missed it.
    let mut page_count = book.page_count;
    if format == BookFormat::Cbz && page_count.is_none() {
        if let Ok(n) = books::cbz_page_count(&book.file_path).await {
            let n = n as i64;
            let _ = service.set_page_count(id, n).await;
            page_count = Some(n);
        }
    }

    Ok(Json(BookInfo {
        id,
        title: book.title,
        format: format.as_str().to_string(),
        page_count,
        reading_mode,
    }))
}

pub async fn get_book_page(
    Path((id, index)): Path<(i64, usize)>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    let book = BookService::new(pool).get(id).await?;

    match books::detect(&book.file_path) {
        Some(BookFormat::Cbz) => {
            let (bytes, mime) = books::cbz_page(&book.file_path, index).await?;
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
            headers.insert(header::CACHE_CONTROL, "private, max-age=3600".parse().unwrap());
            Ok((headers, bytes))
        }
        // PDF/EPUB are rendered client-side from the raw file route.
        _ => Err(AppError::BadRequest(
            "Page extraction is only available for CBZ books".to_string(),
        )),
    }
}

/// Raw, range-aware stream of the book file (used by the client for PDF/EPUB).
pub async fn stream_book_file(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    method: axum::http::Method,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let book = BookService::new(pool).get(id).await?;
    let file_path = book.file_path;

    let mime = match books::detect(&file_path) {
        Some(BookFormat::Pdf) => "application/pdf",
        Some(BookFormat::Epub) => "application/epub+zip",
        Some(BookFormat::Cbz) => "application/vnd.comicbook+zip",
        None => "application/octet-stream",
    };

    let mut file = File::open(&file_path)
        .await
        .map_err(|_| AppError::NotFound("Book file not found".to_string()))?;
    let file_size = file
        .metadata()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to stat file: {}", e)))?
        .len();

    if method == axum::http::Method::HEAD {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CONTENT_LENGTH, file_size.to_string())
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::empty())
            .unwrap());
    }

    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| {
            let s = s.strip_prefix("bytes=")?;
            let mut parts = s.split('-');
            let start = parts.next()?.parse::<u64>().ok()?;
            let end = parts.next().and_then(|s| s.parse::<u64>().ok());
            Some((start, end))
        });

    match range {
        Some((start, end)) => {
            let end = end.unwrap_or(file_size - 1).min(file_size.saturating_sub(1));
            let length = end.saturating_sub(start) + 1;

            file.seek(SeekFrom::Start(start))
                .await
                .map_err(|e| AppError::Internal(format!("Seek failed: {}", e)))?;

            let stream = tokio_util::io::ReaderStream::new(file.take(length));
            let mut response = Response::new(Body::from_stream(stream));
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            let h = response.headers_mut();
            h.insert(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size).parse().unwrap());
            h.insert(header::CONTENT_LENGTH, length.to_string().parse().unwrap());
            h.insert(header::CONTENT_TYPE, mime.parse().unwrap());
            h.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            Ok(response)
        }
        None => {
            let stream = tokio_util::io::ReaderStream::with_capacity(file, 64 * 1024);
            let mut response = Response::new(Body::from_stream(stream));
            let h = response.headers_mut();
            h.insert(header::CONTENT_LENGTH, file_size.to_string().parse().unwrap());
            h.insert(header::CONTENT_TYPE, mime.parse().unwrap());
            h.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            Ok(response)
        }
    }
}

#[derive(Deserialize)]
pub struct ReadingModeRequest {
    pub reading_mode: String,
}

pub async fn set_reading_mode(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<ReadingModeRequest>,
) -> Result<StatusCode, AppError> {
    BookService::new(pool).set_reading_mode(id, &payload.reading_mode).await?;
    Ok(StatusCode::OK)
}
