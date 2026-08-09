-- Manual metadata editing.
--
-- Users can now hand-edit the metadata of any catalog entity. A hand-edit is only
-- durable if an automatic refresh (metadata refresh, or a rescan that re-enriches an
-- item) can't silently overwrite it, so each editable detail table gets a
-- `metadata_locked` flag. When set, `CatalogService::apply_*_metadata` skips the row
-- entirely; only an explicit re-identify clears the lock.

ALTER TABLE movies   ADD COLUMN metadata_locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE series   ADD COLUMN metadata_locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE episodes ADD COLUMN metadata_locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE books    ADD COLUMN metadata_locked INTEGER NOT NULL DEFAULT 0;
