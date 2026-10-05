-- Schema version 4: records which files on disk have been imported as pages, so a second
-- import of the same unchanged file is skipped. Source files are never modified.

CREATE TABLE source_imports (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    page_id      TEXT NOT NULL REFERENCES pages(id),
    source_path  TEXT NOT NULL CHECK (length(source_path) BETWEEN 1 AND 1024),
    content_hash TEXT NOT NULL,
    imported_at  TEXT NOT NULL,
    UNIQUE (workspace_id, source_path)
);
