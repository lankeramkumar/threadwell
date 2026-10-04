# Threadwell

Threadwell is a local-first desktop workspace for notes, projects and tasks. It is built with Tauri 2 (Rust), React,
TypeScript, Tiptap and SQLite. Your data stays in a folder you choose. No account or internet connection is needed.

The full product brief is in [intent.md](intent.md). This README tracks what is built, what is verified, and what is
not done yet. The design is in [docs/architecture.md](docs/architecture.md).

## Status

Milestone 1 (desktop foundation) is implemented. Milestones 2 through 6 are not started. Do not treat this build as the
complete product.

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

### Later milestones (not started)

- 2 AI assistance: provider configuration, streaming, retrieval with citations, reviewed proposals, undo
- 3 Knowledge quality: semantic retrieval, evaluation dataset, trace viewer
- 4 Meetings: transcript import, sourced summaries, audio import through a configured engine
- 5 Recurring workflows: recipes, local scheduling, missed-run handling
- 6 Release polish: accessibility review, performance report, signed packaging

Explicitly excluded from the initial release (per `intent.md`): collaboration, cloud sync, SSO, shell execution, browser
automation, billing, always-on cloud agents. The sidebar and menus show no buttons for these.

## Requirements

- Windows 11 (verified). macOS and Linux are not verified.
- Node.js 22 or newer and npm.
- Rust stable (`rustup`), MSVC build tools (Visual Studio Build Tools with the C++ workload and a Windows SDK).
- WebView2 runtime (ships with Windows 11).

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

Built on Windows 11 with `npm run tauri build` (release profile, LTO):

| File                                                                  | Size     | Signed |
| --------------------------------------------------------------------- | -------- | ------ |
| `src-tauri/target/release/bundle/msi/Threadwell_0.1.0_x64_en-US.msi`  | 2.56 MiB | No     |
| `src-tauri/target/release/bundle/nsis/Threadwell_0.1.0_x64-setup.exe` | 1.92 MiB | No     |

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

Not yet measured: cold window time, autosave latency in the UI, idle memory. The frontend bundle is about 725 kB before
gzip (Tiptap and ProseMirror dominate). Lazy-loading the editor is the first candidate if cold start misses its target.

## Verification

Verified on Windows 11 Pro (build 26200) with Node 24, Rust 1.99 stable, and MSVC toolchain:

- Rust: 40 tests pass with `cargo test`, no compiler warnings. One further ignored test measures search latency.
- Frontend: typecheck, lint, Vitest (14 tests) and `vite build` pass.
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
