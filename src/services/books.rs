//! Book content service.
//!
//! Handles the "Books" library formats. CBZ archives are unpacked on the server
//! so the client can request individual page images; PDF and EPUB are served as
//! raw files and rendered client-side (pdf.js / epub.js).
//!
//! Stateless format utilities (a `BookFormat` enum + path/archive helpers, no DB
//! pool), so this stays a free-function module rather than a `*Service` struct.
//! DB-backed book reads/writes live in [`crate::services::book_service`].

use crate::error::AppError;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Cbz,
    Pdf,
    Epub,
}

impl BookFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            BookFormat::Cbz => "cbz",
            BookFormat::Pdf => "pdf",
            BookFormat::Epub => "epub",
        }
    }
}

/// File extensions recognised for Books libraries (lowercase, no dot).
pub const BOOK_EXTENSIONS: &[&str] = &["pdf", "cbz", "epub"];

/// Detect the book format from a file path's extension.
pub fn detect(path: &str) -> Option<BookFormat> {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())?;
    match ext.as_str() {
        "cbz" => Some(BookFormat::Cbz),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

fn image_mime(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".gif") {
        Some("image/gif")
    } else if lower.ends_with(".webp") {
        Some("image/webp")
    } else if lower.ends_with(".bmp") {
        Some("image/bmp")
    } else {
        None
    }
}

/// Natural-order comparison so page filenames sort as `2 < 10` rather than `10 < 2`.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.bytes().peekable();
    let mut bi = b.bytes().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    // Compare full runs of digits numerically (skipping leading zeros).
                    let na = take_number(&mut ai);
                    let nb = take_number(&mut bi);
                    let trimmed_a = na.trim_start_matches('0');
                    let trimmed_b = nb.trim_start_matches('0');
                    match trimmed_a.len().cmp(&trimmed_b.len()).then(trimmed_a.cmp(trimmed_b)) {
                        std::cmp::Ordering::Equal => continue,
                        ord => return ord,
                    }
                } else {
                    let la = ca.to_ascii_lowercase();
                    let lb = cb.to_ascii_lowercase();
                    if la != lb {
                        return la.cmp(&lb);
                    }
                    ai.next();
                    bi.next();
                }
            }
        }
    }
}

fn take_number(it: &mut std::iter::Peekable<std::str::Bytes<'_>>) -> String {
    let mut s = String::new();
    while let Some(&c) = it.peek() {
        if c.is_ascii_digit() {
            s.push(c as char);
            it.next();
        } else {
            break;
        }
    }
    s
}

/// Return the sorted list of image entry names inside a CBZ (zip) archive.
fn cbz_image_names<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Vec<String> {
    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            if entry.is_dir() {
                return None;
            }
            let name = entry.name().to_string();
            if image_mime(&name).is_some() {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    names
}

/// Count the image pages inside a CBZ archive. Runs blocking IO on a worker thread.
pub async fn cbz_page_count(path: &str) -> Result<usize, AppError> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)
            .map_err(|e| AppError::Internal(format!("Failed to open CBZ: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::Internal(format!("Invalid CBZ archive: {}", e)))?;
        Ok(cbz_image_names(&mut archive).len())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
}

/// Read the page image at `index` (0-based, natural-sorted) from a CBZ archive.
/// Returns the raw bytes and the image mime type.
pub async fn cbz_page(path: &str, index: usize) -> Result<(Vec<u8>, &'static str), AppError> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(&path)
            .map_err(|e| AppError::Internal(format!("Failed to open CBZ: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::Internal(format!("Invalid CBZ archive: {}", e)))?;
        let names = cbz_image_names(&mut archive);
        let name = names
            .get(index)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Page {} not found", index)))?;
        let mime = image_mime(&name).unwrap_or("application/octet-stream");
        let mut entry = archive
            .by_name(&name)
            .map_err(|e| AppError::Internal(format!("Failed to read page: {}", e)))?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| AppError::Internal(format!("Failed to read page bytes: {}", e)))?;
        Ok((buf, mime))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
}
