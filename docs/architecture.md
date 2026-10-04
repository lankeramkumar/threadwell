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

## Not in this milestone

See the progress checklist in `README.md`. Milestone 2 will add provider adapters and the proposal and approval protocol. Those are designed to sit between the existing services and the UI, so they do not write to the database directly.
