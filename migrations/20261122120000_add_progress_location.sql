-- Opaque per-format resume locator for reading progress.
--
-- `position`/`total_duration` are integers, which is enough for video (seconds)
-- and for paged books (CBZ/PDF page index). EPUB has no stable page numbering:
-- epub.js resumes from a CFI string such as
-- `epubcfi(/6/14[chap05ref]!/4[body01]/10[para05]/3:10)`. Store that verbatim
-- alongside the numeric position (which EPUB clients use as a 0-10000 permille
-- of the book, so the rail can still show a progress bar).
ALTER TABLE user_media_progress ADD COLUMN location TEXT;
