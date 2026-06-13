-- Identity spine + per-type detail tables.
--
-- `media_items` is the thin spine: one row per file-backed item, holding only the
-- universal facts (library, type, path, timestamps). Each media type then has its
-- own detail table joined 1:1 on `item_id`, so every column is meaningful in context
-- and no type carries another type's fields. Grouping entities that are not files
-- (series, seasons, artists, albums) live in their own tables and are referenced by FK.

CREATE TABLE IF NOT EXISTS media_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    item_type   TEXT NOT NULL,                -- movie | episode | book | track | music_video | image
    file_path   TEXT NOT NULL UNIQUE,
    added_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_media_items_library ON media_items(library_id);
CREATE INDEX IF NOT EXISTS idx_media_items_type    ON media_items(item_type);

-- ---------------------------------------------------------------------------
-- Movies
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS movies (
    item_id        INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    title          TEXT,
    original_title TEXT,
    year           INTEGER,
    plot           TEXT,
    tagline        TEXT,
    runtime        INTEGER,
    rating         REAL,
    age_rating     TEXT,
    studio_id      INTEGER REFERENCES studios(id) ON DELETE SET NULL,
    collection_name TEXT,
    origin_country TEXT,
    creator        TEXT,     -- comma-separated; rare for movies but providers may emit it
    poster_url     TEXT,
    backdrop_url   TEXT,
    trailer_url    TEXT,
    provider_ids   TEXT      -- JSON object, e.g. {"tmdb": 123}
);

-- ---------------------------------------------------------------------------
-- TV: series -> seasons -> episodes
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS series (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id      INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    year            INTEGER,
    plot            TEXT,
    poster_url      TEXT,
    backdrop_url    TEXT,
    rating          REAL,
    age_rating      TEXT,
    studio_id       INTEGER REFERENCES studios(id) ON DELETE SET NULL,
    trailer_url     TEXT,
    collection_name TEXT,
    origin_country  TEXT,
    creator         TEXT,     -- comma-separated show creators
    provider_ids    TEXT,
    -- One series per (library, name) so the scanner's INSERT OR IGNORE groups all
    -- episodes under a single show instead of creating a duplicate per episode.
    UNIQUE(library_id, name)
);

CREATE INDEX IF NOT EXISTS idx_series_library ON series(library_id);

CREATE TABLE IF NOT EXISTS seasons (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id     INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    name          TEXT,
    plot          TEXT,
    poster_url    TEXT,
    UNIQUE(series_id, season_number)
);

CREATE TABLE IF NOT EXISTS episodes (
    item_id        INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    season_id      INTEGER REFERENCES seasons(id) ON DELETE CASCADE,
    episode_number INTEGER,
    title          TEXT,
    plot           TEXT,
    still_url      TEXT,
    runtime        INTEGER,
    air_date       TEXT
);

CREATE INDEX IF NOT EXISTS idx_episodes_season ON episodes(season_id);

-- ---------------------------------------------------------------------------
-- Books
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS books (
    item_id        INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    title          TEXT,
    plot           TEXT,
    poster_url     TEXT,
    page_count     INTEGER,
    reading_mode   TEXT,
    publisher      TEXT,
    published_date TEXT,
    isbn           TEXT
);

-- ---------------------------------------------------------------------------
-- Music: artists -> albums -> tracks
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS artists (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id   INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    bio          TEXT,
    image_url    TEXT,
    provider_ids TEXT,
    UNIQUE(library_id, name)
);

CREATE TABLE IF NOT EXISTS albums (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    artist_id    INTEGER REFERENCES artists(id) ON DELETE CASCADE,
    library_id   INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    year         INTEGER,
    cover_url    TEXT,
    provider_ids TEXT,
    UNIQUE(artist_id, title)
);

CREATE INDEX IF NOT EXISTS idx_albums_artist ON albums(artist_id);

CREATE TABLE IF NOT EXISTS tracks (
    item_id      INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    album_id     INTEGER REFERENCES albums(id) ON DELETE CASCADE,
    artist_id    INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    track_number INTEGER,
    disc_number  INTEGER,
    title        TEXT,
    duration     INTEGER     -- seconds
);

CREATE INDEX IF NOT EXISTS idx_tracks_album  ON tracks(album_id);
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);

-- ---------------------------------------------------------------------------
-- Music videos
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS music_videos (
    item_id     INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    title       TEXT,
    artist_id   INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    artist_name TEXT,
    year        INTEGER,
    plot        TEXT,
    poster_url  TEXT,
    runtime     INTEGER
);

-- ---------------------------------------------------------------------------
-- Images: galleries -> images
--
-- An Images library is a photo gallery/album feature (browse galleries, then the
-- photos inside), parallel to series->episodes and artist->album->track. A gallery
-- is a grouping entity (typically a folder of photos), not a file, so it lives in
-- its own table; each image references its gallery.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS galleries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    cover_url   TEXT,          -- usually the first/representative photo
    taken_at    TEXT,          -- earliest photo date, for sorting
    UNIQUE(library_id, name)
);

CREATE INDEX IF NOT EXISTS idx_galleries_library ON galleries(library_id);

CREATE TABLE IF NOT EXISTS images (
    item_id      INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    gallery_id   INTEGER REFERENCES galleries(id) ON DELETE SET NULL,
    title        TEXT,
    taken_at     TEXT,
    width        INTEGER,
    height       INTEGER,
    camera_make  TEXT,
    camera_model TEXT,
    lens         TEXT,
    iso          INTEGER,
    focal_length REAL,
    aperture     REAL,
    gps_lat      REAL,
    gps_lon      REAL,
    orientation  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_images_gallery ON images(gallery_id);
