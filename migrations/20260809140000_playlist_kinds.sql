-- Playlists gain a `kind`, so a playlist can hold movies, episodes or music
-- videos instead of only music tracks.
--
--   music       -> track
--   movie       -> movie
--   tvshow      -> episode
--   music_video -> music_video
--   other       -> any playable item type (mixed), and hidden
--
-- `other` is the catch-all kind: it may mix item types and is omitted from
-- playlist listings until the caller unlocks it with their PIN (stored as an
-- argon2 hash under the `hidden_playlists_pin_hash` key in `user_settings`).
--
-- Existing rows predate kinds and are all music playlists, which the DEFAULT
-- backfills.
ALTER TABLE playlists ADD COLUMN kind TEXT NOT NULL DEFAULT 'music';

CREATE INDEX IF NOT EXISTS idx_playlists_user_kind ON playlists(user_id, kind);
