from docx import Document
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.shared import Pt, RGBColor, Cm

OUT = r"C:\MyProjects\threadwell\docs\Threadwell-User-Guide.docx"

doc = Document()
for section in doc.sections:
    section.left_margin = section.right_margin = Cm(2.2)
    section.top_margin = section.bottom_margin = Cm(2)

normal = doc.styles["Normal"]
normal.font.name = "Calibri"
normal.font.size = Pt(11)
for name, size in (("Heading 1", 16), ("Heading 2", 13), ("Heading 3", 11.5)):
    style = doc.styles[name]
    style.font.name = "Calibri"
    style.font.size = Pt(size)
    style.font.color.rgb = RGBColor(0x2E, 0x3A, 0x6B)


def para(text, bold=False, italic=False):
    p = doc.add_paragraph()
    run = p.add_run(text)
    run.bold = bold
    run.italic = italic
    return p


def note(text):
    p = doc.add_paragraph()
    run = p.add_run("Note: " + text)
    run.italic = True
    run.font.color.rgb = RGBColor(0x55, 0x55, 0x55)


def steps(items):
    for item in items:
        doc.add_paragraph(item, style="List Number")


def bullets(items):
    for item in items:
        doc.add_paragraph(item, style="List Bullet")


def table(headers, rows, widths=None):
    t = doc.add_table(rows=1, cols=len(headers))
    t.style = "Light Grid Accent 1"
    t.alignment = WD_TABLE_ALIGNMENT.LEFT
    for i, h in enumerate(headers):
        cell = t.rows[0].cells[i]
        cell.text = h
        for r in cell.paragraphs[0].runs:
            r.bold = True
    for row in rows:
        cells = t.add_row().cells
        for i, value in enumerate(row):
            cells[i].text = value
    if widths:
        for row in t.rows:
            for i, w in enumerate(widths):
                row.cells[i].width = Cm(w)
    doc.add_paragraph()


# ---------------------------------------------------------------- title
title = doc.add_paragraph()
r = title.add_run("Threadwell")
r.bold = True
r.font.size = Pt(28)
r.font.color.rgb = RGBColor(0x2E, 0x3A, 0x6B)
sub = doc.add_paragraph()
r = sub.add_run("Step-by-step user guide")
r.font.size = Pt(15)
para("Version 0.1.5 for Windows 11. Covers notes, tasks, search, import and export, backup, the local assistant, "
     "meetings, recipes, several workspaces, and linked project folders.")

doc.add_heading("Before you start", level=1)
para("Threadwell keeps everything in a folder you choose on your computer. There is no account and no sign-in. "
     "The notes, tasks and search work with no internet connection.")
para("Read these three points first:", bold=True)
bullets([
    "The AI assistant is experimental. Its answers can be wrong or incomplete. Check the sources it cites before you act on them.",
    "Nothing the assistant suggests is saved until you click Apply. You can undo an applied change, but only while the page has not changed since.",
    "Back up your workspace regularly (section 7). Threadwell does not sync or keep copies anywhere else.",
])

# ---------------------------------------------------------------- 1
doc.add_heading("1. Install Threadwell", level=1)
para("You need Windows 11. The installers are not code-signed, so Windows will show a warning the first time.")
steps([
    "Open the installer you were given: Threadwell_0.1.5_x64_en-US.msi (Windows Installer) or Threadwell_0.1.5_x64-setup.exe.",
    "If Windows SmartScreen says it protected your PC, click More info, then Run anyway. Only do this for the file you downloaded from the place you trust.",
    "Follow the installer. Threadwell needs Microsoft Edge WebView2 Runtime, which Windows 11 includes.",
    "Start Threadwell from the Start menu.",
])

# ---------------------------------------------------------------- 2
doc.add_heading("2. Create your first workspace", level=1)
para("A workspace is one folder holding your data. The first time you open Threadwell, it asks you to create or open one.")
steps([
    "Under Start, keep Create a workspace selected.",
    "In Workspace name, type a name, for example My notes.",
    "Click Choose folder…, then choose an empty folder such as C:\\Users\\<you>\\Documents\\Threadwell. The folder must not already contain a Threadwell workspace.",
    "Leave Include the labelled sample project ticked if you want example pages and tasks. Untick it for an empty workspace.",
    "Click Create workspace. The Home screen opens.",
])
note("To open a workspace you created before, choose Open an existing workspace, then choose its folder. "
     "Threadwell reopens your last workspace automatically when you start it.")

