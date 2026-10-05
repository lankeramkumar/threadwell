//! Linked source folders. A source is a folder the user chose. Its readable files are mirrored as
//! read-only pages, so search, the assistant and citations treat them like notes. Source code is
//! kept as code blocks, so a question about a function can cite the file it is in.
//!
//! Syncing reads the folder without holding the workspace lock. It takes the lock only to compare
//! and to apply changes in small batches, so the window stays responsive during large syncs. A
//! file is re-read only when its modified time or size changed. The watcher runs a sync for each
//! source every minute while the app is open. Nothing is ever written to the source folder.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::commands::{with_active, AppState};
use crate::error::{validation, AppError, AppResult};
use crate::workspace::Active;
use crate::{documents, markdown, pages, util};

/// Source and configuration files, kept as code blocks labelled with their extension.
const CODE_EXTS: &[&str] = &[
    "rs", "py", "js", "jsx", "ts", "tsx", "go", "java", "kt", "c", "h", "cc", "cpp", "hpp", "cs", "rb", "php", "swift",
    "sh", "ps1", "sql", "html", "css", "scss", "json", "yaml", "yml", "toml", "xml", "ini", "cfg", "gradle",
];
/// Directories that are build output, dependencies or tooling. They are never read.
const IGNORED_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", "out", "bin", "obj", "__pycache__", "venv", "vendor", "coverage",
    "site-packages",
];
const IGNORED_FILE_SUFFIXES: &[&str] = &[".lock", ".min.js", ".min.css", ".map", ".pyc", ".class", ".dll", ".exe"];
const IGNORED_FILE_NAMES: &[&str] = &["package-lock.json", "pnpm-lock.yaml", "yarn.lock"];
pub const MAX_FILES: usize = 5_000;
const MAX_DEPTH: usize = 12;
const MAX_DOCUMENT_BYTES: u64 = 5 * 1024 * 1024;
const MAX_CODE_BYTES: u64 = 1024 * 1024;
const BATCH: usize = 40;
const WATCH_EVERY: Duration = Duration::from_secs(60);
const TITLE_CHARS: usize = 200;

/// Sources that are syncing now, so two syncs never change the same source at once.
static SYNCING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub added_at: String,
    pub last_synced_at: Option<String>,
    pub last_summary: Option<String>,
    pub file_count: i64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Files that were not read: too large, binary, or in an ignored folder.
    pub skipped: usize,
    /// Files that could not be read or converted, with the reason for the first few.
    pub failed: usize,
    pub failures: Vec<String>,
    pub truncated: bool,
    /// True when the workspace was switched during the sync, so the rest was not applied.
    pub stopped: bool,
}

/// A readable file found in the folder, with the fingerprint used to detect changes.
#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    relative: String,
    path: PathBuf,
    size: u64,
    fingerprint: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Add(Candidate),
    Update { page_id: String, revision: i64, restore: bool, cand: Candidate },
    Remove { page_id: String },
}

struct Existing {
    page_id: String,
    revision: i64,
    fingerprint: String,
    deleted: bool,
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Ignore rules: the built-in list, plus the simple patterns in the folder's `.gitignore`.
/// Negated patterns (`!`) are not supported and are skipped.
struct Rules {
    gitignore: Vec<String>,
}

impl Rules {
    fn load(root: &Path) -> Self {
        let gitignore = fs::read_to_string(root.join(".gitignore"))
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
            .map(|l| l.trim_start_matches('/').trim_end_matches('/').to_string())
            .collect();
        Self { gitignore }
    }

