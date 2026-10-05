-- Schema version 7: linked source folders. A source is a folder the user chose. Each readable
-- file in it is kept as a read-only page whose source_id and source_path identify it, so search,
-- retrieval and citations work the same way as for notes. Source pages are never edited in the app.
-- source_hash holds a file fingerprint (modified time and size), which is compared to decide what to re-read.

CREATE TABLE sources (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    root_path      TEXT NOT NULL,
    name           TEXT NOT NULL,
    added_at       TEXT NOT NULL,
    last_synced_at TEXT,
    last_summary   TEXT,
    UNIQUE (workspace_id, root_path)
);

ALTER TABLE pages ADD COLUMN source_id TEXT REFERENCES sources(id);
ALTER TABLE pages ADD COLUMN source_path TEXT;
ALTER TABLE pages ADD COLUMN source_hash TEXT;

CREATE INDEX pages_by_source ON pages (source_id, source_path);
