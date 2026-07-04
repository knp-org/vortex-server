//! Application configuration module
//! 
//! Provides centralized configuration with environment variable support.

use std::path::{Path, PathBuf};
use rand::Rng;

/// Application configuration
#[derive(Clone)]
pub struct AppConfig {
    /// Server port (default: 3000)
    pub server_port: u16,
    /// Transcode cache directory
    pub transcode_dir: PathBuf,
    /// HLS segment duration in seconds
    pub hls_segment_time: u32,
    /// Maximum wait time for first segment (seconds)
    pub segment_wait_timeout: u32,
    /// Whether to clear transcode cache on startup
    pub clear_cache_on_startup: bool,
    /// JWT Secret Key
    pub jwt_secret: String,
    /// Transcoding Hardware Acceleration (vaapi, nvenc, qsv, none)
    pub transcoding_hwa: Option<String>,
    /// HEVC transcoding threshold in minutes (default: 15.0)
    #[allow(dead_code)]
    pub hevc_transcode_threshold_mins: f64,
    /// Maximum transcode cache size in MB (0 = unlimited, default: 5000)
    pub max_cache_size_mb: u64,
    /// Storage directory for DB and thumbnails
    pub data_dir: PathBuf,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("server_port", &self.server_port)
            .field("transcode_dir", &self.transcode_dir)
            .field("hls_segment_time", &self.hls_segment_time)
            .field("segment_wait_timeout", &self.segment_wait_timeout)
            .field("clear_cache_on_startup", &self.clear_cache_on_startup)
            // Never log the signing secret.
            .field("jwt_secret", &"<redacted>")
            .field("transcoding_hwa", &self.transcoding_hwa)
            .field("hevc_transcode_threshold_mins", &self.hevc_transcode_threshold_mins)
            .field("max_cache_size_mb", &self.max_cache_size_mb)
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: 3000,
            transcode_dir: PathBuf::from("transcode"),
            hls_segment_time: 2,
            segment_wait_timeout: 15,
            clear_cache_on_startup: true,
            // Empty sentinel: `from_env` always replaces this with an env-supplied
            // or persisted random secret. Never used to sign real tokens.
            jwt_secret: String::new(),
            transcoding_hwa: None,
            hevc_transcode_threshold_mins: 15.0,
            max_cache_size_mb: 5000,
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join(".local/share"))
                        .unwrap_or_else(|| PathBuf::from("."))
                })
                .join("vortex"),
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        if let Ok(port) = std::env::var("VORTEX_PORT") {
            if let Ok(p) = port.parse() {
                config.server_port = p;
            }
        }
        
        if let Ok(dir) = std::env::var("VORTEX_TRANSCODE_DIR") {
            config.transcode_dir = PathBuf::from(dir);
        }
        
        if let Ok(time) = std::env::var("VORTEX_HLS_SEGMENT_TIME") {
            if let Ok(t) = time.parse() {
                config.hls_segment_time = t;
            }
        }
        
        if let Ok(timeout) = std::env::var("VORTEX_SEGMENT_TIMEOUT") {
            if let Ok(t) = timeout.parse() {
                config.segment_wait_timeout = t;
            }
        }
        
        if let Ok(clear) = std::env::var("VORTEX_CLEAR_CACHE") {
            config.clear_cache_on_startup = clear.to_lowercase() != "false";
        }

        if let Ok(hwa) = std::env::var("VORTEX_TRANSCODING_HWA") {
            config.transcoding_hwa = Some(hwa.to_lowercase());
        }

        if let Ok(size) = std::env::var("VORTEX_MAX_CACHE_SIZE_MB") {
            if let Ok(s) = size.parse() {
                config.max_cache_size_mb = s;
            }
        }

        if let Ok(dir) = std::env::var("VORTEX_DATA_DIR") {
            config.data_dir = PathBuf::from(dir);
        }

        // Resolve last, once `data_dir` is final, so a generated secret is persisted
        // in the right place.
        config.jwt_secret = resolve_jwt_secret(&config.data_dir);

        config
    }
}

/// Resolve the JWT signing secret without ever falling back to a hardcoded value.
///
/// Order of preference:
/// 1. `VORTEX_JWT_SECRET` env var (if non-empty).
/// 2. A previously persisted secret at `<data_dir>/jwt_secret.key`.
/// 3. A freshly generated random secret, persisted for future runs so existing
///    sessions survive a restart.
fn resolve_jwt_secret(data_dir: &Path) -> String {
    if let Ok(secret) = std::env::var("VORTEX_JWT_SECRET") {
        if !secret.trim().is_empty() {
            return secret;
        }
    }

    let secret_file = data_dir.join("jwt_secret.key");
    if let Ok(existing) = std::fs::read_to_string(&secret_file) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Generate a 48-char alphanumeric secret (~285 bits of entropy).
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();

    if let Err(e) = std::fs::create_dir_all(data_dir) {
        tracing::warn!(error = %e, "Failed to create data dir for JWT secret; using an ephemeral in-memory secret (sessions will not survive restart)");
        return secret;
    }
    match std::fs::write(&secret_file, &secret) {
        Ok(_) => {
            // Restrict to owner-only on Unix so the secret isn't world-readable.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&secret_file, std::fs::Permissions::from_mode(0o600));
            }
            tracing::warn!(path = %secret_file.display(), "VORTEX_JWT_SECRET not set; generated and persisted a random JWT secret");
        }
        Err(e) => tracing::warn!(error = %e, "Failed to persist generated JWT secret; using an ephemeral in-memory secret (sessions will not survive restart)"),
    }
    secret
}

/// Global configuration instance
static CONFIG: std::sync::OnceLock<AppConfig> = std::sync::OnceLock::new();

/// Initialize the global configuration
pub fn init_config() -> &'static AppConfig {
    CONFIG.get_or_init(AppConfig::from_env)
}

/// Get the global configuration (panics if not initialized)
pub fn config() -> &'static AppConfig {
    CONFIG.get().expect("Configuration not initialized. Call init_config() first.")
}