# ---------------------------------------------------------------- 3
doc.add_heading("3. Learn the screen", level=1)
para("The window has three areas:")
table(["Area", "What it contains"], [
    ["Left sidebar", "Your workspace name, a Search box, the links Home, Tasks, Meetings, Recipes, Trash and Settings, then the Pages tree, Favorites and Projects."],
    ["Centre", "The page you are editing, the task list, search results, or the screen you chose."],
    ["Right panel", "The assistant. Use Hide assistant or Show assistant at the top of the centre area to collapse it."],
], widths=[3.5, 12.5])
para("Ctrl+K opens the command palette. Type part of a page title or a command name, press the arrow keys, then Enter. "
     "It is the quickest way to move around.")

# ---------------------------------------------------------------- 4
doc.add_heading("4. Write and organise pages", level=1)
doc.add_heading("Create a page", level=2)
steps([
    "In the sidebar, click + next to Pages to create a top-level page. The page is called Untitled and opens for editing.",
    "Click the title at the top and type a name. The title saves automatically.",
    "Click in the body and start typing. Press Enter for a new paragraph.",
])
doc.add_heading("Format text", level=2)
para("Use the formatting toolbar above the page, or type / on an empty line to open block commands:")
table(["Toolbar label", "Result"], [
    ["B, I, <>", "Bold, italic, inline code"],
    ["H1, H2", "Large and medium headings"],
    ["•, 1., ☐, ❝, { }", "Bulleted list, numbered list, checklist, quote, code block"],
    ["↶, ↷", "Undo and redo"],
], widths=[4, 12])
para("Type / then a word such as table or divider to insert blocks quickly.")
doc.add_heading("Nest, favourite and link pages", level=2)
steps([
    "To make a sub-page, click the + next to the parent page in the sidebar.",
    "To move a page under another, open it and change Parent in the page header. A page cannot be moved under itself.",
    "To favourite a page, click Favorite in the page header. Favourites appear in the sidebar.",
    "To link to another page, choose it from the Link to page… menu in the toolbar. The link opens that page when clicked. Links to web addresses are shown but do not open a browser.",
])
doc.add_heading("Autosave", level=2)
para("Threadwell saves about 0.8 seconds after you stop typing. The status line under the toolbar shows Unsaved changes, "
     "Saving… or Saved. If it says Couldn't save, Threadwell retries every few seconds. Keep the app open until it says Saved.")
doc.add_heading("Attach a file to a page", level=2)
para("Attachments are copies stored inside your workspace. Your original file is not moved or changed.")
steps([
    "Open the page and scroll to the Attachments section below the editor.",
    "Click Attach a file… and choose the file. Files up to 25 MB are accepted.",
    "To see where the stored copy is, click Show in folder. To delete the copy, click Remove. Your original file is not touched.",
])
note("Programs and scripts (.exe, .bat, .ps1, .js, .msi and similar) cannot be attached. Attach a document or export the data instead.")
doc.add_heading("Move a page to the trash", level=2)
steps([
    "Open the page and click Move to trash in the page header.",
    "To get it back, open Trash in the sidebar and click Restore. Trashed pages are hidden from the tree and from search until you restore them.",
])
note("Threadwell has no permanent-delete button in this version.")

# ---------------------------------------------------------------- 5
doc.add_heading("5. Work with tasks", level=1)
steps([
    "Click Tasks in the sidebar. To add a task, open Add a task, type a title, choose a priority, and optionally a due date and project. Click Add task.",
    "To make a project, type its name under New project and click Create project. Projects appear in the sidebar.",
    "Use Table to edit tasks in place: change the title, status, priority, due date or project. Click the × next to a due date to clear it.",
    "Use Board to see three columns: To do, In progress and Done. Drag a card to another column, or use ← and → on the card.",
    "To delete a task, click Delete in its row (table view).",
])
para("Due dates are never filled in automatically. A task has no due date until you set one.")

# ---------------------------------------------------------------- 6
doc.add_heading("6. Search, import and export", level=1)
doc.add_heading("Search", level=2)
steps([
    "Type in the Search box at the top of the sidebar and press Enter, or click Search in the palette.",
    "Results list pages and tasks. Matching words are highlighted. Click a result to open it.",
])
doc.add_heading("Import a single file", level=2)
para("Import copies one Markdown or plain-text file into your workspace. The original file is not changed. "
     "A first line starting with a single # becomes the page title.")
