pub mod books;
pub mod common;
pub mod library;
pub mod media;
pub mod playback;
pub mod transcode;
pub mod images;
pub mod tv;
pub mod auth;
pub mod settings;
pub mod system;

// Re-export specific handlers for convenience if needed, 
// or clean up routes.rs to use fully qualified names.
// For now, let's just expose modules.
