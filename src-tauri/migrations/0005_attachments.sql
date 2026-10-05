-- Schema version 5: files attached to pages. The file itself lives in the workspace's
-- attachments folder under a generated name. The original name is kept for display only.

CREATE TABLE attachments (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    page_id      TEXT NOT NULL REFERENCES pages(id),
    file_name    TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    stored_name  TEXT NOT NULL UNIQUE,
    size         INTEGER NOT NULL CHECK (size >= 0),
    sha256       TEXT NOT NULL,
    created_at   TEXT NOT NULL
);
CREATE INDEX attachments_by_page ON attachments (page_id);