steps([
    "Open Settings in the sidebar, then find Import and export.",
    "Click Import a Markdown file and choose a .md, .markdown or .txt file. The file must be UTF-8 text under 5 MB.",
    "The new page opens. Try it with the sample file samples\\import-project-plan.md from the Threadwell folder.",
])
doc.add_heading("Import a folder of notes", level=2)
para("Use this to bring in a whole folder at once. It reads Markdown (.md), text (.txt), Word (.docx) and text-based PDF "
     "(.pdf) files, one note per file. Your files are never changed, moved or renamed.")
steps([
    "Open Settings and find Import a folder of notes.",
    "Click Choose folder… and select the folder. Threadwell scans it, including subfolders. Hidden files and links are skipped.",
    "Optional: click Watch this folder. Later, Sync watched folders now imports any new notes in it. Changed files are reported, not imported automatically, so no duplicate pages appear.",
    "Read the status column. New means not imported yet. Already imported (unchanged) files are skipped. Changed since import files are imported again as a new page, and the earlier page is kept.",
    "Tick the files you want, or click Select all importable.",
    "Click Import, then read the summary. It lists imported and skipped counts, and any file that failed with the reason.",
])
note("Word files keep their headings, lists and tables, but not fonts or images. PDFs keep their text but not their layout. "
     "Scanned PDFs have no text and cannot be imported. CSV files become tables. OneNote (.one) files are not supported; "
     "export them from OneNote as Word or Markdown first.")
note("Transcripts (.vtt and .srt) are not part of folder import. Import them through Meetings instead (section 11).")
doc.add_heading("Export", level=2)
bullets([
    "Export all pages to Markdown: choose an empty folder. Each page becomes one .md file. Existing files are never overwritten; a copy gets (2) in its name.",
    "Export tasks to CSV: choose where to save the file. Opens in Excel. Values that start with =, +, - or @ are shown as text, which blocks spreadsheet formulas.",
])

# ---------------------------------------------------------------- 7
doc.add_heading("7. Back up and restore", level=1)
para("Do this regularly, and before any restore or big import.")
doc.add_heading("Create a backup", level=2)
steps([
    "Open Settings, then go to Backup and restore.",
    "Click Create backup and choose a folder outside your workspace folder. Threadwell creates a new folder named threadwell-backup-<date and time>.",
    "Copy that folder somewhere safe, such as an external drive.",
])
doc.add_heading("Restore a backup", level=2)
steps([
    "Open Settings, then click Restore from backup….",
    "Choose the backup folder, then choose an empty folder for the restored workspace. A folder that already has files is refused.",
    "Confirm. The restored workspace opens. Your current workspace is not changed.",
])
note("A backup contains your notes in plain form. Store it somewhere you trust.")

# ---------------------------------------------------------------- 8
doc.add_heading("8. Settings and appearance", level=1)
bullets([
    "Appearance → Theme: Match system, Light or Dark.",
    "Finding information → Build or update index: rebuilds the search index for the assistant. Use it if results look out of date.",
    "AI assistance → Model server address, Model name, and an option to allow a server on another machine. Leave the last option off unless you know you need it.",
    "Audio transcription (local engine) → the engine program and the model file you installed yourself. See section 11.",
])

# ---------------------------------------------------------------- 9
doc.add_heading("9. Set up the assistant (optional)", level=1)
para("The assistant runs on a model that lives on your computer, through a free program called Ollama. "
     "Threadwell does not install Ollama or the models for you. Everything else in Threadwell works without it.")
steps([
    "Download and install Ollama from ollama.com. It starts a small server on your computer.",
    "Open a Command Prompt or PowerShell window and run: ollama pull qwen2.5:3b . This downloads the chat model, about 1.9 GB.",
    "Run: ollama pull nomic-embed-text . This downloads the embedding model used for semantic search, about 270 MB.",
    "In Threadwell, open Settings and go to AI assistance. Enter qwen2.5:3b as the Model name.",
    "Click Save and check. The status should say Connected.",
    "Go to Finding information, check that Embedding model is nomic-embed-text, then click Build or update index. Wait until the count shows all passages indexed.",
])
note("Keep the default address http://127.0.0.1:11434 unless you changed Ollama's settings. Only a computer on your own network needs a different address, and then Threadwell needs https.")

