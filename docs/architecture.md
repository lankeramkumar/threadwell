# Threadwell architecture (milestone 1)

## Shape

```
React UI (src/)  ──typed invoke──►  Tauri commands (src-tauri/src/commands.rs)
                                        │  validate ids, paths, payloads
                                        ▼
                             application modules: pages, tasks, search,
                             transfer (import/export/backup), workspace, sample
                                        │
                                        ▼
                  workspace folder: threadwell.db (SQLite, WAL) + attachments/
```

- The UI never touches files or SQL. It calls `src/lib/api.ts`, which wraps each Tauri command.
- Commands run as `async` so SQLite work stays off the UI thread. One mutex guards the single open workspace.
- No filesystem, shell, or HTTP plugins are enabled. File paths come from native pickers, and the Rust side validates them.
- The webview CSP (`src-tauri/tauri.conf.json`) allows only app-origin scripts, no remote connections, and no framing.

## Workspace layout

A workspace is a folder:

| Path                        | Purpose                                                            |
| --------------------------- | ------------------------------------------------------------------ |
| `threadwell.db`             | Authoritative data (SQLite, WAL mode, `synchronous=NORMAL`)        |
| `threadwell.db-wal`, `-shm` | SQLite journal files; present while the app is open                |
| `attachments/`              | Reserved for milestone 1 imports; created and backed up, no UI yet |

The last opened workspace path is stored in the app config directory (`last-workspace.txt`).

## Schema

Migrations live in `src-tauri/migrations/` and are applied in order by `db::migrate`. The schema version is stored in
SQLite's `PRAGMA user_version`. Opening a database written by a newer version is refused.

| Version | File            | Contents                                                                                       |
| ------- | --------------- | ---------------------------------------------------------------------------------------------- |
| 1       | `0001_init.sql` | `workspace_meta`, `settings`, `pages`, `page_links`, `projects`, `tasks`, `page_search` (FTS5) |

Rules:

- Every row carries `workspace_id`, and every query filters on it.
- Pages and tasks carry `revision` (starts at 1). Saves send the revision they last saw, and a mismatch returns `conflict`.
- Deletion is soft (`deleted_at`). Trashing a page also trashes its live descendants.
- `page_search` and `page_links` are derived. `search::rebuild` regenerates the index from pages. Link rows are refreshed on every page save.
- Check constraints enforce title lengths, status and priority values, and the `YYYY-MM-DD` shape of due dates. Service code also validates real calendar dates.

## Content format

Pages store a Tiptap (ProseMirror) JSON document in `body_json`. Markdown is a conversion format only:

- Export: `markdown::to_markdown` writes headings, paragraphs, lists, task lists, quotes, code blocks, rules, tables, and bold/italic/code/link marks.
- Import: `markdown::from_markdown` parses that subset. A leading `# Title` becomes the page title. Unsupported syntax (HTML, footnotes, images) is kept as text.
- Internal links are stored as `threadwell://page/<id>` hrefs. Only link targets that exist and are live in the same workspace create `page_links` rows. External links are rendered but never navigated by the webview.

## Backup and restore

`transfer::create_backup` writes a timestamped folder containing:

- `workspace.db`: a consistent snapshot made with SQLite's online backup API, which includes WAL content.
- `attachments/`: copied recursively.
- `manifest.json`: format name, format version, app version, workspace id and name, schema version, and a SHA-256 of `workspace.db`.

`transfer::restore_backup` refuses an unknown format or version, refuses a newer schema, and verifies the checksum before it copies anything. It restores only into an empty folder, runs `quick_check`, then re-opens the copy through the normal migration path. The original workspace is never modified.

## Search

