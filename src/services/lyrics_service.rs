//! Lyrics resolution for music tracks.
//!
//! Sources are tried cheapest-first: a `.lrc`/`.txt` sidecar sitting next to the
//! audio file, then lyrics embedded in the file's tags (via lofty), and finally
//! the lrclib.net online database. Only the network result is cached on disk
//! (under `lyrics/{item_id}.json`, including negative results) so we never hit
//! lrclib twice for the same track; local sources are always read fresh.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::infrastructure::error::AppError;

/// A single lyric line. `time` is the start offset in seconds for synced
/// (LRC) lyrics, or `None` for plain unsynced text.
#[derive(Debug, Clone, Serialize)]
pub struct LyricLine {
    pub time: Option<f64>,
    pub text: String,
}

/// Resolved lyrics for a track.
#[derive(Debug, Clone, Serialize)]
pub struct Lyrics {
    /// True when at least one line carries a timestamp (karaoke-style).
    pub synced: bool,
    /// Where the lyrics came from: `lrc` | `txt` | `embedded` | `lrclib` | `none`.
    pub source: String,
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    fn empty() -> Self {
        Lyrics { synced: false, source: "none".into(), lines: Vec::new() }
    }

    fn from_lines(lines: Vec<LyricLine>, source: &str) -> Self {
        let synced = lines.iter().any(|l| l.time.is_some());
        Lyrics { synced, source: source.into(), lines }
    }
}

/// The track facts lrclib needs to look up a match.
struct TrackInfo {
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<i64>,
}

/// Resolves lyrics for music tracks from sidecar files, embedded tags, and lrclib.
pub struct LyricsService {
    pool: SqlitePool,
}

impl LyricsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Resolve lyrics for a track item id. Always returns a value (possibly empty).
    pub async fn for_track(&self, item_id: i64, force: bool) -> Result<Lyrics, AppError> {
        let pool = &self.pool;
        let info = match track_info(pool, item_id).await? {
        Some(i) => i,
        None => return Ok(Lyrics::empty()),
    };
    let path = PathBuf::from(&info.path);

    // 1. Sidecar `.lrc` (synced).
    if !force {
        if let Some(text) = read_sidecar(&path, "lrc").await {
            let lines = parse_lrc(&text);
            if !lines.is_empty() {
                return Ok(Lyrics::from_lines(lines, "lrc"));
            }
        }

        // 2. Embedded tag lyrics (synced if the tag holds LRC text, else plain).
        if let Some(text) = read_embedded(&info.path).await {
            let lines = parse_lrc(&text);
            if !lines.is_empty() {
                return Ok(Lyrics::from_lines(lines, "embedded"));
            }
        }

        // 3. Sidecar `.txt` (plain).
        if let Some(text) = read_sidecar(&path, "txt").await {
            let lines = parse_plain(&text);
            if !lines.is_empty() {
                return Ok(Lyrics::from_lines(lines, "txt"));
            }
        }
    }

    // 4. lrclib.net, cached on disk. A reachable-but-empty answer is cached as a
    //    negative result; a transient failure (timeout/offline) is NOT cached so
    //    we retry next time instead of permanently reporting "no lyrics".
    if !force {
        if let Some(cached) = read_cache(item_id).await {
            return Ok(cached);
        }
    }
    match fetch_lrclib(&info).await {
        Ok(Some(lyrics)) => { write_cache(item_id, &lyrics).await; Ok(lyrics) }
        Ok(None) => {
            let empty = Lyrics::empty();
            write_cache(item_id, &empty).await;
            Ok(empty)
        }
        Err(()) => Ok(Lyrics::empty()),
        }
    }
}

async fn track_info(pool: &SqlitePool, item_id: i64) -> Result<Option<TrackInfo>, AppError> {
    let row: Option<(String, Option<String>, Option<String>, Option<String>, Option<i64>)> =
        sqlx::query_as(
            "SELECT mi.file_path, t.title, ar.name AS artist, al.title AS album, t.duration
             FROM media_items mi
             JOIN tracks t ON t.item_id = mi.id
             LEFT JOIN artists ar ON ar.id = t.artist_id
             LEFT JOIN albums  al ON al.id = t.album_id
             WHERE mi.id = ?",
        )
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|(path, title, artist, album, duration)| TrackInfo {
        path, title, artist, album, duration,
    }))
}

/// Read a sidecar file sharing the audio file's stem (e.g. `song.flac` → `song.lrc`).
async fn read_sidecar(audio: &Path, ext: &str) -> Option<String> {
    let sidecar = audio.with_extension(ext);
    tokio::fs::read_to_string(&sidecar).await.ok().filter(|s| !s.trim().is_empty())
}

/// Pull embedded lyrics out of the file's tags (blocking lofty read off-thread).
async fn read_embedded(path: &str) -> Option<String> {
    let p = path.to_string();
    tokio::task::spawn_blocking(move || {
        use lofty::prelude::*;
        use lofty::tag::ItemKey;
        let tagged = lofty::read_from_path(&p).ok()?;
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
        tag.get_string(ItemKey::Lyrics)
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
    })
    .await
    .ok()
    .flatten()
}