doc.add_heading("Status messages", level=2)
table(["Status in the assistant panel", "What to do"], [
    ["Connected", "Ready to use."],
    ["Not set up", "Enter a model name in Settings → AI assistance and click Save and check."],
    ["Model server not reachable", "Start Ollama, then try again. Your notes are not affected."],
    ["Model not installed", "Run ollama pull <model name> in a command window."],
], widths=[6, 10])

# ---------------------------------------------------------------- 10
doc.add_heading("10. Ask the assistant", level=1)
steps([
    "Open the assistant on the right. Choose Start a new conversation in the Conversation list, or select an earlier one.",
    "If a page is open, the box Include “<page>” as context is ticked. Untick it to ask about the whole workspace only.",
    "Type your question and press Enter. Shift+Enter starts a new line. Click Stop to cancel an answer that is still being written.",
    "Read the answer. Numbers such as [1] refer to the Sources list under the answer. Click a source to open that page.",
    "If the assistant says the workspace does not contain the answer, believe it. It is designed to say so rather than guess.",
])
para("Good questions for the sample workspace:", bold=True)
bullets([
    "What did we decide about the launch date?",
    "What is the URL export deadline?",
    "Who signs off on the final copy?",
])
doc.add_heading("Review a suggested change", level=2)
para("When the assistant proposes a change, a card appears with a summary and a preview. Lines starting with + would be added, "
     "and lines starting with - would be removed.")
steps([
    "Read the preview carefully. Nothing has changed yet.",
    "Click Apply to make the change, or Reject to discard it.",
    "After Apply, the card shows an Undo button. Click it to restore the previous content. Undo is refused if the page changed after you applied the suggestion. In that case, fix the page by hand.",
    "If a card says Out of date, the page changed after the suggestion was made. Ask the assistant again.",
])
doc.add_heading("Run an action on selected text", level=2)
steps([
    "Select a passage in a page.",
    "Expand Assistant on selected text under the toolbar.",
    "Choose Rewrite, Summarize, Expand or Translate. For Translate, type the language.",
    "Click Run on selection and wait. Click Stop to cancel.",
    "Compare Original and Suggested. Check any numbers warned about in red. Click Accept to replace the selection, or Reject to keep the original.",
])
doc.add_heading("Use selected text as context", level=2)
para("Select a passage in a page. A box appears in the assistant panel: Include the selected text as context. Tick it to include the passage with your question. Untick it to ask without it.")
doc.add_heading("Find a past conversation", level=2)
para("Type in Search past conversations at the top of the assistant panel. Matching conversations appear with the matching text. Click one to open it.")
doc.add_heading("Keep a page out of the assistant", level=2)
para("Tick Exclude from AI in the page header. The assistant will not search, read or suggest changes to that page. "
     "Pages you exclude are still searchable by you in Search.")
doc.add_heading("See what the assistant did", level=2)
para("Settings → Assistant history lists recent runs with their status, time and token counts. "
     "Click a time to see each tool call and its outcome, and Delete this run's trace to remove that trace. "
     "Suggestions and pages are kept. Model reasoning is not stored.")

# ---------------------------------------------------------------- 11
doc.add_heading("11. Work with meeting transcripts", level=1)
steps([
    "Click Meetings in the sidebar. Open Import a transcript.",
    "Option A: type a title and paste a transcript, then click Import text. Each line should look like [00:01:02] Ana: What we agreed.",
    "Option B: click Import file… and choose a .txt, .vtt or .srt file. Try samples\\meeting-kickoff.txt or samples\\meeting-kickoff.vtt.",
    "The meeting opens with its transcript. Click Extract notes and actions. Processing takes a minute or more on a laptop.",
    "Read Extracted. Every summary line, decision, question and action shows evidence timestamps. Click into the transcript to check them.",
    "Review the action items card. Click Apply to create the tasks, or Reject. A due date appears only if the transcript states that exact date.",
])
doc.add_heading("Audio recordings (optional)", level=2)
para("Threadwell can turn a recording into a transcript with a local engine you install yourself, such as whisper.cpp "
     "(its whisper-cli program) and a model file you download for it. Threadwell does not download either one.", italic=False)
