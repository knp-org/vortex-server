//! Provider Manifest & Config Schema
//!
//! Defines the static descriptor that each metadata provider exposes,
//! including its identity, supported media types, and configuration schema.
//! The config schema drives dynamic UI generation on the frontend.

use serde::Serialize;

/// Static descriptor for a metadata provider.
/// Each provider declares one of these so the registry and admin UI
/// know what it is, what it supports, and what configuration it needs.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderManifest {
    /// Unique provider identifier, e.g. "tmdb"
    pub id: &'static str,
    /// Human-readable name, e.g. "The Movie Database"
    pub name: &'static str,
    /// Short description of the provider
    pub description: &'static str,
    /// Media types this provider can handle, e.g. ["movie", "series"]
    pub media_types: &'static [&'static str],
    /// Configuration fields that drive the dynamic settings form
    pub config_schema: Vec<ConfigField>,
    /// Whether this provider requires an API key to function
    pub requires_api_key: bool,
}

/// A single configuration field exposed by a provider.
/// The frontend renders a form input based on `field_type`.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigField {
    /// Machine key, e.g. "api_key"
    pub key: &'static str,
    /// Human-readable label, e.g. "API Key"
    pub label: &'static str,
    /// Determines the input widget rendered on the frontend
    pub field_type: FieldType,
    /// Whether this field must be provided
    pub required: bool,
    /// Default value (JSON), used when no user value is stored
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// For `Select` fields: list of (value, label) options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<(&'static str, &'static str)>>,
}

/// The type of form widget to render for a config field.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum FieldType {
    /// Plain text input
    Text,
    /// Password / masked input — values are masked in GET responses
    Secret,
    /// Boolean toggle switch
    Bool,
    /// Dropdown select from a list of options
    Select,
    /// Numeric input
    Number,
}
