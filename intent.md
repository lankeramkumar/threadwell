# Threadwell: intent for a local-first AI desktop workspace

## Instructions to Claude Code

Build a working desktop application from this brief, not a marketing website. Inspect the existing repository and its instructions before choosing implementation details. If starting from an empty repository, use the default stack below. Implement milestone 1 first, verify it, then proceed in order. Make routine implementation decisions autonomously and document important tradeoffs. Ask only when a missing decision prevents meaningful progress. Do not treat this document as authorization to deploy, purchase services, or send external messages.

The complete product vision includes every milestone below. Do not claim full completion after delivering only the first milestone. Each milestone must leave the application usable. Keep a progress checklist and record implemented, deferred, and unverified requirements in the README.

## 1. Purpose

Create a lightweight desktop workspace where people write notes, organize projects and tasks, find answers in their own knowledge, and use AI to perform reviewed work within that workspace.

Product name: **Threadwell**. Use this name consistently in the application title, onboarding, documentation, and packaging. Trademark and domain availability have not been verified.

Primary users: individual developers, students, freelancers, and knowledge workers. The initial product is single-user. One installation can hold several separate workspaces, each its own folder and database. It should also demonstrate strong applied AI engineering through grounded retrieval, structured tool use, safe mutations, failure recovery, and repeatable evaluation.

Product promise: "Your notes, projects, and AI assistant in one desktop workspace."

Direction (set October 5, 2026): the main use is asking questions about the material people already work with. A software engineer links a project folder, or a person links a folder of documents, and asks the assistant about it. Each answer cites the file it came from. Notes, tasks and recipes support that use. They are not the centre.

## 2. Reference and interpretation

Reference reviewed October 4, 2026: https://www.notion.com/product/ai

Observed product themes: agents that create and edit pages/databases; search across workspace and connected apps; meeting transcription and summaries; recurring custom agents; writing assistance; database autofill; provider flexibility; usage visibility and permissions.

Use these functional themes as inspiration. Create original branding, icons, text, and application design. The reference is a product page, not a complete application specification. The requirements below are our proposed desktop adaptation, not assertions about Notion's internal implementation.

Notion already has desktop clients. Our differentiation is local workspace ownership, offline editing/search, inspectable AI evidence, reviewed agent changes, and straightforward backup/export.

## 3. Scope and defaults

- Windows 11 first; keep architecture portable to macOS and Linux. Verify builds only on available operating systems and report the others as unverified.
- Local-first persistence; no account or hosted database required for the initial release.
- Notes, tasks, search, export, and viewing previous conversations work without internet.
- Several workspaces per installation. Pages, tasks, meetings, recipes and assistant history never mix between workspaces. The assistant reads another workspace only when the user selects it, read-only.
- Cloud AI uses the user's own API key. Optional local inference connects to an existing local model server; model weights are not bundled.
  *Status (0.1.4):* only the local model server adapter (Ollama) is built. The cloud adapter is not built yet.
- AI is integrated into the editor and project workflow, not isolated in a generic chat screen.
- Default to one agent with a bounded tool loop. Introduce additional agents only if evaluation demonstrates a need.
  *Status (0.1.4):* an optional several-agent mode is built and was compared on held-out splits (see eval/README.md). It did not win on the gates, so one agent remains the default. The user may choose the other mode.

Later, explicitly excluded from the initial release: multi-user collaboration, cloud synchronization, enterprise SSO, arbitrary shell execution, autonomous browser control, remote connectors, billing, and always-on cloud agents. Do not display nonfunctional buttons for these features.

## 4. Essential user journeys

### Write and organize
Create a workspace, create nested pages, write formatted notes, link related pages, and organize tasks into table and board views. Changes survive restart and unexpected process termination within the stated autosave window.

### Ask with evidence
Ask "What did we decide about authentication?" The assistant searches the selected workspace (and any other workspace the user explicitly selects, read-only), answers with clickable page/block citations, identifies conflicting decisions when present, and acknowledges missing evidence. Opening a citation navigates to the relevant content.

### Turn notes into work
Ask "Create tasks from this planning page." The assistant proposes task titles, descriptions, and source references. Unknown deadlines remain unset. The user reviews the proposed changes, applies them, and can undo that run.

