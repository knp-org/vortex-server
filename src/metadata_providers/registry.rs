//! Provider Registry
//!
//! Central list of all known metadata providers. Adding a new provider
//! requires only one additional entry here — no match arms elsewhere.

use std::sync::OnceLock;
use crate::metadata_providers::manifest::ProviderManifest;
use crate::metadata_providers::traits::MetadataProvider;
use crate::error::AppError;

/// Factory function type: builds a provider instance from JSON config.
pub type Factory = fn(&serde_json::Value) -> Result<Box<dyn MetadataProvider>, AppError>;

/// A single entry in the provider registry.
pub struct RegistryEntry {
    pub manifest: ProviderManifest,
    pub factory: Factory,
}

static REGISTRY: OnceLock<Vec<RegistryEntry>> = OnceLock::new();

/// Returns the global provider registry, initialised once on first call.
///
/// **To add a new provider**, push one `RegistryEntry` here. That's it.
pub fn registry() -> &'static [RegistryEntry] {
    REGISTRY.get_or_init(|| {
        use crate::metadata_providers::tmdb::TmdbProvider;
        use crate::metadata_providers::tvdb::TvdbProvider;

        vec![
            RegistryEntry {
                manifest: TmdbProvider::provider_manifest(),
                factory: TmdbProvider::from_config,
            },
            RegistryEntry {
                manifest: TvdbProvider::provider_manifest(),
                factory: TvdbProvider::from_config,
            },
        ]
    })
}

/// Look up a provider manifest by its id (e.g. "tmdb").
pub fn manifest(id: &str) -> Option<&'static ProviderManifest> {
    registry().iter().find(|e| e.manifest.id == id).map(|e| &e.manifest)
}

/// Look up a provider factory by its id.
pub fn factory(id: &str) -> Option<&'static Factory> {
    registry().iter().find(|e| e.manifest.id == id).map(|e| &e.factory)
}
