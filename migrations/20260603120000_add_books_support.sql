-- Books library support.
-- Per-book page count (populated for CBZ at scan time; PDF/EPUB filled by the client),
-- per-book reading-mode override, and a library-level default reading mode.
ALTER TABLE media ADD COLUMN page_count INTEGER;
ALTER TABLE media ADD COLUMN reading_mode TEXT;
ALTER TABLE libraries ADD COLUMN default_reading_mode TEXT;