- Pages are matched with FTS5 (`unicode61` tokenizer). User input is quoted term by term, so operators cannot be injected. The last term matches as a prefix.
- Tasks are matched with `LIKE` on title and description. `%`, `_` and `\` in input are escaped.
- Both queries filter by workspace and exclude trashed rows.

## Security notes

- Paths must be absolute. Export and restore destinations must sit outside the open workspace folder. Destinations containing `..` are refused.
- Exports and backups never overwrite existing files. Exported filenames are sanitized, and reserved Windows device names are avoided.
- CSV exports prefix cells that start with `= + - @` or tab/CR with an apostrophe to block spreadsheet formula injection.
- Imports accept only `.md`, `.markdown`, or `.txt` files up to 5 MB, and the source file is only read.
- Error messages to the UI carry a code and a safe message. Database and file details go to stderr only.
- No secrets exist in milestone 1. There is no network code.

## Assistant (milestone 2)

```
question ─► retrieve (lexical, OR over keywords) ─► prompt with untrusted sources
                                                        │
          ┌─────────────────────────────────────────────┘
          ▼
   provider (Ollama, streaming NDJSON)  ◄── cancel flag checked per line and per tool call
          │ tool calls
          ▼
   tools: search / read / list (read-only) · propose_* (write a proposal row only)
          │
          ▼
   proposals ── user Apply ─► one transaction, revision check ─► pages / tasks
                            └ stale → status 'stale' (no write)
                            └ Undo  → only if the item is still at its applied revision
