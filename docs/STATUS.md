# Project status

This is the detailed checklist for every milestone in [intent.md](../intent.md): what is built, what is verified,
and what is not. The overview is in the [README](../README.md).

## Update for version 0.1.3

| Item                                            | Status                                                                                                                                                                                                                    |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenTelemetry local traces                      | Built, off by default. Spans for runs, steps, model calls and tool calls go to a local file. No content is recorded. Tested end to end, including a privacy test.                                                         |
| Multi-agent option                              | Built as a graph of roles with per-role tool allowlists. Compared with the single agent on heldout3: no gated improvement, slower. Kept as an experimental option; the single agent stays the default, as the brief asks. |
| Abstention regression in the multi-agent option | Diagnosed: the writer cites every source when the research finds nothing. Not fixed, to avoid tuning on the held-out split.                                                                                               |
| Assistant task suggestions                      | Still below target (0.50 on heldout2 with 7B, 0.08 on heldout3 with 3B).                                                                                                                                                  |
| Bulk and save performance                       | Unchanged from 0.1.2.                                                                                                                                                                                                     |
| Idle memory                                     | 372 MiB measured on 0.1.2; not improved.                                                                                                                                                                                  |

## Update for version 0.1.2 (latest measurements)

These supersede the older numbers below where they conflict.

| Item                                                | Result                                                                                                                                                           |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bulk creation of 5,000 pages (create and update)    | **34 s** (was about 190 s). Cause: each save deleted its search row by an unindexed column, which scanned the whole index. Fixed by deleting by rowid.           |
| Search p95 at 5,000 pages                           | **1.29 ms** (was 2.75 ms)                                                                                                                                        |
| Save of one page on a 5,000-page workspace          | p50 0.35 ms, p95 0.74 ms, measured in release. The 800 ms autosave pause dominates what the user sees. End-to-end UI time is not measured.                       |
| Window visible on launch                            | 59 to 267 ms across three launches                                                                                                                               |
| Idle memory, whole process tree (20 s after launch) | **372 MiB** across three launches: 380, 371, 372. Target is 250 MiB. Not met, and higher than the 336 MiB measured for 0.1.1. The cause has not been identified. |
| Bundle                                              | Main JavaScript 238 kB (was 725 kB). The editor (480 kB) loads only when a page is opened.                                                                       |
| Assistant, qwen2.5:7b on heldout2                   | 7 of 8 gated checks pass. Task keyword recall 0.50 fails. p50 87 s per case on CPU. See eval/README.md.                                                          |
| Installers                                          | MSI 4.12 MiB, setup 3.10 MiB (unsigned)                                                                                                                          |

## Summary

All six milestones in `intent.md` are implemented in this build, with the limits listed under each one. Two results
need to be read with care:

- **The assistant does not meet its own quality gates.** On the held-out evaluation, four of nine gates fail, including
  the injection gates. See [eval/README.md](eval/README.md). Treat the assistant as experimental.
- **Several items cannot be verified here.** Audio transcription has no engine, scheduled runs need the app open, and
  the installers are unsigned and were not tested on a clean machine.

The brief asks that the product not be described as complete when only part of it is verified. Items below are marked
as done only when the automated suite or a recorded measurement covers them.

### Milestone 1: desktop foundation

- [x] Installable Windows build (MSI and NSIS) produced by `npm run tauri build`. See "Windows artifact" below.
- [x] Migrations with a schema version; refuses databases from newer versions
- [x] Create or open a workspace folder; the last workspace reopens on launch
- [x] Nested pages: create, move (cycle-checked), favorite, move to trash, restore
- [x] Rich-text editor (Tiptap): headings, lists, checklists, quotes, code blocks, tables, bold/italic/code, links
- [x] `/` block commands and a formatting toolbar
- [x] Links between pages (`Link to page…`), with a "Linked from" list on the target page
- [x] Autosave about 800 ms after typing stops; retries on failure; revision check rejects stale saves
- [x] Tasks: table view and board view, title, status, priority, optional due date, project, source page link, delete
- [x] Projects; tasks can be filtered by project
- [x] Lexical search over pages (FTS5) and tasks, with highlighted snippets
- [x] Export: all pages to Markdown, tasks to CSV (formula-safe)
- [x] Import: one Markdown or text file as a new page (source is never modified)
- [x] Versioned full-workspace backup with checksum; restore into an empty folder and open it
- [x] Settings: theme (system, light, dark), search index rebuild
- [x] Ctrl+K command palette: open pages and switch views
- [x] Labelled sample workspace (optional at creation): related notes, conflicting decisions, tasks, a meeting transcript
- [x] Works offline; no network code in the app
- [x] Keyboard focus styles, labelled controls, reduced-motion support
- [ ] Full keyboard and screen-reader audit (done by inspection only, not tested with assistive technology)
- [ ] Attachments UI (the `attachments/` folder exists and is backed up, but there is no import flow yet)
- [ ] Drag-and-drop on the board (buttons move cards between columns instead)
- [ ] Permanent deletion from trash (intentionally absent in this build)
- [ ] Measured performance (see "Performance" below)