    /// True when a directory or file name should not be read.
    fn ignores_name(&self, name: &str, is_dir: bool) -> bool {
        if name.starts_with('.') {
            return true;
        }
        if is_dir {
            if IGNORED_DIRS.contains(&name) {
                return true;
            }
        } else if IGNORED_FILE_NAMES.contains(&name) || IGNORED_FILE_SUFFIXES.iter().any(|s| name.ends_with(s)) {
            return true;
        }
        self.gitignore.iter().any(|pattern| match pattern.strip_prefix("*.") {
            Some(suffix) => !is_dir && name.ends_with(&format!(".{suffix}")),
            None => name == pattern,
        })
    }
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default()
}

fn fingerprint(meta: &fs::Metadata) -> String {
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{modified}:{}", meta.len())
}

#[derive(Default)]
struct Scan {
    found: Vec<Candidate>,
    skipped: usize,
    truncated: bool,
}

fn walk(root: &Path, dir: &Path, depth: usize, rules: &Rules, scan: &mut Scan) -> AppResult<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if !rules.ignores_name(&name, true) {
                walk(root, &path, depth + 1, rules, scan)?;
            }
            continue;
        }
        if !meta.is_file() || rules.ignores_name(&name, false) {
            continue;
        }
        let ext = extension(&path);
        let limit = if matches!(ext.as_str(), "md" | "markdown" | "txt") || CODE_EXTS.contains(&ext.as_str()) {
            MAX_CODE_BYTES
        } else if matches!(ext.as_str(), "docx" | "pdf" | "csv") {
            MAX_DOCUMENT_BYTES
        } else {
            continue;
        };
        if meta.len() > limit {
            scan.skipped += 1;
            continue;
        }
        if scan.found.len() >= MAX_FILES {
            scan.truncated = true;
            return Ok(());
        }
        scan.found.push(Candidate {
            relative: relative_string(root, &path),
            path: path.clone(),
            size: meta.len(),
            fingerprint: fingerprint(&meta),
        });
    }
    Ok(())
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn scan_folder(root: &Path) -> AppResult<Scan> {
    let rules = Rules::load(root);
    let mut scan = Scan::default();
    walk(root, root, 0, &rules, &mut scan)?;
    Ok(scan)
}

// ---------------------------------------------------------------------------
// Reading files into page content
// ---------------------------------------------------------------------------

/// Title and content for one file. Runs outside the workspace lock.
fn read_content(cand: &Candidate) -> AppResult<(String, Value)> {
    let ext = extension(&cand.path);
    let bytes = fs::read(&cand.path)?;
    let title: String = cand.relative.chars().take(TITLE_CHARS).collect();
    let is_binary = |b: &[u8]| b[..b.len().min(8000)].contains(&0);
    let body = match ext.as_str() {
        "docx" => markdown::from_markdown(&documents::docx_to_markdown(&bytes).map_err(AppError::Validation)?),
        "pdf" => markdown::from_markdown(&documents::pdf_to_text(&bytes).map_err(AppError::Validation)?),
        "csv" => {
            let text = utf8(&bytes)?;
            markdown::from_markdown(&documents::csv_to_markdown(&text).map_err(AppError::Validation)?)
        }
        "md" | "markdown" | "txt" => {
            if is_binary(&bytes) {
                return validation("The file looks binary");
            }
            markdown::from_markdown(&utf8(&bytes)?)
        }
        _ => {
            if is_binary(&bytes) {
                return validation("The file looks binary");
            }
            code_document(&utf8(&bytes)?, &ext)
        }
    };
    Ok((title, body))
}

fn utf8(bytes: &[u8]) -> AppResult<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| AppError::Validation("The file is not UTF-8 text".into()))
}

/// A whole file as one code block, labelled with its extension.
fn code_document(text: &str, language: &str) -> Value {
    let mut block = json!({ "type": "codeBlock", "attrs": { "language": language } });
    if !text.is_empty() {
        block["content"] = json!([{ "type": "text", "text": text }]);
    }
    json!({ "type": "doc", "content": [block] })
}

// ---------------------------------------------------------------------------
// Planning and applying
// ---------------------------------------------------------------------------