/// Parse LRC text into timestamped lines. Lines without a valid `[mm:ss.xx]`
/// timestamp are treated as metadata tags (`[ar:..]`, `[ti:..]`) and dropped.
/// A single source line may carry several timestamps; each yields one entry.
fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = Vec::new();
    let mut saw_timestamp = false;

    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps: Vec<f64> = Vec::new();
        let mut metadata = false;

        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            let inside = &rest[1..end];
            match parse_timestamp(inside) {
                Some(t) => stamps.push(t),
                None => { metadata = true; break; }   // a tag like [ar:..]
            }
            rest = rest[end + 1..].trim_start();
        }

        if metadata { continue; }
        let content = rest.trim().to_string();

        if stamps.is_empty() {
            // Unsynced line embedded in an otherwise-synced file.
            lines.push(LyricLine { time: None, text: content });
        } else {
            saw_timestamp = true;
            for t in stamps {
                lines.push(LyricLine { time: Some(t), text: content.clone() });
            }
        }
    }

    if saw_timestamp {
        // Synced: order by time and drop the stray untimed lines.
        lines.retain(|l| l.time.is_some());
        lines.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        // No timestamps anywhere → not LRC; let the caller treat it as plain.
        lines.retain(|l| !l.text.is_empty());
    }
    lines
}

/// `mm:ss`, `mm:ss.xx` or `hh:mm:ss` → seconds. Anything else (a tag) → None.
fn parse_timestamp(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        [m, rest] => {
            let mm: f64 = m.trim().parse().ok()?;
            let ss: f64 = rest.trim().parse().ok()?;
            Some(mm * 60.0 + ss)
        }
        [h, m, rest] => {
            let hh: f64 = h.trim().parse().ok()?;
            let mm: f64 = m.trim().parse().ok()?;
            let ss: f64 = rest.trim().parse().ok()?;
            Some(hh * 3600.0 + mm * 60.0 + ss)
        }
        _ => None,
    }
}

/// Plain text → one untimed line per non-empty source line.
fn parse_plain(text: &str) -> Vec<LyricLine> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| LyricLine { time: None, text: l.to_string() })
        .collect()
}

// ── lrclib.net ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// Look the track up on lrclib.net, preferring synced lyrics.
///
/// - `Ok(Some(_))` — found.
/// - `Ok(None)` — lrclib was reachable but has nothing (safe to cache negatively).
/// - `Err(())` — transient failure (timeout/offline/parse); do NOT cache, retry later.
async fn fetch_lrclib(info: &TrackInfo) -> Result<Option<Lyrics>, ()> {
    let title = match info.title.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t,
        None => return Ok(None), // nothing to query on; a definitive miss
    };
    let artist = info.artist.as_deref().unwrap_or("").trim();

    let mut query: Vec<(&str, String)> = vec![
        ("track_name", title.to_string()),
        ("artist_name", artist.to_string()),
    ];
    if let Some(album) = info.album.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        query.push(("album_name", album.to_string()));
    }
    if let Some(d) = info.duration {
        query.push(("duration", d.to_string()));
    }

    // Bounded timeouts so a slow/unreachable lrclib (or no internet) resolves as a
    // miss instead of hanging the request — otherwise the client spinner never stops.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|_| ())?;
    let resp = client
        .get("https://lrclib.net/api/get")
        .query(&query)
        .header(reqwest::header::USER_AGENT, "vortex-server (music library)")
        .send()
        .await
        .map_err(|_| ())?; // timeout / DNS / connection refused → transient

    // 404 means lrclib is up but has no match for this track: a definitive miss.
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(()); // 5xx / rate-limited → transient, retry later
    }
    let body: LrclibResponse = resp.json().await.map_err(|_| ())?;

    if let Some(synced) = body.synced_lyrics.filter(|s| !s.trim().is_empty()) {
        let lines = parse_lrc(&synced);
        if !lines.is_empty() {
            return Ok(Some(Lyrics::from_lines(lines, "lrclib")));
        }
    }
    if let Some(plain) = body.plain_lyrics.filter(|s| !s.trim().is_empty()) {
        let lines = parse_plain(&plain);
        if !lines.is_empty() {
            return Ok(Some(Lyrics::from_lines(lines, "lrclib")));
        }
    }
    Ok(None)
}

// ── disk cache (network results only) ────────────────────────────────────────

fn cache_path(item_id: i64) -> PathBuf {
    Path::new("lyrics").join(format!("{}.json", item_id))
}

#[derive(Serialize, Deserialize)]
struct CachedLyrics {
    synced: bool,
    source: String,
    lines: Vec<CachedLine>,
}

#[derive(Serialize, Deserialize)]
struct CachedLine {
    time: Option<f64>,
    text: String,
}

async fn read_cache(item_id: i64) -> Option<Lyrics> {
    let raw = tokio::fs::read_to_string(cache_path(item_id)).await.ok()?;
    let cached: CachedLyrics = serde_json::from_str(&raw).ok()?;
    Some(Lyrics {
        synced: cached.synced,
        source: cached.source,
        lines: cached.lines.into_iter().map(|l| LyricLine { time: l.time, text: l.text }).collect(),
    })
}

async fn write_cache(item_id: i64, lyrics: &Lyrics) {
    let dir = Path::new("lyrics");
    if !dir.exists() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    let cached = CachedLyrics {
        synced: lyrics.synced,
        source: lyrics.source.clone(),
        lines: lyrics.lines.iter().map(|l| CachedLine { time: l.time, text: l.text.clone() }).collect(),
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        let _ = tokio::fs::write(cache_path(item_id), json).await;
    }
}
