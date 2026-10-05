# Threadwell

Project-specific notes for Claude Code. Workspace conventions live in `C:\MyProjects\CLAUDE.md`.

- Product brief: `intent.md` (source of truth for scope). Milestones 1 and 2 (local Ollama) are implemented.
- Stack: Tauri 2 (Rust, `src-tauri/`), React + TypeScript + Vite (`src/`), Tiptap, SQLite via rusqlite with FTS5.
- Run: `npm run dev`. Full gate: `npm run check` (typecheck, lint, format, Vitest, cargo test).
- Backend commands are the only path to files and the database. Validate ids, paths and payloads in Rust.
- Network: the only outbound HTTP is the Ollama adapter (src-tauri/src/ai/provider.rs), limited by the endpoint policy in
  ai/config.rs (loopback unless remote is enabled, https for remote). Do not add fs/shell/http Tauri plugins; use the dialog
  plugin for paths and validate them in Rust.
- The assistant never writes content directly. Model output becomes a proposal row (ai/proposals.rs) that the user applies.
- New schema changes go in a new numbered file under `src-tauri/migrations/`; never edit an applied migration.
- Do not display buttons for features outside the current milestone.
