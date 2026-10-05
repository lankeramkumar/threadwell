-- Schema version 6: remember each page's full-text row id. page_search.page_id is UNINDEXED,
-- so deleting by page_id scans the whole index. Deleting by rowid is a direct lookup, which
-- makes bulk page creation linear rather than quadratic.

ALTER TABLE pages ADD COLUMN fts_rowid INTEGER;
UPDATE pages SET fts_rowid = (SELECT rowid FROM page_search WHERE page_search.page_id = pages.id);
CREATE INDEX pages_by_fts_rowid ON pages (fts_rowid);
