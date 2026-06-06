-- Initial Schema

CREATE TABLE IF NOT EXISTS libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    library_type TEXT NOT NULL DEFAULT 'movie'
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    title TEXT,
    year INTEGER,
    poster_url TEXT,
    plot TEXT,
    media_type TEXT,
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    library_id INTEGER DEFAULT 0,
    series_name TEXT,
    season_number INTEGER,
    episode_number INTEGER,
    provider_ids TEXT,
    backdrop_url TEXT,
    still_url TEXT,
    runtime INTEGER,
    genres TEXT,
    rating REAL,
    cast TEXT,
    director TEXT
);

CREATE TABLE IF NOT EXISTS playback_progress (
    media_id INTEGER PRIMARY KEY,
    position INTEGER NOT NULL,
    total_duration INTEGER NOT NULL,
    last_watched DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(media_id) REFERENCES media(id)
);


