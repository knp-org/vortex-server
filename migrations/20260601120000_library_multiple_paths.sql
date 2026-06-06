-- Support multiple folder paths per library.
CREATE TABLE IF NOT EXISTS library_paths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_library_paths_library_id ON library_paths(library_id);

-- Migrate existing single paths into the new table.
INSERT INTO library_paths (library_id, path)
SELECT id, path FROM libraries WHERE path IS NOT NULL AND path != '';
