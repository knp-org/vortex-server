-- Promote the `users` table to a real migration.
-- Mirrors the imperative DDL previously created at runtime by
-- UserService::create_table, so this is a no-op on databases that already have it.
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