steps([
    "Install the engine and download a model file, following the engine's own instructions.",
    "In Settings → Audio transcription, choose the engine program and the model file, then click Save.",
    "In Meetings, click Import audio… and choose a WAV, MP3, M4A, MP4, WebM, FLAC or OGG recording.",
    "The transcript is imported as a meeting. A transcription can take a long time; it stops after 30 minutes.",
])
note("Without an engine, Import audio… explains what is needed. You can always import a text transcript instead. "
     "Recording-to-text quality depends on the engine and model you choose.")

# ---------------------------------------------------------------- 12
doc.add_heading("Agent design and local performance traces (optional)", level=2)
para("Settings → Finding information → Agent design chooses how the assistant works. One agent is the default and is the "
     "one we recommend. Several agents splits each question into planning, research, writing and an optional action step. "
     "On our fourth test set it was slower and gave worse answers on most measures, though it refused injected instructions better. "
     "Keep one agent unless you want to try the other on your own notes.")
para("Settings → Performance traces (local only) can record timing for assistant runs in a file on your computer, in "
     "OpenTelemetry format. Traces never include prompts, answers, page text or search results. Nothing is sent anywhere. "
     "The setting takes effect the next time you start Threadwell. Delete traces removes the file.")

doc.add_heading("12. Use recipes (recurring drafts)", level=1)
para("A recipe is a set of instructions that drafts a page for you. Each run creates a draft suggestion that you review. "
     "Nothing is written to your pages automatically.")
steps([
    "Click Recipes in the sidebar, then New recipe.",
    "Enter a name and instructions, for example: Draft a weekly update from completed tasks. Group them by project.",
    "Choose a schedule: Manual only, Daily, or Weekly. For a schedule, pick the time and, for weekly, the day. The timezone is filled in from your computer.",
    "Click Save recipe. The list shows when it will next run.",
    "To test it now, click Run now. When the run finishes, click Review draft in its history row, then Apply or Reject.",
])
note("Recipes run only while Threadwell is open. If your computer was off at the scheduled time, the recipe runs once when you next open Threadwell, not once for each missed time. "
     "Pause stops a recipe without deleting it. Delete removes the recipe and its run history, but drafts already created are kept.")

# ---------------------------------------------------------------- 13
doc.add_heading("13. Work with several workspaces", level=1)
para("Use one workspace per area of your life or work. Each workspace has its own pages, tasks, meetings, recipes and "
     "assistant history, and pages never move between them. The assistant can also answer from other workspaces.")
steps([
    "Open Settings and find the Workspaces section. Your known workspaces are listed, and the open one is marked.",
    "In the sidebar, the Workspaces section lists the pages of each workspace. Pages of other workspaces are read only. Clicking one switches to that workspace and opens the page.",
    "To add one, click Create or open another workspace. Create a new workspace in an empty folder, or open an existing one.",
    "To change workspace, click Switch on the one you want. Anything it was doing in the previous workspace is stopped first.",
    "To rename the open workspace, type a name under Rename the open workspace and click Rename.",
    "To take a workspace off the list, click Remove from list. Its folder and pages are kept, and you can open it again later.",
])
doc.add_heading("Ask the assistant about other workspaces", level=2)
para("Above the message box, the Answer from list has three kinds of choice. This workspace only is the default. "
     "All workspaces searches every workspace. A workspace name searches only that one. Other workspaces are read-only "
     "for the assistant. It never changes them, and proposed changes always go to the open workspace only. "
     "You can also name a workspace in your question, for example: What is in my Home workspace?")

doc.add_heading("14. Link a folder and ask about it", level=1)
para("Link a project folder, such as software source code, or a folder of documents. Threadwell reads the files, keeps them in "
     "step while the app is open, and the assistant answers questions about them. Each answer names the file it came from. "
     "Your folder is never changed.")