### Milestone 2: AI assistance (local Ollama)

- [x] Local provider adapter for Ollama (`/api/chat` streaming, `/api/tags` readiness check)
- [x] Endpoint allow-list: only loopback addresses unless remote endpoints are switched on; remote requires https
- [x] Streaming conversation with saved history, reopenable from the conversation list
- [x] Retrieval before generation: the question is searched in the workspace and the hits are given to the model as untrusted sources
- [x] Tools for the model: `search_workspace`, `read_page`, `list_tasks` (read) and `propose_create_page`, `propose_edit_page`, `propose_task_changes` (record a suggestion only). There is no delete, SQL or shell tool
- [x] Bounded loop: at most 6 steps, 3 tool calls per step, 2 recoverable tool failures, 5 proposals per run
- [x] Citations: the model cites with tokens; the backend keeps only tokens that match a source retrieved in that run, numbers them, and removes the rest
- [x] Change proposals with line-diff preview; approve, reject, or undo
- [x] Apply is one transaction, checks the revision the suggestion was based on, and is idempotent; stale suggestions are marked stale, never written over
- [x] Undo restores previous content only if nothing changed since the apply
- [x] Cancellation stops streaming and tool steps; pending suggestions are never applied by a cancelled run
- [x] Page actions on a selection: rewrite, summarize, expand, translate. Accept replaces the selection only if it still matches what was sent
- [x] Number check on page actions: numbers dropped or added in the suggestion are listed before acceptance
- [x] Run history with status, steps, duration and token counts; per-run tool trace
- [x] Keys: none needed for Ollama. The app stores no secrets in this milestone
- [ ] Cloud provider adapter (planned as the next adapter; not built)
- [ ] OS credential store for cloud API keys (needed only with a cloud adapter)
- [ ] Measured evaluation. The live test checks one answer; the 40-case dataset and metrics are milestone 3
- [ ] Automatic answer-support checking. Citations show which source the model used; they do not prove the claim is supported
- [ ] Cancellation granularity: a stop takes effect at the next streamed line or tool step. A stalled model is interrupted by its 180-second timeout
- [ ] Live check for page actions (only the code path and unit tests are verified)
- [ ] Conversation search and a side-by-side selected-context panel

### Milestone 3: knowledge quality

- [x] Semantic index: chunks by heading and size, embeddings through a local model (`nomic-embed-text` by default)
- [x] Chunks keep their embedding when their text is unchanged; a changed paragraph is the only thing re-embedded
- [x] Embeddings are keyed by model, so switching models re-embeds rather than mixing vector spaces
- [x] Hybrid ranking: FTS5 BM25 and cosine, each normalized 0–1, with configurable weights; keyword-only mode
- [x] Exclusion controls: pages marked "Exclude from AI" are never retrieved, read, or edited by the assistant
- [x] Trashed and deleted pages never appear in retrieval
- [x] Background indexer with progress events; one at a time; stops when the workspace changes
- [x] Trace viewer: recent assistant runs with status, steps, time and token counts, and per-run tool traces
- [x] Regression dataset: 40 synthetic cases (20 dev, 20 held-out) with thresholds committed before the held-out run
- [x] Measured results reported against those gates, including the failures
- [ ] Passing gates. Four held-out gates fail (see [eval/README.md](eval/README.md))
- [ ] A corpus large enough to show a semantic benefit. The current one is too small and too easy for keywords to fail
- [ ] Human rubric review of answers. Not done by anyone other than the author
- [ ] Provider adapter tests for the embedding path beyond the scripted-server checks

### Milestone 4: meeting knowledge

- [x] Transcript import: pasted text, `.txt`, WebVTT and SRT, with timestamps and speakers where present
- [x] Each meeting is a page (searchable, linkable) plus timestamped segments
- [x] Extraction in JSON mode: summary, decisions, open questions, action items
- [x] Every claim must cite segments that exist and share a content word with them; unsupported claims are dropped and counted
- [x] Action items arrive as one reviewable proposal, with the source meeting page attached
- [x] Due dates are kept only when the transcript states that exact date
- [x] Explicit failure states: invalid model output, cancelled, model not installed
- [ ] Audio import. The command exists and reports that no transcription engine is configured. No engine is bundled or configured
- [ ] Live check of extraction quality. Covered by unit tests on validation only
- [ ] Speaker identification beyond labels in the transcript

### Milestone 5: recurring workflows

