-- Schema version 1: initial Threadwell workspace.
-- Every row carries workspace_id so queries can be scoped even though each
-- workspace is currently its own SQLite file.

CREATE TABLE workspace_meta (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    created_at  TEXT NOT NULL
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE TABLE pages (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    parent_id    TEXT REFERENCES pages(id),
    title        TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    body_json    TEXT NOT NULL,
    revision     INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    position     INTEGER NOT NULL DEFAULT 0,
    is_favorite  INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
    deleted_at   TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE INDEX pages_by_parent ON pages (workspace_id, parent_id, position);

-- Derived from body_json on every page save; rebuildable from pages.
CREATE TABLE page_links (
    from_page_id TEXT NOT NULL REFERENCES pages(id),
    to_page_id   TEXT NOT NULL REFERENCES pages(id),
    PRIMARY KEY (from_page_id, to_page_id)
);

CREATE TABLE projects (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    name         TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE TABLE tasks (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    project_id     TEXT REFERENCES projects(id),
    title          TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    description    TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 20000),
    status         TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done')),
    priority       TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high')),
    due_date       TEXT CHECK (due_date IS NULL OR due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    source_page_id TEXT REFERENCES pages(id),
    revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    position       INTEGER NOT NULL DEFAULT 0,
    deleted_at     TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX tasks_by_project ON tasks (workspace_id, project_id, status);

-- Lexical index. Derived data: the source of truth is pages.
CREATE VIRTUAL TABLE page_search USING fts5(
    page_id UNINDEXED,
    title,
    body,
    tokenize = 'unicode61'
);