```

- **Threads and locks.** A run starts on a worker thread. The workspace mutex is held only for database reads and
  writes, never during generation. The client chooses the run id, so events cannot race the command response.
- **Bounds.** `agent::MAX_STEPS` (6), `MAX_CALLS_PER_STEP` (3), `MAX_RECOVERABLE_FAILURES` (2),
  `MAX_PROPOSALS_PER_RUN` (5). Each limit has a test.
- **Citations.** The model writes `[cite:kind:id]`. `agent::resolve_citations` keeps only tokens that match a source
  retrieved in this run, renumbers them, and counts what it removed.
- **Untrusted content.** Pages, imports, search snippets and task text are wrapped in `<untrusted_content>` with
  closing tags neutralized. The system prompt says that text is data.
- **Transactions.** `db::Tx` is a `BEGIN IMMEDIATE` at top level and a savepoint when nested, so proposal apply
  can call the page and task services and roll back as one unit.
- **Failures.** Provider errors map to categories (`provider_unreachable`, `model_missing`, `provider_timeout`,
  `tool_recovery_exhausted`, `step_limit`). Each run row keeps its category, steps and token counts.
- **Endpoint policy.** `ai::config::validate_endpoint` allows loopback names only unless remote is enabled, and rejects
  userinfo, paths, whitespace and lookalike hosts such as `127.0.0.1.evil.com`.

## Knowledge index (milestone 3)

- **Chunks.** `knowledge::chunk_body` splits a page at headings, then at paragraphs once a chunk reaches 900 characters.
  `reindex_page` runs inside the page save transaction. A chunk whose text is unchanged keeps its row and its embedding.
- **Embeddings.** One row per chunk and model in `chunk_embeddings`. A row is valid only if its content hash matches the
  chunk. Changing the embedding model leaves old rows in place and creates new ones.
- **Retrieval.** `knowledge::retrieve` combines FTS5 BM25 (over OR-ed keywords) and cosine similarity. Each score is
  normalized to 0–1 across the candidates before the weights apply. Search is brute force over the workspace's chunks.
- **Exclusion.** `pages.ai_excluded` removes a page from every AI read path: retrieval, `read_page`, `propose_edit_page`,
  page actions, and the open-page context. Tests cover each path.

## Meetings (milestone 4)

- A meeting is a page plus `meetings` and `meeting_segments` rows. Processing asks the model for JSON, validates it in
  `meetings::validate_extraction`, and stores `meeting_claims`. Action items become one `task_changes` proposal.
- Validation: every item needs segment numbers that exist, and must share a content word with the cited text. Due dates
  must appear literally in the transcript.

## Recipes and the scheduler (milestone 5)

- `recipes::local_to_utc` converts wall-clock slots in the recipe's timezone. `recipe_runs` has a unique key on
  `(recipe_id, scheduled_for)`, so a slot runs once. `due` considers only the most recent slot.
- `start_scheduler` ticks every 30 seconds while the app is open. It claims due runs under the workspace lock, then
  runs each model call without it. Results are draft proposals.

## Migrations (milestone 3)

Migration 0003 rebuilds `ai_runs` to add new run kinds. SQLite cannot change a `CHECK` constraint in place, so the
migrator turns foreign-key enforcement off for that migration only (`db::FK_OFF_MIGRATIONS`), then checks for
violations before commit.

## Evaluation

`ai::eval` runs the real retrieval and agent loop over `eval/cases.json` and writes `eval/reports/`. See `eval/README.md`
for the gates, the results, and the limits.

## Linked sources (after 0.1.4)

`sources.rs` links folders. A source row (migration 0007) records the folder per workspace. Each readable file is a page with
`source_id`, `source_path` and `source_hash` set. Source pages go through the normal search, retrieval and citation paths. The
only difference is that `pages::update` refuses them. The sync uses `pages::save`, which is the same write without that check.

Syncing takes the workspace lock in three short steps. It first walks the folder and reads metadata without the lock. It then
compares fingerprints (modified time and size) under the lock. Changed files are read and converted without the lock. Finally,
the changes are applied in batches of 40 under the lock, so other commands can run between batches. A workspace switch during a
sync stops it. A per-source guard stops two syncs of the same source from running together.

## Workspaces (version 0.1.4)

Each workspace is a folder with its own `threadwell.db`, so pages, tasks, meetings, recipes and assistant history never mix.
`workspaces.rs` keeps a registry of known workspaces (name and path) in `config_dir/workspaces.json`. Every workspace the
app opens is added to it by `commands::install`. Switching, renaming and forgetting go through commands that check their
inputs; forgetting never deletes data, and the open workspace cannot be forgotten.

The assistant chooses a scope for each question: `current` (the open workspace only, the default), `all`, or a named
workspace. The scope comes from the composer's list, or from the message ("all workspaces", or a workspace name as a
whole word). Other workspaces are opened on a read-only connection and searched by keyword. Their hits are labelled with
the workspace name, and their citations use the kind `remote_page`, which the interface shows as text rather than a link
into the open workspace. Changes are proposed only for the open workspace.

## Agents, workspaces and telemetry (version 0.1.4)

**Single agent (default).** `ai::agent::run_loop` runs one model with the full tool set, within six steps.

**Several agents (optional, `ai.architecture = multi`).** `ai::graph::run_multi` runs four roles in sequence:

- _planner_: one JSON call that decides whether the question asks for a change.
- _researcher_: a tool loop with read tools only, three steps at most.
- _writer_: one call with no tools. It is given source tokens only when the research found sources, so it cannot cite
  material the research never read.
- _actor_: a tool loop with read and propose tools. It runs only when the planner asked for a change and the turn covers
  the open workspace only.

Earlier conversation turns are passed to every role as quoted data. Each role is shown only the tool schemas it may use,
and `Allowed` refuses any other call at execution time. The caller resolves citations exactly as for the single agent.
The single agent stays the default. On the fourth held-out split the several-agent design was better at refusing
injected instructions and worse on answer phrasing, due-date grounding and speed; see `eval/README.md`.

**Telemetry (off by default).** `telemetry::init` installs a `tracing` subscriber with an OpenTelemetry layer when the
user enables local traces. Spans are exported by a local JSON-lines exporter, with one rotated backup at 5 MB. The spans
are `agent.run`, `agent.step`, `model.chat` and `tool.call` (and `agent.run` with `architecture = multi`). They carry
names, counts, durations and outcomes only. A test checks that text passed to a span never reaches the file.

## Not in this milestone

See the progress checklist in `README.md`. Milestone 2 will add provider adapters and the proposal and approval protocol. Those are designed to sit between the existing services and the UI, so they do not write to the database directly.
