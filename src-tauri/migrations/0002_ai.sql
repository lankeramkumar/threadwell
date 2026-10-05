-- Schema version 2: AI conversations, agent runs, tool events and change proposals.
-- Nothing here writes workspace content. Proposals hold suggested changes until a user applies them.

CREATE TABLE ai_conversations (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    title        TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 120),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE INDEX ai_conversations_by_ws ON ai_conversations (workspace_id, updated_at);

CREATE TABLE ai_runs (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    conversation_id TEXT REFERENCES ai_conversations(id),
    kind           TEXT NOT NULL CHECK (kind IN ('chat', 'page_action')),
    status         TEXT NOT NULL CHECK (status IN ('running', 'completed', 'cancelled', 'failed')),
    page_id        TEXT,
    provider       TEXT NOT NULL,
    model          TEXT NOT NULL,
    steps          INTEGER NOT NULL DEFAULT 0,
    error_category TEXT,
    prompt_tokens  INTEGER,
    output_tokens  INTEGER,
    started_at     TEXT NOT NULL,
    finished_at    TEXT,
    duration_ms    INTEGER
);
CREATE INDEX ai_runs_by_ws ON ai_runs (workspace_id, started_at);

CREATE TABLE ai_messages (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    conversation_id TEXT NOT NULL REFERENCES ai_conversations(id),
    run_id         TEXT REFERENCES ai_runs(id),
    role           TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content        TEXT NOT NULL CHECK (length(content) <= 40000),
    citations_json TEXT NOT NULL DEFAULT '[]',
    created_at     TEXT NOT NULL
);
CREATE INDEX ai_messages_by_conv ON ai_messages (conversation_id, created_at);

CREATE TABLE ai_tool_events (
    id             TEXT PRIMARY KEY,
    run_id         TEXT NOT NULL REFERENCES ai_runs(id),
    step           INTEGER NOT NULL,
    tool           TEXT NOT NULL,
    args_json      TEXT NOT NULL CHECK (length(args_json) <= 4000),
    ok             INTEGER NOT NULL CHECK (ok IN (0, 1)),
    summary        TEXT NOT NULL,
    error_category TEXT,
    created_at     TEXT NOT NULL
);

CREATE TABLE change_proposals (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace_meta(id),
    run_id          TEXT REFERENCES ai_runs(id),
    kind            TEXT NOT NULL CHECK (kind IN ('create_page', 'edit_page', 'task_changes')),
    target_page_id  TEXT,
    base_revision   INTEGER,
    payload_json    TEXT NOT NULL,
    diff_text       TEXT NOT NULL,
    summary         TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'applied', 'rejected', 'stale', 'undone')),
    idempotency_key TEXT NOT NULL UNIQUE,
    applied_revision INTEGER,
    undo_json       TEXT,
    created_at      TEXT NOT NULL,
    decided_at      TEXT
);
CREATE INDEX change_proposals_by_run ON change_proposals (workspace_id, run_id);
