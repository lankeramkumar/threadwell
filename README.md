# Threadwell

**Your notes, projects and an AI assistant in one desktop workspace.**

Threadwell is a Windows desktop app for asking questions about your documents and project folders, writing notes,
organising pages and tasks, and searching everything you have written. Your data stays in a folder on your computer. There is no account, no sign-in and no cloud service. An optional
AI assistant runs on a model you install on your own machine.

[![CI](https://github.com/lankeramkumar/threadwell/actions/workflows/ci.yml/badge.svg)](https://github.com/lankeramkumar/threadwell/actions/workflows/ci.yml)

> **Read this first.** The notes, tasks, search, import, export and backup features are ready to use. The AI assistant
> is experimental: on our latest held-out split, four of its eight gated quality measures fail. Check the sources it cites, and
> nothing it suggests is saved until you click **Apply**. The installers are not code-signed, so Windows will warn you.

---

## Contents

- [What you can do](#what-you-can-do)
- [Download and install](#download-and-install)
- [Quick start: your first 15 minutes](#quick-start-your-first-15-minutes)
- [How to use each feature](#how-to-use-each-feature)
- [Set up the AI assistant](#set-up-the-ai-assistant)
- [Your data, backups and privacy](#your-data-backups-and-privacy)
- [Troubleshooting](#troubleshooting)
- [Build from source](#build-from-source)
- [Project documentation](#project-documentation)
- [Project status and limits](#project-status-and-limits)
- [License](#license)

---

## What you can do

| Area                        | What it does                                                                                                                                                                                                                                                                                                                                                            |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pages**                   | Nested pages with a rich editor: headings, lists, checklists, quotes, code, tables, links between pages. Autosave. Favourites, move, trash and restore. Attach files (up to 25 MB) that are stored inside the workspace. Attached Word, PDF, CSV, Markdown and text files are also read into a note under the page, so the assistant can answer from them.              |
| **Tasks**                   | Tasks with status, priority, optional due date and project. Table view and board view with drag and drop. Export to CSV.                                                                                                                                                                                                                                                |
| **Search**                  | One search box across pages and tasks, with highlighted matches. Ctrl+K command palette for jumping to any page or view.                                                                                                                                                                                                                                                |
| **Import and export**       | Import single files or a whole folder of notes (Markdown, text, Word, text-based PDF and CSV). Watch a folder and sync its new notes. Export every page to Markdown files.                                                                                                                                                                                              |
| **Backup**                  | Full workspace backup to a folder, checksummed, and restore into an empty folder.                                                                                                                                                                                                                                                                                       |
| **AI assistant** (optional) | Ask questions about your notes and get answers with numbered sources. Use the selected text as context, search past conversations, and rewrite, summarise, expand or translate selected text. Suggested changes are reviewed before anything is saved.                                                                                                                  |
| **Meetings** (optional)     | Import a meeting transcript (text, WebVTT or SRT), or a recording through a local transcription engine you install. Get a summary, decisions, open questions and action items, each linked to the lines it came from.                                                                                                                                                   |
| **Recipes** (optional)      | Saved instructions that draft a page on a daily or weekly schedule while the app is open.                                                                                                                                                                                                                                                                               |
| **Linked folders**          | Link a project folder or a folder of documents, by button or by dragging it onto the window. Threadwell reads the files (source code and configuration, Markdown, text, Word, text-based PDF, CSV) and keeps them in step while it is open. Ask the assistant about them: each answer names the file and, for code, the lines it came from. Linked files are read only. |
| **Workspaces**              | Several separate workspaces, each with its own pages, tasks and history, switchable in Settings. The assistant can answer from one, or from all of them.                                                                                                                                                                                                                |

Everything except the assistant, meetings and recipes works with no internet connection and no AI model installed.

## Download and install

1. Go to the [Releases page](https://github.com/lankeramkumar/threadwell/releases) and download one of:
   - `Threadwell_0.1.5_x64_en-US.msi`: Windows Installer package
   - `Threadwell_0.1.5_x64-setup.exe`: setup program
2. Run it. If Windows SmartScreen says it protected your PC, click **More info**, then **Run anyway**. This happens
   because the installer is not code-signed.
3. Start **Threadwell** from the Start menu.

**Requirements:** Windows 11. Threadwell uses Microsoft Edge WebView2, which Windows 11 includes. macOS and Linux are
not supported yet.

## Quick start: your first 15 minutes

1. **Create a workspace.** On first launch, keep _Create a workspace_ selected, type a name, click **Choose folder…**
   and pick an empty folder. Leave _Include the labelled sample project_ ticked to get example pages and tasks, then
   click **Create workspace**.
2. **Look around.** The left sidebar has your pages tree, Tasks, Meetings, Recipes, Trash and Settings. The centre is
   where you work. The right panel is the assistant.
3. **Import the demo notes.** Open **Settings** → **Import a folder of notes** → **Choose folder…** and select
   `samples/demo-project`. Tick **Select all importable** and click **Import 40 notes**. The demo has ten Markdown, text,
   Word and PDF notes on each of ten topics, so you can try search and the assistant on real-looking material.
4. **Try a meeting.** Open **Meetings** → **Import a transcript** → **Import file…** and choose
   `samples/demo-project/meetings-vtt/meeting-food-safety-audit.vtt`. Then click **Extract notes and actions** (this
   needs the AI model, see below).
5. **Make a backup.** Open **Settings** → **Create backup**, and pick a folder outside your workspace.

The step-by-step [Word user guide](docs/Threadwell-User-Guide.docx) walks through every feature with the exact button
names. The sample files are described in [samples/README.md](samples/README.md).

## How to use each feature

### Pages

- **Create:** click **+** next to _Pages_ in the sidebar. The page opens with the title selected.
- **Format:** use the toolbar above the page, or type **/** on an empty line for block commands (headings, lists,
  checklists, table, divider, code block).
- **Nest:** click **+** next to a parent page to create a sub-page, or change _Parent_ in a page's header.
- **Link:** choose a page from _Link to page…_ in the toolbar. The link opens that page when clicked.
- **Favourite:** click **Favorite** in the page header. Favourites appear in the sidebar.
- **Delete (reversibly):** click **Move to trash**. Restore it from **Trash**. There is no permanent-delete button yet.
- **Saving:** Threadwell saves about one second after you stop typing. The status line shows _Saved_, _Saving…_ or
  _Unsaved changes_. Keep the app open until it says _Saved_.

### Tasks

- Open **Tasks** and expand **Add a task**. Give it a title, priority, and optionally a due date and project.
- **Table** view lets you edit status, priority, due date and project in place.
- **Board** view shows three columns. Use ← and → on a card to move it.
- Due dates are never filled in for you. A task has no due date until you set one.
- Create a project under **New project**. Projects appear in the sidebar and filter the task list.

### Search

Type in the **Search** box at the top of the sidebar and press Enter. Results include pages and tasks, with the matching
words highlighted. Press **Ctrl+K** for the command palette, which lists pages and views by name.

### Import and export

- **Import:** _Settings_ → _Import and export_ → **Import a file**. A Markdown, text, Word (`.docx`), text-based PDF or CSV file of up to
  5 MB becomes a new page, so the assistant can answer from it. A first line starting with a single `#` becomes the title. Your original file is not changed.
- **Import a folder of notes:** _Settings_ → **Choose folder…**. Threadwell lists the Markdown, text, Word (`.docx`) and
  text-based PDF files in the folder. Tick the ones you want and click **Import**. Your files are never changed, and
  unchanged files are skipped on later imports. Scanned PDFs and OneNote (`.one`) files are not supported.
- **Export pages:** _Settings_ → **Export all pages to Markdown**, then choose an empty folder. Existing files are never
  overwritten.
- **Export tasks:** _Settings_ → **Export tasks to CSV**.

### Backup and restore

- **Back up:** _Settings_ → **Create backup**, then choose a folder outside your workspace. You get a folder named
  `threadwell-backup-…` containing a consistent copy of your data.
- **Restore:** _Settings_ → **Restore from backup…**, choose the backup folder, then choose an **empty** folder for the
  restored workspace. Your current workspace is not changed.

Back up regularly, and before any large import.

### The AI assistant (optional)

Once a model is set up (see below):

- **Ask:** type a question in the assistant panel and press Enter. Answers cite their sources with numbers such as
  `[1]`. Click a source to open that page. If the answer says the workspace does not contain the information, believe it.
- **Review suggestions:** when the assistant proposes a new page, an edit, or new tasks, a card shows a preview. Click
  **Apply** to save it, or **Reject** to discard it. After applying, **Undo** restores the previous content, as long as
  the page has not changed since.
- **Act on selected text:** select a passage, expand **Assistant on selected text**, choose _Rewrite_, _Summarize_,
  _Expand_ or _Translate_, and click **Run on selection**. Compare the original and the suggestion, then **Accept** or
  **Reject**. Numbers the suggestion dropped are flagged in red.
- **Keep a page private:** tick **Exclude from AI** in the page header. The assistant will not read or search it.
- **Choose which workspaces it answers from:** the **Answer from** list above the message box offers _This workspace only_,
  _All workspaces_, or one named workspace. Other workspaces are read-only for the assistant, and it never proposes changes
  to them. Proposed changes always go to the open workspace only.
- **Check what it did:** _Settings_ → **Assistant history** lists recent runs and each tool call.

### Meetings (optional)

1. Open **Meetings** → **Import a transcript**. Paste text with lines such as `[00:01:02] Ana: What we agreed`, or use
   **Import file…** for a `.txt`, `.vtt` or `.srt` file.
2. Open the meeting and click **Extract notes and actions**. This needs the AI model.
3. Read the extracted summary, decisions and questions. Each line shows the timestamps it came from.
4. Review the action items card. **Apply** creates the tasks. A due date appears only if the transcript states that
   exact date.

Audio recordings are not supported yet. Convert the recording to text first.

### Recipes (optional)

1. Open **Recipes** → **New recipe**. Give it a name and instructions, for example _Draft a weekly update from
   completed tasks. Group them by project._
2. Choose **Manual only**, **Daily** or **Weekly**, with a time (and a day for weekly). The timezone is filled in from
   your computer.
3. Click **Run now** to test it. Then open the run in the history and click **Review draft**.

Recipes run only while Threadwell is open. A missed run happens once when you next open the app.

## Video guides

Short silent videos of the app, with step lists in [docs/videos/README.md](docs/videos/README.md):

- [Getting around](docs/videos/01-getting-around.mp4) (about 32 seconds)
- [Linked folders and questions](docs/videos/02-linked-folders-and-questions.mp4) (about 1 minute 40 seconds)
- [Several workspaces](docs/videos/03-several-workspaces.mp4) (about 34 seconds)

## Set up the AI assistant

Threadwell does not include an AI model and does not install one for you. It talks to a model server on your own
computer. The supported server is [Ollama](https://ollama.com).

1. Install Ollama from ollama.com.
2. In a command window, run:
   ```bash
   ollama pull qwen2.5:3b
   ollama pull nomic-embed-text
   ```
   The first model is about 1.9 GB and is used for answers. The second, about 270 MB, is used for semantic search.
   For questions about documents, the larger model is more reliable. Run `ollama pull qwen2.5:7b` (about 4.7 GB) and choose
   `qwen2.5:7b` in the same settings. In our test on an attached Word file, the 3B model misstated what the file said, while
   the 7B model answered correctly. The 7B model is slower on a laptop.
3. In Threadwell, open **Settings** → **AI assistance**. Enter `qwen2.5:3b` as the model name and click **Save and check**.
   The status should say _Connected_.
4. Open **Settings** → **Finding information** and click **Build or update index**.

The default server address is `http://127.0.0.1:11434`. Remote servers are off by default. Turning them on sends your
notes to that server, and only `https` is accepted.

Small local models make mistakes. Use the assistant to find and summarise material, and check the sources it cites before
acting on an answer.

## Your data, backups and privacy

- **Where your data is:** in the workspace folder you chose. It holds `threadwell.db` (notes, tasks, meetings, settings)
  and an `attachments` folder. Threadwell also keeps its list of workspaces and the one to reopen in your user profile
  (`workspaces.json` and `last-workspace.txt`).
- **Network:** Threadwell makes no network requests by itself. The only outbound connection is to the model server you
  set up, and only when you use the assistant.
- **Telemetry:** none by default. Performance traces are opt-in and stay on your computer (Settings → Performance traces).
- **Secrets:** Threadwell stores no API keys. The repository has a secret scan in its checks.
- **Backups:** a backup is a readable folder containing your data in plain form. Store it somewhere you trust.

Do not edit `threadwell.db` by hand, and do not copy the workspace folder while Threadwell is open. Use **Create backup**.

### Linked folders (sources)

Link a folder when you want to ask questions about it, such as a software project or a folder of documents. The folder
itself is never changed.

1. Open **Sources** in the sidebar and click **Link a folder…**. Choose the project folder.
2. Threadwell reads it once. Each readable file becomes a read-only page titled with its path, such as `src/charge.rs`.
   Ignored: `.git`, `node_modules`, `target`, `dist`, `build`, `bin`, `obj`, `__pycache__`, `venv`, lock files, hidden
   files, binary files, files over 1 MB (code) or 5 MB (documents), and anything listed in the folder's `.gitignore`.
   Simple patterns are supported (`name`, `name/`, `*.ext`). Negated patterns (`!`) are not.
3. While Threadwell is open, it checks each linked folder about once a minute. Changed files are read again, new files
   are added, and deleted files are moved to Trash. Click **Sync now** to check immediately.
4. Ask the assistant a question, for example _How are duplicate charges prevented?_ Answers cite the file they came from.
   The source is searched like your notes, and the assistant can also search other workspaces you select.
5. **Unlink** removes the link. The files become ordinary pages in Trash, and the folder itself is not touched.

Limits: 5,000 files per folder. Citations name the file and, for code, the line section. The sync only runs while Threadwell is
open. A file that changes without changing its size or modified time is not noticed until its size or time changes.

Try it with the sample project in `samples/sample-repo`. New workspaces can also include a copy of it: tick the sample
option when you create a workspace, and the project is written to a `sample-project` folder inside the workspace and linked.

**Citations name the lines.** For source code, each citation names the section it drew on, such as
`src/charge.rs · Lines 41–80`. Click it and Threadwell brings that section into view. Documents cite the file, and the
heading where they have one.

**Drag and drop.** Drop a folder on the window to link it. Drop a Markdown or text file to import it as a note. Other file
types are reported, with the way to bring them in. The original files are never changed.

**Quick questions.** On a linked file, **Summarise this file**, **List open items** and **Explain how it works** put a
question in the assistant box, ready to send. On a linked folder, **Ask about this folder** does the same.

### Several workspaces

Each workspace is its own folder with its own pages, tasks, meetings, recipes and assistant history. Pages never move
between workspaces.

1. Open **Settings** → **Workspaces**. Your known workspaces are listed, with the open one marked.
   The sidebar's **Workspaces** section lists the pages of each workspace. Click a workspace to show its pages. Pages of
   another workspace are read only there, and clicking one switches to that workspace and opens it.
2. **Create or open another workspace** returns to the first-run screen, where you can create a new workspace in an
   empty folder or open an existing one.
3. **Switch** opens another workspace. Runs in progress in the old one are stopped first.
4. **Remove from list** forgets a workspace in Threadwell's list. Its folder and data are kept, and you can open it again
   later from **Create or open another workspace**.
5. **Rename the open workspace** changes the name shown in the sidebar.

### Deleting a workspace

Threadwell has no delete button for workspaces, because a workspace is a folder. To remove one:

1. Close Threadwell completely, including from the system tray. Back up first if you might need the data.
2. Delete the workspace folder in File Explorer. Deleting it is permanent, and it removes `threadwell.db`, its `-wal`
   and `-shm` files, and the `attachments` folder.
3. Optional: in Settings → Workspaces, click **Remove from list**. This also clears the entry from the list stored in
   `%APPDATA%\dev.threadwell.app\workspaces.json`.
   If the folder is gone, the app shows the first-run screen anyway.

Uninstalling Threadwell does not delete workspaces.

## Troubleshooting

| Problem                                     | What to do                                                                              |
| ------------------------------------------- | --------------------------------------------------------------------------------------- |
| SmartScreen blocks the installer            | Click **More info**, then **Run anyway**, for the file you downloaded.                  |
| _Couldn't save. Retrying._                  | Check that the disk has space and is writable. Threadwell keeps your edits and retries. |
| Assistant says _Model server not reachable_ | Start Ollama, then try again. Your notes are not affected.                              |
| Assistant says _Model not installed_        | Run `ollama pull <model name>`.                                                         |
| _Semantic search was unavailable_           | Run `ollama pull nomic-embed-text`. Keyword search still works.                         |
| A suggestion says _Out of date_             | The page changed after the suggestion was made. Ask again.                              |
| Restore refuses the folder                  | Choose a new or empty folder.                                                           |
| Import refuses the file                     | Check the extension (`.md`, `.markdown`, `.txt`) and that it is under 5 MB.             |

More answers are in the Word guide, section 13.

## Build from source

Prerequisites: Node.js 22 or newer, Rust (stable, via `rustup`), and the Visual Studio Build Tools with the C++ workload
and a Windows SDK.

```bash
npm ci              # install exact dependency versions
npm run dev         # run the app in development mode
npm run tauri build # build the MSI and NSIS installers
npm run check       # run every check: typecheck, lint, format, secret scan, tests
```

The first build takes several minutes because SQLite is compiled from source.

## Project documentation

| Document                                           | Contents                                                         |
| -------------------------------------------------- | ---------------------------------------------------------------- |
| [Word user guide](docs/Threadwell-User-Guide.docx) | Step-by-step instructions for every feature                      |
| [samples/](samples/README.md)                      | Sample files and a first-session walkthrough                     |
| [docs/videos/](docs/videos/README.md)              | Video guides for the main screens (MP4)                          |
| [docs/STATUS.md](docs/STATUS.md)                   | Milestone checklist, measurements and verification               |
| [docs/architecture.md](docs/architecture.md)       | How the app is built: data model, transactions, assistant design |
| [docs/acceptance.md](docs/acceptance.md)           | Each release gate and the test that enforces it                  |
| [eval/README.md](eval/README.md)                   | Assistant evaluation: method, results and limits                 |
| [intent.md](intent.md)                             | The product brief this project is built against                  |

## Project status and limits

Threadwell implements all six milestones in its brief. Version 0.1.5 adds line citations for code, drag and drop, quick questions, and a sample project. Version 0.1.4 added several workspaces, scoped assistant
retrieval across them, and an optional several-agent mode. Details are in [docs/STATUS.md](docs/STATUS.md). The main limits:

- **Assistant quality:** on the latest held-out split, four of eight gated measures fail (phrasing, task recall, one leaked injected word, and injection-triggered proposals). Treat its output as a draft. See [eval/README.md](eval/README.md).
- **Platform:** Windows 11 only, verified on one machine.
- **Installers:** unsigned, and not yet tested on a clean machine.

- **OneNote (`.one`) files:** not supported. Export them from OneNote as Word or Markdown first. Scanned PDFs have no text and cannot be imported.
- **Audio transcription:** needs a local engine (for example whisper.cpp) and a model that you install yourself. It has not been tested end to end on this machine.
- **Scheduled recipes:** run only while the app is open.
- **Other workspaces in the assistant:** searched by keyword only, read-only, and only when you select them.
- **Memory:** about 336 MiB across the app and its WebView2 processes when idle, above the 250 MiB target.
- **Accessibility:** reviewed by inspection, not tested with assistive technology.

## License

No license has been chosen yet. Until one is added, all rights are reserved by the author. Contact the author before
reusing the code.
