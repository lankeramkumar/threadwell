//! Folder import: read Markdown and plain-text notes from a folder on disk, one note per file.
//!
//! The folder is read only. Nothing is written, moved or renamed there. Each file becomes a new
//! page. A record of its path and content hash stops an unchanged file from being imported
//! twice. A file that changed since its last import is imported again as a new page, and the
//! earlier page is left as it was.
//!
//! Safety: the chosen folder must be a real directory (not a symlink). Symlinks are never
//! followed, hidden entries are skipped, depth and file count are capped, and every path is
//! checked to stay inside the chosen folder after canonicalization. OneNote (`.one`) and other
//! binary formats are listed as unsupported.

use std::fs;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::commands::{with_active, AppState};
use crate::db::Tx;
use crate::error::{validation, AppError, AppResult};
use crate::{documents, markdown, pages, util};

pub const MAX_FILES: usize = 2_000;
const MAX_DEPTH: usize = 8;
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const SUPPORTED: &[&str] = &["md", "markdown", "txt", "docx", "pdf"];
const UNSUPPORTED: &[&str] = &["one", "onetoc2", "doc", "enex"];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanItem {
    /// Path relative to the chosen folder, with forward slashes.
    pub relative_path: String,
    pub size: u64,
    /// "new", "imported" (unchanged since import), "changed", "unsupported" or "too_large".
    pub status: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub items: Vec<ScanItem>,
    pub truncated: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
    pub failed: Vec<ImportFailure>,
    pub page_ids: Vec<String>,
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The folder must be absolute and must be a real directory, not a symlink or junction.
fn validate_root(root: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(root.trim());
    if !path.is_absolute() {
        return validation("Choose a full folder path");
    }
    let meta = fs::symlink_metadata(&path).map_err(|_| AppError::Validation("That folder does not exist".into()))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return validation("Choose a regular folder, not a link");
    }
    Ok(path)
}

/// Turns a relative path from the frontend into a real file inside `root`, or refuses it.
fn resolve(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let rel = Path::new(relative);
    let plain = !relative.is_empty() && rel.components().all(|c| matches!(c, Component::Normal(_)));
    if !plain || rel.is_absolute() {
        return validation("That path is not allowed");
    }
    let path = root.join(rel);
    let meta = fs::symlink_metadata(&path).map_err(|_| AppError::NotFound("File".into()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return validation("Only regular files can be imported");
    }
    let canonical_root = fs::canonicalize(root)?;
    let canonical = fs::canonicalize(&path)?;
    if !canonical.starts_with(&canonical_root) {
        return validation("That file is outside the chosen folder");
    }
    Ok(path)
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default()
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64, String)>, truncated: &mut bool) -> AppResult<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk(root, &path, depth + 1, out, truncated)?;
        } else if meta.is_file() {
            let ext = extension(&path);
            let status = if SUPPORTED.contains(&ext.as_str()) {
                if meta.len() > MAX_FILE_BYTES { "too_large" } else { "candidate" }
            } else if UNSUPPORTED.contains(&ext.as_str()) {
                "unsupported"
            } else {
                continue;
            };
            if out.len() >= MAX_FILES {
                *truncated = true;
                return Ok(());
            }
            out.push((path.clone(), meta.len(), status.to_string()));
        }
    }
    Ok(())
}