fn existing_pages(conn: &Connection, source_id: &str) -> AppResult<HashMap<String, Existing>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_path, revision, source_hash, deleted_at IS NOT NULL FROM pages WHERE source_id = ?1",
    )?;
    let rows = stmt.query_map(params![source_id], |row| {
        Ok((
            row.get::<_, String>(1)?,
            Existing {
                page_id: row.get(0)?,
                revision: row.get(2)?,
                fingerprint: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                deleted: row.get::<_, i64>(4)? == 1,
            },
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (path, existing) = row?;
        map.insert(path, existing);
    }
    Ok(map)
}

/// Decides what to change. Pure with respect to the folder: it only compares fingerprints.
fn decide(cands: Vec<Candidate>, mut existing: HashMap<String, Existing>) -> Vec<Op> {
    let mut ops = Vec::new();
    for cand in cands {
        match existing.remove(&cand.relative) {
            None => ops.push(Op::Add(cand)),
            // Same fingerprint: unchanged. A file the user removed from the workspace stays removed.
            Some(e) if e.fingerprint == cand.fingerprint => {}
            Some(e) => ops.push(Op::Update { page_id: e.page_id, revision: e.revision, restore: e.deleted, cand }),
        }
    }
    // Files that are gone from the folder: remove their pages (kept in Trash, so this is recoverable).
    for (_, e) in existing {
        if !e.deleted {
            ops.push(Op::Remove { page_id: e.page_id });
        }
    }
    ops
}

fn apply(conn: &Connection, ws: &str, source_id: &str, op: &Op, content: Option<(String, Value)>, report: &mut SyncReport) {
    let result: AppResult<()> = (|| match op {
        Op::Add(cand) => {
            let (title, body) = content.clone().ok_or_else(|| AppError::Validation("missing content".into()))?;
            let page = pages::create(conn, ws, &title, None)?;
            pages::save(conn, ws, &page.id, &title, &body, page.revision)?;
            conn.execute(
                "UPDATE pages SET source_id = ?1, source_path = ?2, source_hash = ?3 WHERE id = ?4",
                params![source_id, cand.relative, cand.fingerprint, page.id],
            )?;
            report.added += 1;
            Ok(())
        }
        Op::Update { page_id, revision, restore, cand } => {
            let (title, body) = content.clone().ok_or_else(|| AppError::Validation("missing content".into()))?;
            pages::save(conn, ws, page_id, &title, &body, *revision)?;
            conn.execute(
                "UPDATE pages SET source_hash = ?1, deleted_at = CASE WHEN ?2 THEN NULL ELSE deleted_at END WHERE id = ?3",
                params![cand.fingerprint, restore, page_id],
            )?;
            report.updated += 1;
            Ok(())
        }
        Op::Remove { page_id } => {
            pages::trash(conn, ws, page_id)?;
            report.removed += 1;
            Ok(())
        }
    })();
    if let Err(error) = result {
        report.failed += 1;
        if report.failures.len() < 5 {
            let relative = match op {
                Op::Add(c) | Op::Update { cand: c, .. } => c.relative.clone(),
                Op::Remove { .. } => "a removed file".to_string(),
            };
            report.failures.push(format!("{relative}: {error}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Syncing
// ---------------------------------------------------------------------------

/// Brings one source up to date with its folder. Holds the workspace lock only while comparing and
/// applying changes, never while reading files.
pub fn sync_source(active: &Arc<Mutex<Option<Active>>>, source_id: &str) -> AppResult<SyncReport> {
    let claimed = SYNCING.lock().map(|mut set| set.insert(source_id.to_string())).unwrap_or(false);
    if !claimed {
        return validation("This folder is already syncing. Try again in a moment.");
    }
    let result = sync_inner(active, source_id);
    if let Ok(mut set) = SYNCING.lock() {
        set.remove(source_id);
    }
    result
}

fn sync_inner(active: &Arc<Mutex<Option<Active>>>, source_id: &str) -> AppResult<SyncReport> {
    util::validate_id(source_id)?;
    let (ws, root_text) = with_active(active, |a| {
        let root: String = a
            .conn
            .query_row(
                "SELECT root_path FROM sources WHERE id = ?1 AND workspace_id = ?2",
                params![source_id, a.info.id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(AppError::NotFound("Source".into()))?;
        Ok((a.info.id.clone(), root))
    })?;
    let root = PathBuf::from(&root_text);
    if !root.is_dir() {
        let message = "The linked folder can no longer be found. Its pages were kept; remove the source to clear them.";
        record_summary(active, source_id, message)?;
        return validation(message);
    }

    // Walk and fingerprint without the lock.
    let scan = scan_folder(&root)?;
    let mut report = SyncReport { skipped: scan.skipped, truncated: scan.truncated, ..SyncReport::default() };
    let candidates = scan.found;

    // Compare under the lock; the comparison is cheap.
    let total = candidates.len();
    let ops = with_active(active, |a| {
        if a.info.id != ws {
            return Ok(None);
        }
        let existing = existing_pages(&a.conn, source_id)?;
        Ok(Some(decide(candidates, existing)))
    })?;
    let Some(ops) = ops else {
        report.stopped = true;
        return Ok(report);
    };
    let changed = ops.iter().filter(|o| matches!(o, Op::Add(_) | Op::Update { .. })).count();
    report.unchanged = total - changed;

    // Read and convert changed files without the lock.
    let mut prepared = Vec::with_capacity(ops.len());
    for op in ops {
        let content = match &op {
            Op::Add(c) | Op::Update { cand: c, .. } => match read_content(c) {
                Ok(content) => Some(content),
                Err(error) => {
                    report.failed += 1;
                    if report.failures.len() < 5 {
                        report.failures.push(format!("{}: {error}", c.relative));
                    }
                    continue;
                }
            },
            Op::Remove { .. } => None,
        };
        prepared.push((op, content));
    }

    // Apply in small batches, so other commands can run between them.
    for batch in prepared.chunks(BATCH) {
        let stopped = with_active(active, |a| {
            if a.info.id != ws {
                return Ok(true);
            }
            for (op, content) in batch {
                apply(&a.conn, &ws, source_id, op, content.clone(), &mut report);
            }
            Ok(false)
        })?;
        if stopped {
            report.stopped = true;
            return Ok(report);
        }
    }

    let summary = format!(
        "{} added, {} updated, {} removed, {} unchanged, {} skipped, {} failed",
        report.added, report.updated, report.removed, report.unchanged, report.skipped, report.failed
    );
    with_active(active, |a| {
        a.conn.execute(
            "UPDATE sources SET last_synced_at = ?1, last_summary = ?2 WHERE id = ?3",
            params![util::now(), summary, source_id],
        )?;
        Ok(())
    })?;
    Ok(report)
}

fn record_summary(active: &Arc<Mutex<Option<Active>>>, source_id: &str, message: &str) -> AppResult<()> {
    with_active(active, |a| {
        a.conn.execute(
            "UPDATE sources SET last_synced_at = ?1, last_summary = ?2 WHERE id = ?3",
            params![util::now(), message, source_id],
        )?;
        Ok(())
    })
}

/// Syncs every source in the open workspace once a minute while the app is open. Nothing runs when
/// the app is closed, and the watcher never writes to the source folders.
pub fn start_watcher(active: Arc<Mutex<Option<Active>>>) {
    let _ = std::thread::Builder::new().name("threadwell-sources".into()).spawn(move || loop {
        std::thread::sleep(WATCH_EVERY);
        let ids: Vec<String> = with_active(&active, |a| {
            let mut stmt = a.conn.prepare("SELECT id FROM sources WHERE workspace_id = ?1")?;
            let rows = stmt.query_map(params![a.info.id], |r| r.get(0))?;
            Ok(rows.collect::<Result<_, _>>()?)
        })
        .unwrap_or_default();
        for id in ids {
            let _ = sync_source(&active, &id);
        }
    });
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn info_rows(conn: &Connection, ws: &str) -> AppResult<Vec<SourceInfo>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.root_path, s.added_at, s.last_synced_at, s.last_summary,
                (SELECT COUNT(*) FROM pages p WHERE p.source_id = s.id AND p.deleted_at IS NULL)
         FROM sources s WHERE s.workspace_id = ?1 ORDER BY s.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![ws], |row| {
        Ok(SourceInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            root_path: row.get(2)?,
            added_at: row.get(3)?,
            last_synced_at: row.get(4)?,
            last_summary: row.get(5)?,
            file_count: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub async fn sources_list(state: State<'_, AppState>) -> AppResult<Vec<SourceInfo>> {
    with_active(&state.active, |a| info_rows(&a.conn, &a.info.id))
}

/// Links a folder and syncs it once. The folder is only read.
#[tauri::command]
pub async fn sources_add(state: State<'_, AppState>, path: String) -> AppResult<SourceInfo> {
    let root = util::validate_abs_path(&path)?;
    if !root.is_dir() {
        return validation("Choose a folder that exists");
    }
    // Windows canonical paths carry a \\?\ prefix. It is removed so the path reads normally.
    let canonical = fs::canonicalize(&root)?.display().to_string().trim_start_matches(r"\\?\").to_string();
    let name: String = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| canonical.clone())
        .chars()
        .take(120)
        .collect();
    let id = with_active(&state.active, |a| {
        let duplicate: i64 = a.conn.query_row(
            "SELECT COUNT(*) FROM sources WHERE workspace_id = ?1 AND root_path = ?2",
            params![a.info.id, canonical],
            |r| r.get(0),
        )?;
        if duplicate > 0 {
            return validation("This folder is already linked to this workspace");
        }
        let id = util::new_id();
        a.conn.execute(
            "INSERT INTO sources (id, workspace_id, root_path, name, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, a.info.id, canonical, name, util::now()],
        )?;
        Ok(id)
    })?;
    sync_source(&state.active, &id)?;
    with_active(&state.active, |a| {
        info_rows(&a.conn, &a.info.id)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(AppError::NotFound("Source".into()))
    })
}

#[tauri::command]
pub async fn sources_sync(state: State<'_, AppState>, id: String) -> AppResult<SyncReport> {
    sync_source(&state.active, &id)
}

/// Unlinks a folder. Its pages go to Trash and become ordinary pages there, so nothing is lost.
/// The folder itself is not touched.
#[tauri::command]
pub async fn sources_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    util::validate_id(&id)?;
    with_active(&state.active, |a| {
        let ids: Vec<String> = {
            let mut stmt = a.conn.prepare("SELECT id FROM pages WHERE source_id = ?1 AND deleted_at IS NULL")?;
            let rows = stmt.query_map(params![id], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for page_id in ids {
            pages::trash(&a.conn, &a.info.id, &page_id)?;
        }
        a.conn.execute("UPDATE pages SET source_id = NULL WHERE source_id = ?1", params![id])?;
        a.conn.execute("DELETE FROM sources WHERE id = ?1 AND workspace_id = ?2", params![id, a.info.id])?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn shared(root: &Path) -> Arc<Mutex<Option<Active>>> {
        Arc::new(Mutex::new(Some(workspace::create(root, "Repo test", false).unwrap())))
    }

    fn add_source(active: &Arc<Mutex<Option<Active>>>, folder: &Path) -> String {
        with_active(active, |a| {
            let id = util::new_id();
            a.conn.execute(
                "INSERT INTO sources (id, workspace_id, root_path, name, added_at) VALUES (?1, ?2, ?3, 'repo', ?4)",
                params![id, a.info.id, folder.display().to_string(), util::now()],
            )?;
            Ok(id)
        })
        .unwrap()
    }

    #[test]
    fn ignores_build_output_hidden_and_locked_files_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        write(&repo.join("src/lib.rs"), "fn main() {}");
        write(&repo.join("node_modules/pkg/index.js"), "x");
        write(&repo.join("target/debug/out.rs"), "x");
        write(&repo.join(".git/config"), "x");
        write(&repo.join("Cargo.lock"), "x");
        write(&repo.join("logs/run.log"), "x");
        write(&repo.join("notes.log"), "x");
        write(&repo.join(".gitignore"), "# comment\n*.log\n");
        write(&repo.join("picture.png"), "x");
        let names: Vec<String> = scan_folder(&repo).unwrap().found.into_iter().map(|c| c.relative).collect();
        assert_eq!(names, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn binary_and_oversized_files_are_skipped_with_a_count() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        write(&repo.join("big.rs"), &"a".repeat(MAX_CODE_BYTES as usize + 10));
        write(&repo.join("ok.rs"), "fn ok() {}");
        let scan = scan_folder(&repo).unwrap();
        assert_eq!(scan.skipped, 1);
        assert_eq!(scan.found.len(), 1);
    }

    #[test]
    fn sync_mirrors_files_as_read_only_pages_and_follows_changes() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        write(&repo.join("src/auth.rs"), "pub fn passkey_login() {}");
        write(&repo.join("docs/plan.md"), "# Plan\n\nShip passkeys first.");
        let active = shared(&dir.path().join("ws"));
        let source = add_source(&active, &repo);

        let first = sync_source(&active, &source).unwrap();
        assert_eq!(first.added, 2);
        assert_eq!(first.failed, 0);

        with_active(&active, |a| {
            let hits = crate::search::search(&a.conn, &a.info.id, "passkey_login").unwrap();
            assert!(hits.iter().any(|h| h.title == "src/auth.rs"), "code is searchable by its file path and text");
            let page_id: String = a
                .conn
                .query_row("SELECT id FROM pages WHERE source_path = 'src/auth.rs'", [], |r| r.get(0))
                .unwrap();
            let page = pages::get(&a.conn, &a.info.id, &page_id).unwrap();
            assert_eq!(page.source_id.as_deref(), Some(source.as_str()));
            let refused = pages::update(&a.conn, &a.info.id, &page_id, "Edited", &page.body, page.revision);
            assert!(matches!(refused, Err(AppError::Validation(_))), "a source page cannot be edited in the app");
            Ok(())
        })
        .unwrap();

        // Nothing changed: the second sync reads and changes nothing.
        let again = sync_source(&active, &source).unwrap();
        assert_eq!((again.added, again.updated, again.removed), (0, 0, 0));
        assert_eq!(again.unchanged, 2);

        // A changed file is updated; a deleted file goes to Trash.
        write(&repo.join("src/auth.rs"), "pub fn passkey_login() { verify_device(); }");
        fs::remove_file(repo.join("docs/plan.md")).unwrap();
        let third = sync_source(&active, &source).unwrap();
        assert_eq!((third.updated, third.removed), (1, 1));
        with_active(&active, |a| {
            let hits = crate::search::search(&a.conn, &a.info.id, "verify_device").unwrap();
            assert_eq!(hits.len(), 1, "the new content is searchable");
            let trashed: i64 = a
                .conn
                .query_row("SELECT COUNT(*) FROM pages WHERE source_path = 'docs/plan.md' AND deleted_at IS NOT NULL", [], |r| r.get(0))
                .unwrap();
            assert_eq!(trashed, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn the_sample_project_links_its_code_and_documents() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("samples").join("sample-repo");
        let names: Vec<String> = scan_folder(&sample).unwrap().found.into_iter().map(|c| c.relative).collect();
        for expected in ["README.md", "docs/decisions.md", "src/charge.rs", "src/refund.rs", "src/retry.py", "tests/test_retry.py"] {
            assert!(names.contains(&expected.to_string()), "{expected} should be linked, found {names:?}");
        }
        assert_eq!(names.len(), 6, "the .gitignore file itself is not linked");
    }

    #[test]
    fn a_missing_folder_is_reported_and_its_pages_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        write(&repo.join("a.rs"), "fn a() {}");
        let active = shared(&dir.path().join("ws"));
        let source = add_source(&active, &repo);
        sync_source(&active, &source).unwrap();
        fs::remove_dir_all(&repo).unwrap();
        assert!(sync_source(&active, &source).is_err());
        with_active(&active, |a| {
            let live: i64 = a
                .conn
                .query_row("SELECT COUNT(*) FROM pages WHERE source_id = ?1 AND deleted_at IS NULL", params![source], |r| r.get(0))
                .unwrap();
            assert_eq!(live, 1, "a missing folder does not delete anything");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn removing_a_source_keeps_its_pages_in_trash_as_ordinary_pages() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        write(&repo.join("a.rs"), "fn a() {}");
        let active = shared(&dir.path().join("ws"));
        let source = add_source(&active, &repo);
        sync_source(&active, &source).unwrap();
        with_active(&active, |a| {
            let ids: Vec<String> = {
                let mut stmt = a.conn.prepare("SELECT id FROM pages WHERE source_id = ?1 AND deleted_at IS NULL")?;
                let rows = stmt.query_map(params![source], |r| r.get(0))?;
                rows.collect::<Result<_, _>>()?
            };
            for page_id in &ids {
                pages::trash(&a.conn, &a.info.id, page_id)?;
            }
            a.conn.execute("UPDATE pages SET source_id = NULL WHERE source_id = ?1", params![source])?;
            a.conn.execute("DELETE FROM sources WHERE id = ?1", params![source])?;
            let count: i64 = a.conn.query_row("SELECT COUNT(*) FROM pages WHERE deleted_at IS NOT NULL", [], |r| r.get(0))?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
        assert!(!sources_still_registered(&active));
    }

    fn sources_still_registered(active: &Arc<Mutex<Option<Active>>>) -> bool {
        with_active(active, |a| {
            let n: i64 = a.conn.query_row("SELECT COUNT(*) FROM sources", [], |r| r.get(0))?;
            Ok(n > 0)
        })
        .unwrap()
    }
}
