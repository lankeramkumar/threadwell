# Threadwell

Project-specific notes for Claude Code. Workspace conventions live in `C:\MyProjects\CLAUDE.md`.

- Product brief: `intent.md` (source of truth for scope). Milestone 1 is implemented; later milestones are not started.
- Stack: Tauri 2 (Rust, `src-tauri/`), React + TypeScript + Vite (`src/`), Tiptap, SQLite via rusqlite with FTS5.
- Run: `npm run dev`. Full gate: `npm run check` (typecheck, lint, format, Vitest, cargo test).
- Backend commands are the only path to files and the database. Validate ids, paths and payloads in Rust.
- Keep the app offline: no network code in milestone 1. Do not add fs/shell/http Tauri plugins; use the dialog plugin
  for paths and validate them in Rust.
- New schema changes go in a new numbered file under `src-tauri/migrations/`; never edit an applied migration.
- Do not display buttons for features outside the current milestone.
