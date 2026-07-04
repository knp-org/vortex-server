-- ---------------------------------------------------------------------------
-- Images recycle bin
--
-- Removing a photo from an album used to just NULL out images.gallery_id, which
-- left the photo un-findable and with no way to put it back. Instead we now soft
-- delete: mark the photo trashed and remember which album it came from so it can
-- be restored. A photo is "in the recycle bin" iff deleted_at IS NOT NULL.
--
-- prev_gallery_id references galleries with ON DELETE SET NULL so that trashing
-- survives the album itself being deleted (restore then lands it ungrouped).
-- ---------------------------------------------------------------------------
ALTER TABLE images ADD COLUMN deleted_at TEXT;
ALTER TABLE images ADD COLUMN prev_gallery_id INTEGER REFERENCES galleries(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_images_deleted_at ON images(deleted_at);
