-- Book series support: linking books into a dedicated book_series table.
CREATE TABLE IF NOT EXISTS book_series (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id      INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    plot            TEXT,
    poster_url      TEXT,
    backdrop_url    TEXT,
    rating          REAL,
    UNIQUE(library_id, name)
);
CREATE INDEX IF NOT EXISTS idx_book_series_library ON book_series(library_id);

ALTER TABLE books ADD COLUMN book_series_id INTEGER REFERENCES book_series(id) ON DELETE CASCADE;
ALTER TABLE books ADD COLUMN chapter_number REAL;