pub fn scan(conn: &Connection, ws: &str, root_text: &str) -> AppResult<ScanReport> {
    let root = validate_root(root_text)?;
    let mut found = Vec::new();
    let mut truncated = false;
    walk(&root, &root, 0, &mut found, &mut truncated)?;
    let mut items = Vec::with_capacity(found.len());
    for (path, size, status) in found {
        let relative = relative_string(&root, &path);
        let status = if status == "candidate" {
            match conn
                .query_row(
                    "SELECT content_hash FROM source_imports WHERE workspace_id = ?1 AND source_path = ?2",
                    params![ws, relative],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
            {
                None => "new".to_string(),
                Some(stored) => {
                    let current = hash_bytes(&fs::read(&path)?);
                    if current == stored { "imported" } else { "changed" }.to_string()
                }
            }
        } else {
            status
        };
        items.push(ScanItem { relative_path: relative, size, status });
    }
    Ok(ScanReport { items, truncated })
}

pub fn import(conn: &Connection, ws: &str, root_text: &str, relative_paths: &[String], parent: Option<&str>) -> AppResult<ImportReport> {
    let root = validate_root(root_text)?;
    if relative_paths.len() > MAX_FILES {
        return validation("Too many files selected at once");
    }
    let mut report = ImportReport::default();
    for relative in relative_paths {
        match import_one(conn, ws, &root, relative, parent) {
            Ok(ImportOutcome::Imported(page_id)) => {
                report.imported += 1;
                report.page_ids.push(page_id);
            }
            Ok(ImportOutcome::Skipped) => report.skipped += 1,
            Err(error) => report.failed.push(ImportFailure {
                relative_path: relative.clone(),
                reason: error.to_string(),
            }),
        }
    }
    Ok(report)
}

enum ImportOutcome {
    Imported(String),
    Skipped,
}

fn import_one(conn: &Connection, ws: &str, root: &Path, relative: &str, parent: Option<&str>) -> AppResult<ImportOutcome> {
    let path = resolve(root, relative)?;
    let ext = extension(&path);
    if !SUPPORTED.contains(&ext.as_str()) {
        return validation("This file type is not supported. Export it as Markdown or text first.");
    }
    let size = fs::metadata(&path)?.len();
    if size > MAX_FILE_BYTES {
        return validation("This file is larger than the 5 MB limit");
    }
    let bytes = fs::read(&path)?;
    let hash = hash_bytes(&bytes);
    let already: Option<String> = conn
        .query_row(
            "SELECT content_hash FROM source_imports WHERE workspace_id = ?1 AND source_path = ?2",
            params![ws, relative],
            |r| r.get(0),
        )
        .optional()?;
    if already.as_deref() == Some(hash.as_str()) {
        return Ok(ImportOutcome::Skipped);
    }
    let text = match ext.as_str() {
        "docx" => documents::docx_to_markdown(&bytes).map_err(AppError::Validation)?,
        "pdf" => documents::pdf_to_text(&bytes).map_err(AppError::Validation)?,
        _ => String::from_utf8(bytes).map_err(|_| AppError::Validation("The file is not UTF-8 text".into()))?,
    };

    let mut doc = markdown::from_markdown(&text);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported note");
    let title: String = markdown::take_title(&mut doc)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| stem.to_string())
        .chars()
        .take(200)
        .collect();

    let tx = Tx::begin(conn)?;
    let page = pages::create(&tx, ws, &title, parent)?;
    pages::update(&tx, ws, &page.id, &title, &doc, page.revision)?;
    tx.execute(
        "INSERT INTO source_imports (id, workspace_id, page_id, source_path, content_hash, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (workspace_id, source_path) DO UPDATE SET
            page_id = excluded.page_id, content_hash = excluded.content_hash, imported_at = excluded.imported_at",
        params![util::new_id(), ws, page.id, relative, hash, util::now()],
    )?;
    tx.commit()?;
    Ok(ImportOutcome::Imported(page.id))
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn local_scan_folder(state: State<'_, AppState>, path: String) -> AppResult<ScanReport> {
    with_active(&state.active, |a| scan(&a.conn, &a.info.id, &path))
}

#[tauri::command]
pub async fn local_import_folder(
    state: State<'_, AppState>,
    path: String,
    relative_paths: Vec<String>,
    parent_id: Option<String>,
) -> AppResult<ImportReport> {
    with_active(&state.active, |a| import(&a.conn, &a.info.id, &path, &relative_paths, parent_id.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    struct Fixture {
        _dir: tempfile::TempDir,
        notes: PathBuf,
        conn: Connection,
        ws: String,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("i.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'I', ?2)", params![ws, util::now()]).unwrap();
        let notes = dir.path().join("notes");
        fs::create_dir_all(notes.join("sub")).unwrap();
        fs::write(notes.join("plan.md"), "# Plan\n\n- first step").unwrap();
        fs::write(notes.join("sub").join("ideas.txt"), "Plain idea text").unwrap();
        fs::write(notes.join("old.one"), "binary onenote data").unwrap();
        fs::write(notes.join(".hidden.md"), "secret draft").unwrap();
        Fixture { _dir: dir, notes, conn, ws }
    }

    fn root(f: &Fixture) -> String {
        f.notes.display().to_string()
    }

    #[test]
    fn scan_lists_notes_and_marks_unsupported_files_without_reading_hidden_ones() {
        let f = fixture();
        let report = scan(&f.conn, &f.ws, &root(&f)).unwrap();
        let names: Vec<(&str, &str)> = report.items.iter().map(|i| (i.relative_path.as_str(), i.status.as_str())).collect();
        assert!(names.contains(&("plan.md", "new")));
        assert!(names.contains(&("sub/ideas.txt", "new")));
        assert!(names.contains(&("old.one", "unsupported")));
        assert!(!names.iter().any(|(p, _)| p.contains("hidden")));
    }

    #[test]
    fn import_creates_pages_and_never_changes_the_source_folder() {
        let f = fixture();
        let before = fs::read(f.notes.join("plan.md")).unwrap();
        let report = import(&f.conn, &f.ws, &root(&f), &["plan.md".into(), "sub/ideas.txt".into()], None).unwrap();
        assert_eq!(report.imported, 2);
        assert!(report.failed.is_empty());
        assert_eq!(fs::read(f.notes.join("plan.md")).unwrap(), before);
        let titles: Vec<String> = pages::list(&f.conn, &f.ws).unwrap().into_iter().map(|p| p.title).collect();
        assert!(titles.contains(&"Plan".to_string()), "a leading heading becomes the title");
        assert!(titles.contains(&"ideas".to_string()), "otherwise the file name is the title");
    }

    #[test]
    fn unchanged_files_are_skipped_and_changed_files_are_reported_as_changed() {
        let f = fixture();
        import(&f.conn, &f.ws, &root(&f), &["plan.md".into()], None).unwrap();
        let again = import(&f.conn, &f.ws, &root(&f), &["plan.md".into()], None).unwrap();
        assert_eq!((again.imported, again.skipped), (0, 1));

        fs::write(f.notes.join("plan.md"), "# Plan\n\n- changed step").unwrap();
        let after = scan(&f.conn, &f.ws, &root(&f)).unwrap();
        let plan = after.items.iter().find(|i| i.relative_path == "plan.md").unwrap();
        assert_eq!(plan.status, "changed");
    }

    #[test]
    fn traversal_and_absolute_paths_are_refused() {
        let f = fixture();
        let report = import(
            &f.conn,
            &f.ws,
            &root(&f),
            &["../outside.md".into(), "C:/Windows/win.ini".into(), "".into()],
            None,
        )
        .unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.failed.len(), 3);
    }

    #[test]
    fn unsupported_and_non_utf8_files_are_reported_not_imported() {
        let f = fixture();
        fs::write(f.notes.join("binary.txt"), [0xff_u8, 0xfe, 0x00, 0x81]).unwrap();
        let report = import(&f.conn, &f.ws, &root(&f), &["old.one".into(), "binary.txt".into()], None).unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.failed.len(), 2);
        assert!(report.failed.iter().any(|x| x.reason.contains("not supported")));
        assert!(report.failed.iter().any(|x| x.reason.contains("UTF-8")));
    }

    #[test]
    fn relative_roots_and_missing_folders_are_refused() {
        let f = fixture();
        assert!(scan(&f.conn, &f.ws, "relative/folder").is_err());
        assert!(scan(&f.conn, &f.ws, &f.notes.join("missing").display().to_string()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn symlinks_are_never_followed() {
        let f = fixture();
        let target = f._dir.path().join("secret.md");
        fs::write(&target, "outside the folder").unwrap();
        if std::os::windows::fs::symlink_file(&target, f.notes.join("link.md")).is_err() {
            return; // creating symlinks needs developer mode or elevation on some machines
        }
        let report = scan(&f.conn, &f.ws, &root(&f)).unwrap();
        assert!(!report.items.iter().any(|i| i.relative_path == "link.md"));
        let imported = import(&f.conn, &f.ws, &root(&f), &["link.md".into()], None).unwrap();
        assert_eq!(imported.imported, 0);
    }
}

#[cfg(test)]
mod demo_project_tests {
    use super::*;
    use crate::db;

    #[test]
    fn demo_project_imports_every_note_format() {
        let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("samples").join("demo-project");
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("demo.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'Demo', ?2)", params![ws, util::now()]).unwrap();

        let root = demo.display().to_string();
        let scanned = scan(&conn, &ws, &root).unwrap();
        let notes: Vec<&ScanItem> = scanned.items.iter().filter(|i| i.status == "new").collect();
        assert_eq!(notes.len(), 40, "10 each of md, txt, docx and pdf");
        let paths: Vec<String> = notes.iter().map(|i| i.relative_path.clone()).collect();
        let report = import(&conn, &ws, &root, &paths, None).unwrap();
        assert!(report.failed.is_empty(), "failed: {:?}", report.failed);
        assert_eq!(report.imported, 40);
        assert_eq!(pages::list(&conn, &ws).unwrap().len(), 40);

        let again = import(&conn, &ws, &root, &paths, None).unwrap();
        assert_eq!((again.imported, again.skipped), (0, 40));
    }
}
