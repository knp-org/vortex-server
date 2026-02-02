use serde::Deserialize;

/// Request to identify a media item with a specific provider ID
#[derive(Deserialize)]
pub struct IdentifyRequest {
    pub provider_id: String,           // Generic string ID to support any provider
    pub media_type: Option<String>,    // "movie" or "series"
}

/// Request to search for metadata
#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub media_type: Option<String>,
}