- [x] Recipes: name, instructions, manual, daily or weekly schedule, explicit IANA timezone, enable and pause
- [x] Scheduling is DST-aware: gap times move forward an hour, overlapping times use the first occurrence
- [x] Each scheduled slot runs at most once, enforced by a unique key, including across restarts
- [x] A missed schedule produces one catch-up run for the latest slot, never a burst
- [x] Results are draft proposals; nothing is written to a page without review
- [x] Run history with trigger, status, duration and a link to each draft
- [x] Recipes run only while the app is open. The UI says so
- [ ] A background service for running while the app is closed (not built, excluded by the brief for this release)
- [ ] Live end-to-end test of a scheduled run with a real model. Covered by unit tests on scheduling and claims

### Milestone 6: release polish

- [x] Backup and restore verified by tests, including checksum tampering and non-empty destinations
- [x] Secret scan over tracked files, run in `npm run check` and CI
- [x] Acceptance map: each release gate linked to the test that enforces it, in [docs/acceptance.md](docs/acceptance.md)
- [x] Measured: search p95 at 5,000 pages; installer sizes; cold start and idle memory (see Performance)
- [ ] Keyboard and screen-reader review with assistive technology. Done by inspection only
- [ ] Clean-machine install of the MSI and NSIS packages
- [ ] Code signing. None; the installers are unsigned
- [ ] Autosave latency measured in the UI

Explicitly excluded from the initial release (per `intent.md`): collaboration, cloud sync, SSO, shell execution, browser
automation, billing, always-on cloud agents, and a background service. The sidebar and menus show no buttons for these.

## Requirements

- Windows 11 (verified). macOS and Linux are not verified.
- Node.js 22 or newer and npm.
- Rust stable (`rustup`), MSVC build tools (Visual Studio Build Tools with the C++ workload and a Windows SDK).
- WebView2 runtime (ships with Windows 11).

## Assistant setup (local model)

Threadwell does not bundle model weights or start a model server. Install a local server, then pull a model:

