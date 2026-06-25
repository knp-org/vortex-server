//! Application configuration module
//! 
//! Provides centralized configuration with environment variable support.

use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: 3000,
            transcode_dir: PathBuf::from("transcode"),
            hls_segment_time: 2,
            segment_wait_timeout: 15,
            clear_cache_on_startup: true,
            jwt_secret: "vortex_quantum_secret_key_default".to_string(),
            transcoding_hwa: None,
            hevc_transcode_threshold_mins: 15.0,
            max_cache_size_mb: 5000,
            data_dir: PathBuf::from("/var/lib/vortex"),
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

        if let Ok(secret) = std::env::var("VORTEX_JWT_SECRET") {
            config.jwt_secret = secret;
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

        config
    }
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