### Ask about linked files
Link a project folder or a folder of documents (Sources). Threadwell reads the files, keeps them in step while it is open, and answers questions about them, such as "How are duplicate charges prevented?". The answer cites the file it came from. Linked files are read only, and nothing is written to the folder.

### Edit with AI
Select text and request a rewrite, summary, expansion, or translation. Show a before/after diff with accept and reject actions. Preserve numbers, names, and factual meaning unless the requested transformation requires a change.

### Process a meeting
Import a transcript, then optionally an audio recording. Produce a meeting page with a summary, decisions, unresolved questions, and proposed action items. Link extracted claims to transcript passages or timestamps. Audio processing requires a configured transcription provider or supported local engine.

### Automate recurring work
Create a recipe such as "Draft a weekly update from completed tasks." Run it manually or schedule it. Scheduled runs operate only while the application is running unless a separately implemented background service is enabled. Missed schedules produce one catch-up draft when appropriate, not a burst of duplicate runs.

## 5. Application experience

Use a restrained productivity interface: readable typography, neutral colors, consistent spacing, subtle borders, and light/dark themes. Support keyboard navigation and accessible labels, focus states, contrast, and reduced motion.

Layout:
- Left sidebar: search, pages by workspace, sources, projects, meetings, automations, settings, favorites, and trash.
- Center: document editor, task table/board, search results, or meeting page.
- Collapsible right panel: AI conversation, selected context, citations, and proposed changes.
- Command palette: create/open pages, search, switch views, invoke supported AI actions.

Editor: headings, paragraphs, lists, checklists, quotes, code blocks, links, and basic tables. Provide slash commands, autosave status, breadcrumbs, undo/redo, and Markdown import/export. Use a maintained editor library rather than implementing rich text from scratch.

Show distinct loading, empty, offline, indexing, cancellation, quota, authentication, and error states. Streaming AI output must not freeze editing. The user can cancel a run. Surface concise operation summaries and tool results; do not fabricate or expose hidden model reasoning.

First launch: choose a local workspace and optionally load a clearly labeled sample project. AI configuration is optional. Without credentials, the workspace remains useful; show a labeled recorded demo or deterministic fixture mode separately from live AI.

## 6. Architecture

Default stack for an empty repository:
- Tauri 2 desktop shell with Rust backend.
- React, TypeScript, and Vite frontend.
- Tiptap or another maintained structured editor.
- SQLite for authoritative local persistence and FTS5 lexical search.
- A modest design system using accessible components and CSS variables.
- A schema validation library for tool requests and model outputs.

Avoid a bundled Python runtime, browser engine, Docker dependency, or separate database service for the normal desktop installation. Reuse an established stack if the repository already has one and explain any material difference.

Logical flow:
UI -> typed desktop commands -> application services -> SQLite/files
AI orchestration -> context retrieval -> provider adapter -> validated tool requests -> reviewed mutation service

All filesystem access, secrets, database writes, and provider requests belong behind controlled backend interfaces. Validate paths, identifiers, and payloads in the backend. Tauri capabilities do not substitute for validation in custom backend commands.

Define narrow provider contracts for generation/streaming, embeddings, and transcription. Implement one real cloud generation adapter first. Keep provider-specific payloads out of the editor and domain models. Pin dependencies and document verified setup instructions rather than assuming remembered APIs are current.

## 7. Data model and ownership

Persist workspaces, pages, blocks, page links, projects, tasks, meetings, transcript segments, conversations, messages, agent runs, tool events, change proposals, recipes, schedules, and migrations.

Linked source folders are recorded per workspace. Each readable file in a linked folder is stored as a read-only page that records its folder and relative path.

Use stable IDs, timestamps, workspace IDs, and revision numbers. Tasks initially have title, description, status, priority, optional due date, project, and source references. Pages have parent, title, structured body, revision, and soft-delete state. Avoid a general-purpose database/formula engine in the first release; introduce extensible properties after basic task views work.

SQLite is authoritative. Search indexes are derived and rebuildable. Attachments live in a workspace-managed directory. Imports copy approved files; never silently move or alter originals. Export Markdown, tasks as CSV, and a versioned full-workspace backup. Implement and verify restoration. Backup consistency must account for SQLite WAL, preferably using the database backup API.

