-- Per-user state and settings.
--
-- Playback progress, favorites and settings become per-user. The old global
-- `playback_progress` table is dropped (existing progress is intentionally discarded).

CREATE TABLE IF NOT EXISTS user_media_progress (
    user_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id        INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    position       INTEGER NOT NULL,
    total_duration INTEGER NOT NULL,
    reading_style  TEXT,
    last_watched   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, item_id)
);

CREATE INDEX IF NOT EXISTS idx_user_progress_user ON user_media_progress(user_id, last_watched);

CREATE TABLE IF NOT EXISTS user_favorites (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id    INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, item_id)
);

-- Per-user preferences. The existing global `settings` table is kept for
-- server-wide config (provider keys, scan paths); this holds per-user prefs.
CREATE TABLE IF NOT EXISTS user_settings (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key     TEXT NOT NULL,
    value   TEXT NOT NULL,
    PRIMARY KEY (user_id, key)
);

-- Discard the old global, single-user progress table.
DROP TABLE IF EXISTS playback_progress;
