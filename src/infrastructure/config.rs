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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: 3000,
            transcode_dir: PathBuf::from("transcode"),
            hls_segment_time: 2,
            segment_wait_timeout: 5,
            clear_cache_on_startup: true,
            jwt_secret: "vortex_quantum_secret_key_default".to_string(),
            transcoding_hwa: None,
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