## 8. Retrieval and grounding

Start with lexical search, then add semantic retrieval in milestone 3. Chunk by meaningful block/section boundaries and preserve page/block IDs, revisions, headings, and source locations.

Use hybrid ranking with configurable lexical/vector weights. Prefer a simple embedded vector index; do not require a hosted vector database. Reindex changed pages, exclude deleted pages promptly, and invalidate stale embeddings by content hash and embedding-model identity.

Every citation must correspond to a retrieved source ID, not a model-invented filename. Citations to linked files name the file's relative path. For source code, a citation also names the section it drew on, such as "src/charge.rs · Lines 41–80", and clicking it brings that section into view. Documents cite the file, and where the document has headings, the heading. Validate citation IDs before rendering. If sources changed after retrieval, indicate staleness or rerun retrieval. Treat page, import and linked-file content as untrusted data, not instructions granting tool access. Linked-file pages are never editable from the app; a change to the file arrives through the next sync. Keep retrieval scoped to the active workspace and user-selected context. Other workspaces are searched only when the user selects them, by keyword, read-only, and each hit is labelled with its workspace name. Allow exclusion of pages from AI context.

## 9. Agent behavior and mutation protocol

Initial tools: search_workspace, read_page, list_tasks, propose_create_page, propose_edit_page, and propose_task_changes. Add tools only alongside validated schemas and meaningful evaluation cases.

The model proposes domain actions; it never sends raw SQL or shell commands for execution. Limit run duration, tool steps, output sizes, and retries. Permit at most two controlled recovery attempts for a recoverable tool failure before presenting an actionable error.

All AI-originated writes are proposals. Before applying, show the affected pages/tasks and a readable diff. Apply approved changes transactionally, with idempotency keys and revision checks. If content changed meanwhile, reject the stale proposal and offer regeneration. Save enough information to undo the approved run; if subsequent edits conflict, offer a reviewed restoration rather than overwriting them.

Cancellation stops future model/tool work and must not apply pending proposals. Proposals are created only for the open workspace, never for another one. Recording a read-only trace is allowed; changing workspace content requires the described review flow.

Recurring recipes create drafts/proposals by default. Record trigger, run status, duration, provider/model, tool results, and error category. Scheduling uses an explicit timezone and daylight-saving-aware behavior. Avoid claiming 24/7 execution when the process is closed or the device is asleep.

## 10. Meetings, privacy, and credentials

Milestone 4 supports pasted/imported transcripts before live capture. Audio import needs supported formats, progress, cancellation, provider error handling, and timestamp alignment. Live microphone/system-audio capture is a later extension with visible recording controls and participant-consent UX; never silently record.

Store API keys in the operating system credential store, not localStorage, exported backups, or plaintext config. Local endpoint addresses are configurable; allow remote endpoints only through explicit settings. State which selected context/audio is transmitted when using cloud providers. Do not label cloud processing as offline or promise provider retention/training terms without checking the configured service.

Dropping a file or folder on the window only links a folder or imports a Markdown or text file. It does not move or change the original. Linked folders are read only. Threadwell never writes to them, and skips build output, dependencies, hidden files, lock files, binaries, and files over 1 MB (code) or 5 MB (documents). Syncing happens while the app is open.

Do not collect telemetry by default. Local performance traces are opt-in: they are written to a file on the user's computer in OpenTelemetry format, record names, counts and timings but no prompts, answers or page text, and can be deleted. Never log API keys. Diagnostic traces should minimize sensitive content and support deletion. Render imported HTML/Markdown and model output safely, restrict external navigation, and sanitize filenames and paths. Make no enterprise security certification claims.

## 11. Delivery milestones

