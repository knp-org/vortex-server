-- Metadata Provider Plugin System
-- Stores per-provider configuration (enabled, priority, settings)

CREATE TABLE IF NOT EXISTS provider_configs (
    provider_id  TEXT PRIMARY KEY,        -- "tmdb"
    enabled      INTEGER NOT NULL DEFAULT 1,
    priority     INTEGER NOT NULL DEFAULT 100,  -- lower = tried first
    config_json  TEXT NOT NULL DEFAULT '{}'     -- {"api_key": "...", "language": "en"}
);

-- Optional per-library override of the global provider chain.
CREATE TABLE IF NOT EXISTS library_providers (
    library_id   INTEGER NOT NULL,
    provider_id  TEXT NOT NULL,
    priority     INTEGER NOT NULL DEFAULT 100,
    enabled      INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (library_id, provider_id),
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

-- Back-compat: migrate the existing tmdb_api_key from settings into provider_configs.
-- Uses INSERT OR IGNORE so this is safe to run multiple times.
INSERT OR IGNORE INTO provider_configs (provider_id, enabled, priority, config_json)
SELECT 'tmdb', 1, 10,
       json_object('api_key', COALESCE((SELECT value FROM settings WHERE key='tmdb_api_key'), ''))
;
