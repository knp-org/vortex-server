-- Normalized lookup tables for genres, tags, studios and people, plus the join
-- tables that link them to media items and grouping entities. Replaces the
-- comma-joined `genres`/`cast`/`director`/`tags` strings the old `media` table used.
--
-- Runs before the media_items spine migration so `movies.studio_id` and the join
-- tables can reference these parents.

CREATE TABLE IF NOT EXISTS genres (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS studios (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS people (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL UNIQUE,
    profile_url  TEXT,
    provider_ids TEXT
);

-- Join tables. Item-level links reference media_items (created in the spine
-- migration that runs next); series/album links reference their grouping entity.

CREATE TABLE IF NOT EXISTS item_genres (
    item_id  INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, genre_id)
);

CREATE TABLE IF NOT EXISTS series_genres (
    series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    genre_id  INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (series_id, genre_id)
);

CREATE TABLE IF NOT EXISTS album_genres (
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (album_id, genre_id)
);

CREATE TABLE IF NOT EXISTS item_tags (
    item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, tag_id)
);

CREATE TABLE IF NOT EXISTS series_tags (
    series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (series_id, tag_id)
);

-- Cast/crew. Exactly one of item_id / series_id is set (enforced in code).
CREATE TABLE IF NOT EXISTS credits (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id  INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    item_id    INTEGER REFERENCES media_items(id) ON DELETE CASCADE,
    series_id  INTEGER REFERENCES series(id) ON DELETE CASCADE,
    role       TEXT,        -- "actor" or a crew job (Director, Writer, ...)
    character  TEXT,
    ord        INTEGER
);

CREATE INDEX IF NOT EXISTS idx_credits_item   ON credits(item_id);
CREATE INDEX IF NOT EXISTS idx_credits_series ON credits(series_id);
CREATE INDEX IF NOT EXISTS idx_credits_person ON credits(person_id);