1. Install [Ollama](https://ollama.com). It runs on `http://127.0.0.1:11434` by default.
2. Pull a model, for example `ollama pull qwen2.5:3b` (about 1.9 GB). Small models make more mistakes than larger
   ones, so check answers against their sources.
3. In Threadwell, open **Settings → AI assistance**, enter the model name, and choose **Save and check**.

Nothing is sent to a model until you use the assistant. Remote servers are off by default. Turning them on sends your
notes to that server, and only https is accepted.

## Setup

```bash
npm ci
```

`npm ci` installs exactly the versions in `package-lock.json`. Rust dependencies are pinned in `src-tauri/Cargo.lock`.

## Run

```bash
npm run dev
```

This starts Vite on port 1420 and opens the desktop window through `tauri dev`. The first build takes several minutes
because SQLite is compiled from source.

## Test

```bash
npm run check
```

`check` runs, in order:

| Command                | What it covers                                                                                                                                                                                                                                                                                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `npm run typecheck`    | TypeScript strict mode, no unused locals                                                                                                                                                                                                                                                                                                                                                         |
| `npm run lint`         | ESLint with typescript-eslint recommended rules                                                                                                                                                                                                                                                                                                                                                  |
| `npm run format:check` | Prettier                                                                                                                                                                                                                                                                                                                                                                                         |
| `npm test`             | Frontend unit tests (Vitest, jsdom): tree building, breadcrumbs, search snippets, task grouping, error mapping, app shell smoke test                                                                                                                                                                                                                                                             |
| `npm run test:rust`    | Backend tests (`cargo test`): migrations and newer-schema refusal, persistence across reopen, page revisions and conflicts, trash and restore, cycle prevention, link extraction, workspace isolation, task validation, search query building, Markdown round trip, export naming and CSV escaping, import rules, backup and restore round trip, tampered-backup and non-empty-restore rejection |

CI (`.github/workflows/ci.yml`) runs the same steps on `windows-latest` for every push and pull request.

### Live model check (not in the default run)

Needs Ollama running with `qwen2.5:3b`. It asks a grounded question about a seeded page and checks the answer and its
citation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml live_ollama -- --ignored --nocapture
```

Last run on the build machine: passed, in about 49 seconds on CPU. The answer was grounded and cited the source page.
The same model first answered without searching, which is why retrieval now runs before the first model call.

### Not automated

- The end-to-end scenarios in `intent.md` section 13 (create, edit, restart; approved task creation) need a WebDriver
  harness for the Tauri window. That harness is not set up. These flows were checked manually and are listed as unverified below.

## Build a Windows installer

```bash
npm run tauri build
```

Output is written to `src-tauri/target/release/bundle/` (MSI and NSIS). Installers are **unsigned**. Do not describe
them as signed or verified by a publisher.

### Windows artifact

Built on Windows 11 with `npm run tauri build` (release profile, LTO). These are the final numbers for the current build:

| File                                                                  | Size     | Signed |
| --------------------------------------------------------------------- | -------- | ------ |
| `src-tauri/target/release/bundle/msi/Threadwell_0.1.1_x64_en-US.msi`  | 4.07 MiB | No     |
| `src-tauri/target/release/bundle/nsis/Threadwell_0.1.1_x64-setup.exe` | 3.07 MiB | No     |

The release executable starts, stays responsive, and shows a window titled "Threadwell". Installing the MSI or NSIS
package on a clean machine has not been tested.

## Data and privacy

- Your workspace is the folder you choose. It contains `threadwell.db` and `attachments/`.
- Back up with **Settings → Create backup**. The backup is a readable folder that includes your data in plain form.
- Threadwell collects no telemetry and makes no network requests in this build.
- There are no API keys or secrets in this milestone. The `.env` pattern is kept for later milestones; `.env` is
  git-ignored and `.env.example` documents the policy.

## Performance

Measured so far (Windows 11, release build of the Rust backend, warm disk cache, one machine):

| Target in `intent.md`                  | Measured                                                                                            | Notes                                                                                                                                                      |
| -------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Search p95 below 300 ms at 5,000 pages | **p50 1.50 ms, p95 2.75 ms** over 200 queries                                                       | Generated corpus: 5,000 pages of about 180 words each, deterministic vocabulary. Run with `cargo test --release -- --ignored --nocapture measures_search`. |
| Installer size                         | MSI 2.56 MiB, NSIS 1.92 MiB                                                                         | Unsigned. Requires WebView2 (preinstalled on Windows 11).                                                                                                  |
| Bulk page creation                     | Not a target, but slow: 5,000 pages took about 190 s to create and update through the service layer | Each create and update runs its own transaction and index write. Batching bulk imports is the next step if this matters.                                   |

Measured in milestone 6 (Windows 11, release build, same machine as the model server; 3 launches):

| Target in `intent.md`                  | Measured                                                                                                                                                   | Result                                                            |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Cold usable window within 3 s          | Window visible in 80 ms (median of 3); 452 ms on the first launch after install. "Usable" (first page rendered and interactive) is not measured separately | Met for the window; usability not timed                           |
| Idle app memory below 250 MB           | App process alone: 26 MiB. Whole process tree (app plus 6 WebView2 processes), working set after 20 s idle: **336 MiB**                                    | **Not met** on working-set accounting. Private bytes not measured |
| Autosave within 1 s after typing stops | Debounce is 800 ms in code. End-to-end time in the UI not measured                                                                                         | Not measured                                                      |
| Installer size, published              | MSI 4.07 MiB, NSIS 3.07 MiB (version 0.1.1). Requires WebView2 (preinstalled on Windows 11)                                                                | Reported                                                          |

The bundle is about 725 kB before gzip, mostly the editor. Lazy-loading the editor is the next candidate if memory or
startup needs to come down.

## Verification

Verified on Windows 11 Pro (build 26200) with Node 24, Rust 1.99 stable, and MSVC toolchain. Current counts:

- Rust: 107 tests pass with `cargo test`, including 34 assistant tests against a scripted Ollama server. Three are ignored by default: the search benchmark, the live Ollama check, and the evaluation harness.
- Frontend: 17 tests pass, including assistant panel states and diff rendering.
- Frontend: typecheck, lint and `vite build` pass.
- Release build: `npm run tauri build` completes and produces the MSI and NSIS bundles listed above.
- Release executable: launched on the build machine; it stayed alive and responsive with the window titled "Threadwell".
- `npm run check` exits 0 (typecheck, lint, format check, frontend tests, Rust tests). The frontend and backend
  test counts are listed above.

### Unverified (needs a person or a platform check)

- Running the installed app through a full create, edit, restart cycle on a clean machine.
- Keyboard-only and screen-reader workflows.
- Behaviour of the installer on machines without WebView2 (it is expected to bootstrap it; not tested).
- macOS and Linux builds (the code is portable but has not been compiled there).
- The board's behaviour with very large task counts, and search at the 5,000-page target.
- Code signing: none. Windows SmartScreen will warn on unsigned installers.

## Project layout

```
src/                 React UI
  components/        screens and editor
  lib/               typed API wrappers, pure helpers, types
src-tauri/
  src/               Rust backend: commands, pages, tasks, search, transfer, markdown, db
  migrations/        ordered SQL migrations
  capabilities/      Tauri permissions (dialog only)
tests/               Vitest suites
docs/                architecture and design notes
```

## Conventions

- Workspace conventions are in `C:\MyProjects\CLAUDE.md`. This project is its own git repository.
- Commit messages explain why. Small, scoped commits.
- Do not commit `.env` or any credentials.