steps([
    "Click Sources in the sidebar, then Link a folder. Choose the folder.",
    "Threadwell reads the files. Each one appears as a read-only page titled with its path, such as src/charge.rs.",
    "Source code, configuration, Markdown, text, Word, text-based PDF and CSV files are read. Build output, dependencies, hidden files, lock files and anything in the folder's .gitignore are skipped.",
    "Click Show files to open a linked file. It is read only.",
    "Ask the assistant a question about the project. For code, the citation names the file and the lines, such as src/charge.rs · Lines 41–80. Click it to see that part of the file.",
    "Drop a folder on the Threadwell window to link it, or drop a Markdown or text file to import it as a note.",
    "On a linked file, use Summarise this file, List open items or Explain how it works. The question goes into the assistant box, ready to send.",
    "New workspace: tick the sample option and the sample project is written to a sample-project folder inside the workspace and linked.",
    "Click Sync now to check for changes at once. Threadwell also checks about once a minute while it is open.",
    "Click Unlink to remove the link. The files move to Trash. The folder is not changed.",
])
note("Limits: 5,000 files per folder. For code, answers name the file and the line section, for example Lines 41–80. Checks happen only while Threadwell is open. Try the sample project in samples\\sample-repo.")

doc.add_heading("15. Troubleshooting", level=1)
table(["Problem", "Cause and fix"], [
    ["SmartScreen blocks the installer", "The installer is unsigned. Click More info, then Run anyway, for the file you trust."],
    ["Threadwell says it cannot find a workspace", "The folder was moved or renamed. Open the workspace again from its new location."],
    ["Create workspace refuses the folder", "The folder already contains a Threadwell workspace. Choose Open an existing workspace, or choose a different folder."],
    ["Restore refuses the destination", "The restore folder must be empty. Choose a new or empty folder."],
    ["Backup or restore says the database does not match", "The backup was changed or damaged. Use an earlier backup."],
    ["Couldn't save. Retrying.", "The disk may be full or read-only. Free space, then wait; the app keeps your edits and retries."],
    ["Assistant gives an unhelpful answer", "Small local models make mistakes. Rephrase the question, open the cited source, and do not rely on the answer alone."],
    ["Assistant says Semantic search was unavailable", "The embedding model is not running or not installed. Keyword search still works. Run ollama pull nomic-embed-text."],
    ["Suggestion says Out of date", "The page changed after the suggestion. Ask again."],
    ["Import fails: file is larger than 5 MB", "Split the file into smaller ones."],
    ["Folder import says Not supported", "The file type is .one (OneNote), .doc or .enex. Export it as Markdown, text, Word, PDF or CSV first."],
    ["Folder import says the PDF has no text layer", "The PDF is a scan. Use an OCR tool to make a text PDF, then import it again."],
    ["Folder import shows Changed since import", "The file changed after its last import. Importing it again creates a new page; the earlier page stays."],
    ["Import audio… explains an engine is needed", "Choose an engine program and a model file in Settings → Audio transcription, or import a text transcript instead."],
    ["Attach a file says it cannot be attached", "The file is a program or script, or it is larger than 25 MB."],
    ["The disk is full message", "Free some disk space. Threadwell keeps your edits open; save again after freeing space."],
], widths=[6, 10])

# ---------------------------------------------------------------- 14
doc.add_heading("16. Where your data lives", level=1)
bullets([
    "Your workspace folder contains threadwell.db (all notes, tasks, meetings and settings), plus the files that go with the database: threadwell.db-wal and threadwell.db-shm while the app is open.",
    "Do not edit those files by hand, and do not copy the folder while Threadwell is open. Use Create backup instead.",
    "Threadwell keeps only one small file outside your workspace: it remembers which workspace to reopen.",
])

doc.add_heading("17. Delete a workspace", level=1)
para("Threadwell has no delete button for workspaces. A workspace is a folder, so you delete the folder. This is permanent.")
steps([
    "Close Threadwell completely, including from the system tray.",
    "Back up first if you might need the data later: open the workspace, then Settings → Create backup.",
    "In File Explorer, delete the workspace folder. It holds threadwell.db, the -wal and -shm files beside it, and an attachments folder. Deleting the folder removes everything.",
    "Optional: in Settings → Workspaces, click Remove from list. This clears the entry from %APPDATA%\\dev.threadwell.app\\workspaces.json. If the folder is gone, the app shows the first-run screen anyway.",
])
note("Uninstalling Threadwell does not delete workspaces, because they live outside the program folder.")

doc.add_heading("Quick checklist for a new user", level=1)
bullets([
    "Created a workspace in an empty folder.",
    "Made a backup folder somewhere else.",
    "Tried the sample import and a sample meeting.",
    "Read at least one assistant answer's sources before trusting it.",
    "Know how to Undo a change and how to Restore from backup.",
])

doc.save(OUT)
print("saved", OUT)