1. **Desktop foundation:** installable development build, migrations, page tree/editor, autosave, tasks with table/board views, lexical search, export/import, settings, sample workspace. Works offline and survives restart.
2. **AI assistance:** real provider configuration, streaming conversation, current-page actions, workspace retrieval with citations, task/page proposals, approval, revision checks, cancellation, and undo.
3. **Knowledge quality:** semantic indexing, hybrid retrieval, exclusion controls, index rebuilds, trace viewer, regression dataset, latency/token reporting, provider adapter tests. Show measured changes against lexical-only retrieval.
4. **Meeting knowledge:** transcript import, sourced summaries and proposed tasks, audio import through a configured transcription adapter, explicit failure states.
5. **Recurring workflows:** recipe editor, manual runs, local scheduling, missed-run handling, draft review, history, and duplicate prevention.
6. **Release polish:** keyboard/accessibility review, backup restoration, performance measurements, Windows packaging, setup documentation, reproducible demo, and final acceptance verification.

Status after milestone 6: milestones 1 to 6 are built and released as 0.1.3. Version 0.1.4 adds several workspaces with scoped assistant retrieval and the several-agent fixes. Remaining gaps are listed in docs/STATUS.md.

7. **Linked sources and project questions:** link folders, read documents and source code, keep linked files in step while the app is open, cite the file behind each answer, and evaluate questions about code on a pre-registered split. Built: linking, read-only mirroring, file citations with section labels for code, drag and drop of folders and Markdown or text files, "Ask about this file" and folder questions, and a sample project that new workspaces can include. Still to do: a code-question evaluation split, and syntax highlighting in the file view.

Future extensions: selected-folder indexing, authenticated connectors, custom task properties, live audio capture, optional background service, and collaboration. Document connector authentication and authorization requirements before implementing external access.

## 12. Performance targets

These are targets to measure, not promised results:
- Cold usable window within 3 seconds on a documented reference machine.
- Search p95 below 300 ms for 5,000 representative pages after indexing.
- Autosave within 1 second after editing stops, with visible failure/retry behavior.
- Idle app memory target below 250 MB, excluding a separately running local model server.
- Keep the installer small; publish actual size and required system webview prerequisites.

Run indexing/transcription outside the UI thread with progress and cancellation. Debounce index updates. Document dataset size, hardware, measurement method, and whether caches were warm. Report deviations honestly.

## 13. Verification and AI evaluations

Use meaningful tests for persistence, migrations, proposal approval, stale revisions, rollback/undo, workspace isolation, cancellation, idempotency, backup restoration, and secret handling. Add UI integration coverage for create-edit-restart, cited search, and approved task creation.

Create at least 40 synthetic evaluation cases with reference sources and expected outcomes: grounded questions, missing answers, conflicting sources, task extraction, unsupported deadlines, prompt injection in pages, malformed tools, stale content, duplicate retries, and cancelled runs. Split development and held-out cases; label recorded/fixture tests separately from live-model evaluations.

Measure retrieval relevance, citation validity, answer support, task-field precision/recall, abstention, mutation approval compliance, latency, and available token usage. A rubric and human spot checks are required for semantic quality; an LLM judge alone is insufficient. Preserve failures and run configurations. Do not fabricate benchmark results or costs when provider usage data is unavailable.

Release gates: no writes without approval in the automated suite; no retrieval from another workspace unless the user selected it, checked in isolation tests; every rendered citation resolves to its source; no plaintext secrets in checked artifacts; successful backup/restore round trip. Declare thresholds for probabilistic quality before evaluating the held-out set and report the results.

## 14. Recruiter demo and definition of done

Provide a sample project containing related notes, conflicting decisions, tasks, and a meeting transcript. A three-minute demo should show:
1. Edit a note and reopen the app to demonstrate persistence.
2. Ask a project question and navigate a supporting citation.
3. Ask about missing information and show an appropriate refusal to invent it.
4. Create tasks from a page, inspect the proposed changes, apply, and undo.
5. Show meeting action items linked to transcript evidence.
6. Run a weekly-update recipe and inspect its draft and run history.

Deliver source code, dependency lockfiles, setup/build instructions, architecture notes, schema/migration documentation, tests, evaluation data and measured report, performance results, and a Windows release artifact when the environment supports building it. Document signing status; do not call an unsigned artifact signed or verified.

Done means the implemented milestone works end to end with persistent data and real configured AI where required. Mock screens, disconnected buttons, placeholder citations, and simulated agents do not count as implemented features. If credentials or platform tooling are unavailable, preserve the implementation, verify what can be verified, and explicitly list the remaining live/platform checks.
