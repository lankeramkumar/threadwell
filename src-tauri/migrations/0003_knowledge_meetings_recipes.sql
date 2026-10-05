-- Schema version 3: knowledge index (chunks and embeddings), AI exclusion for pages,
-- meetings, and recipes. The ai_runs rebuild lets runs record the new kinds. The migrator
-- turns foreign-key enforcement off for this migration only, because SQLite cannot change a
-- CHECK constraint in place.

ALTER TABLE pages ADD COLUMN ai_excluded INTEGER NOT NULL DEFAULT 0 CHECK (ai_excluded IN (0, 1));

CREATE TABLE page_chunks (
    id           TEXT PRIMARY KEY,
    page_id      TEXT NOT NULL REFERENCES pages(id),
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    ord          INTEGER NOT NULL,
    heading      TEXT NOT NULL DEFAULT '',
    text         TEXT NOT NULL CHECK (length(text) <= 4000),
    content_hash TEXT NOT NULL,
    created_at   TEXT NOT NULL
);
CREATE INDEX page_chunks_by_page ON page_chunks (page_id);
CREATE INDEX page_chunks_by_ws ON page_chunks (workspace_id);

-- One vector per chunk per embedding model. A row whose content_hash no longer matches its
-- chunk is stale, and the indexer replaces it.
CREATE TABLE chunk_embeddings (
    chunk_id     TEXT NOT NULL REFERENCES page_chunks(id) ON DELETE CASCADE,
    model        TEXT NOT NULL,
    dims         INTEGER NOT NULL CHECK (dims > 0),
    vector       BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    PRIMARY KEY (chunk_id, model)
);

CREATE TABLE ai_runs_v3 (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    conversation_id TEXT REFERENCES ai_conversations(id),
    kind           TEXT NOT NULL CHECK (kind IN ('chat', 'page_action', 'meeting', 'recipe', 'eval')),
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
INSERT INTO ai_runs_v3 SELECT * FROM ai_runs;
DROP TABLE ai_runs;
ALTER TABLE ai_runs_v3 RENAME TO ai_runs;
CREATE INDEX ai_runs_by_ws ON ai_runs (workspace_id, started_at);

CREATE TABLE meetings (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspace_meta(id),
    page_id      TEXT NOT NULL REFERENCES pages(id),
    title        TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    source_kind  TEXT NOT NULL CHECK (source_kind IN ('transcript_paste', 'transcript_file')),
    status       TEXT NOT NULL CHECK (status IN ('imported', 'processing', 'processed', 'failed')),
    error        TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    processed_at TEXT
);
CREATE INDEX meetings_by_ws ON meetings (workspace_id, created_at);

CREATE TABLE meeting_segments (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    ord        INTEGER NOT NULL,
    start_ms   INTEGER,
    speaker    TEXT NOT NULL DEFAULT '' CHECK (length(speaker) <= 80),
    text       TEXT NOT NULL CHECK (length(text) <= 2000),
    UNIQUE (meeting_id, ord)
);

-- Claims extracted from a transcript. Each one lists the segment ords it came from.
-- Claims without valid evidence are not stored.
CREATE TABLE meeting_claims (
    id           TEXT PRIMARY KEY,
    meeting_id   TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    ord          INTEGER NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('summary', 'decision', 'question', 'action')),
    text         TEXT NOT NULL CHECK (length(text) <= 1000),
    segment_ords TEXT NOT NULL,
    proposal_id  TEXT REFERENCES change_proposals(id),
    created_at   TEXT NOT NULL
);

CREATE TABLE recipes (
    id            TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL REFERENCES workspace_meta(id),
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    prompt        TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 2000),
    schedule_kind TEXT NOT NULL CHECK (schedule_kind IN ('manual', 'daily', 'weekly')),
    schedule_time TEXT CHECK (schedule_time IS NULL OR schedule_time GLOB '[0-2][0-9]:[0-5][0-9]'),
    weekday       INTEGER CHECK (weekday IS NULL OR weekday BETWEEN 0 AND 6),
    timezone      TEXT NOT NULL CHECK (length(timezone) BETWEEN 1 AND 64),
    enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- scheduled_for is the nominal slot in UTC. The unique key stops one slot running twice,
-- including when the app restarts. Manual runs have no slot, so NULL keeps them distinct.
CREATE TABLE recipe_runs (
    id             TEXT PRIMARY KEY,
    recipe_id      TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    workspace_id   TEXT NOT NULL REFERENCES workspace_meta(id),
    trigger        TEXT NOT NULL CHECK (trigger IN ('manual', 'schedule', 'catch_up')),
    scheduled_for  TEXT,
    status         TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
    error_category TEXT,
    message        TEXT,
    proposal_id    TEXT REFERENCES change_proposals(id),
    ai_run_id      TEXT,
    started_at     TEXT NOT NULL,
    finished_at    TEXT,
    duration_ms    INTEGER,
    UNIQUE (recipe_id, scheduled_for)
);
CREATE INDEX recipe_runs_by_recipe ON recipe_runs (recipe_id, started_at);
