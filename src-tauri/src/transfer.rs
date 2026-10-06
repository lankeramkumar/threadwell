//! Import and export: Markdown per page, tasks as CSV, and a versioned full-workspace
//! backup that can be restored into a new folder.
//!
//! Imports copy content into the workspace and never modify the source file.
//! Exports never overwrite existing files; name collisions get a numeric suffix.
//! Destinations inside the open workspace folder are refused so exports cannot
//! clobber the database.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::documents;
use crate::db::{self, ATTACHMENTS_DIR, DB_FILE};
use crate::error::{validation, AppError, AppResult};
use crate::pages::{self, Page};
use crate::{markdown, tasks, util, workspace};

pub const BACKUP_FORMAT: &str = "threadwell-backup";
pub const BACKUP_FORMAT_VERSION: u32 = 1;
const BACKUP_DB_FILE: &str = "workspace.db";
const MANIFEST_FILE: &str = "manifest.json";
const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub schema_version: i64,
    pub database_sha256: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub created_at: String,
    pub pages: i64,
    pub tasks: i64,
}

/// Replaces characters that are invalid in Windows filenames and trims dots and spaces.
pub fn sanitize_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .take(100)
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1"];
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else if reserved.iter().any(|r| r.eq_ignore_ascii_case(trimmed)) {
        format!("{trimmed}_")
    } else {
        trimmed.to_string()
    }
}

/// Validates that an export or restore destination is an absolute path outside the
/// open workspace folder.
fn validate_destination(dest: &Path, workspace_root: &Path) -> AppResult<()> {
    if !dest.is_absolute() {
        return validation("Choose a full destination path");
    }
    if dest.components().any(|c| matches!(c, Component::ParentDir)) {
        return validation("Destination path cannot contain '..'");
    }
    let root = fs::canonicalize(workspace_root)?;
    if resolve_existing_prefix(dest)?.starts_with(&root) {
        return validation("Choose a folder outside the open workspace");
    }
    Ok(())
}

/// Canonicalizes the longest existing ancestor of `path` and re-appends the rest,
/// so comparisons work for destinations that do not exist yet.
fn resolve_existing_prefix(path: &Path) -> AppResult<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    while !existing.exists() {
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name);
                existing = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut resolved = fs::canonicalize(&existing)?;
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

fn unique_file(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let base = sanitize_filename(stem);
    let mut candidate = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{base} ({n}).{ext}"));
        n += 1;
    }
    candidate
}

pub fn export_page_markdown(
    conn: &Connection,
    ws: &str,
    workspace_root: &Path,
    page_id: &str,
    dest_dir: &Path,
) -> AppResult<PathBuf> {
    validate_destination(dest_dir, workspace_root)?;
    fs::create_dir_all(dest_dir)?;
    let page = pages::get(conn, ws, page_id)?;
    write_page(&page, dest_dir)
}

fn write_page(page: &Page, dest_dir: &Path) -> AppResult<PathBuf> {
    let path = unique_file(dest_dir, &page.title, "md");
    let mut file = File::options().write(true).create_new(true).open(&path)?;
    file.write_all(markdown::to_markdown(&page.body).as_bytes())?;
    Ok(path)
}

pub fn export_all_markdown(
    conn: &Connection,
    ws: &str,
    workspace_root: &Path,
    dest_dir: &Path,
) -> AppResult<usize> {
    validate_destination(dest_dir, workspace_root)?;
    fs::create_dir_all(dest_dir)?;
    let summaries = pages::list(conn, ws)?;
    for summary in &summaries {
        let page = pages::get(conn, ws, &summary.id)?;
        write_page(&page, dest_dir)?;
    }
    Ok(summaries.len())
}

/// Escapes one CSV field. Cells that start with a formula trigger are prefixed with
/// an apostrophe so spreadsheet apps show them as text.
fn csv_field(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

pub fn export_tasks_csv(conn: &Connection, ws: &str, workspace_root: &Path, dest_file: &Path) -> AppResult<usize> {
    if !dest_file.is_absolute() {
        return validation("Choose a full destination path");
    }
    validate_destination(dest_file.parent().unwrap_or(dest_file), workspace_root)?;
    if dest_file.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase) != Some("csv".into()) {
        return validation("Tasks export must be a .csv file");
    }
    let all = tasks::list_tasks(conn, ws, None)?;
    let projects = tasks::list_projects(conn, ws)?;
    let mut out = String::from("id,title,description,status,priority,due_date,project,created_at,updated_at\n");
    for task in &all {
        let project = task
            .project_id
            .as_ref()
            .and_then(|pid| projects.iter().find(|p| &p.id == pid))
            .map(|p| p.name.as_str())
            .unwrap_or("");
        let row = [
            task.id.as_str(),
            &task.title,
            &task.description,
            &task.status,
            &task.priority,
            task.due_date.as_deref().unwrap_or(""),
            project,
            &task.created_at,
            &task.updated_at,
        ];
        let line: Vec<String> = row.iter().map(|f| csv_field(f)).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    let mut file = File::options().write(true).create_new(true).open(dest_file)?;
    file.write_all(out.as_bytes())?;
    Ok(all.len())
}

/// Imports a Markdown file as a new page. A leading level-1 heading becomes the title;
/// otherwise the file name is used.
pub fn import_markdown(conn: &Connection, ws: &str, src: &Path, parent: Option<&str>) -> AppResult<Page> {
    if !src.is_absolute() {
        return validation("Choose a file to import");
    }
    let ext = src.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !documents::READABLE_EXTENSIONS.contains(&ext.as_str()) {
        return validation("Only Markdown, text, Word, PDF and CSV files can be imported");
    }
    let meta = fs::metadata(src)?;
    if !meta.is_file() {
        return validation("Choose a file to import");
    }
    if meta.len() > MAX_IMPORT_BYTES {
        return validation("This file is larger than the 5 MB import limit");
    }
    let bytes = fs::read(src)?;
    let text = documents::file_to_markdown(&ext, &bytes).map_err(AppError::Validation)?;
    let mut doc = markdown::from_markdown(&text);
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported page");
    let title = markdown::take_title(&mut doc)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| stem.to_string());
    let title: String = title.chars().take(200).collect();
    let page = pages::create(conn, ws, &title, parent)?;
    pages::update(conn, ws, &page.id, &title, &doc, page.revision)
}

// ---------------------------------------------------------------------------
// Backup and restore
// ---------------------------------------------------------------------------

fn sha256_file(path: &Path) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn copy_dir(src: &Path, dst: &Path) -> AppResult<()> {
    fs::create_dir_all(dst)?;
    if !src.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Writes a consistent snapshot of the open workspace into a new timestamped folder
/// inside `dest_parent`, using SQLite's online backup API so WAL contents are included.
pub fn create_backup(conn: &Connection, workspace_root: &Path, dest_parent: &Path) -> AppResult<BackupInfo> {
    validate_destination(dest_parent, workspace_root)?;
    fs::create_dir_all(dest_parent)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f").to_string();
    let dest = dest_parent.join(format!("threadwell-backup-{stamp}"));
    if dest.exists() {
        return validation("A backup with this name already exists. Try again.");
    }
    fs::create_dir_all(&dest)?;
    let db_path = dest.join(BACKUP_DB_FILE);
    {
        let mut target = Connection::open(&db_path)?;
        Backup::new(conn, &mut target)?.run_to_completion(256, Duration::from_millis(0), None)?;
        db::quick_check(&target)?;
    }
    copy_dir(&workspace_root.join(ATTACHMENTS_DIR), &dest.join(ATTACHMENTS_DIR))?;

    let info = workspace::read_summary(conn)?;
    let created_at = util::now();
    let manifest = Manifest {
        format: BACKUP_FORMAT.into(),
        format_version: BACKUP_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: created_at.clone(),
        workspace_id: info.id,
        workspace_name: info.name,
        schema_version: info.schema_version,
        database_sha256: sha256_file(&db_path)?,
    };
    let mut file = File::options().write(true).create_new(true).open(dest.join(MANIFEST_FILE))?;
    file.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    Ok(BackupInfo {
        path: dest.display().to_string(),
        created_at,
        pages: pages::count_live(conn, &manifest.workspace_id)?,
        tasks: tasks::list_tasks(conn, &manifest.workspace_id, None)?.len() as i64,
    })
}

/// Restores a backup folder into `dest_root`, which must not contain a workspace.
/// The database is verified against the manifest checksum before it is copied.
pub fn restore_backup(backup_dir: &Path, dest_root: &Path) -> AppResult<()> {
    if !backup_dir.is_absolute() || !dest_root.is_absolute() {
        return validation("Choose full folder paths for the backup and destination");
    }
    let manifest_path = backup_dir.join(MANIFEST_FILE);
    if fs::metadata(&manifest_path).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return validation("This folder is not a valid Threadwell backup");
    }
    let manifest: Manifest = serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|_| {
        AppError::Validation("This folder is not a valid Threadwell backup".into())
    })?)
    .map_err(|_| AppError::Validation("This folder is not a valid Threadwell backup".into()))?;
    if manifest.format != BACKUP_FORMAT || manifest.format_version != BACKUP_FORMAT_VERSION {
        return validation("This backup format is not supported by this version of Threadwell");
    }
    if manifest.schema_version > db::schema_version_latest() {
        return validation("This backup was made by a newer version of Threadwell");
    }
    let backup_db = backup_dir.join(BACKUP_DB_FILE);
    if sha256_file(&backup_db)? != manifest.database_sha256 {
        return validation("The backup database does not match its manifest. It may be damaged.");
    }
    if dest_root.exists() {
        let empty = dest_root.is_dir() && fs::read_dir(dest_root)?.next().is_none();
        if !empty {
            return validation("Restore into an empty folder. The destination already has content.");
        }
    }
    fs::create_dir_all(dest_root)?;
    let source = Connection::open_with_flags(&backup_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db::quick_check(&source)?;
    let mut target = Connection::open(dest_root.join(DB_FILE))?;
    Backup::new(&source, &mut target)?.run_to_completion(256, Duration::from_millis(0), None)?;
    drop(target);
    drop(source);

    // Re-open through the normal path so migrations and checks run on the restored copy.
    let restored = workspace::open(dest_root)?;
    if restored.info.id != manifest.workspace_id {
        return validation("The restored workspace does not match the backup manifest");
    }
    drop(restored);
    copy_dir(&backup_dir.join(ATTACHMENTS_DIR), &dest_root.join(ATTACHMENTS_DIR))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_hostile_titles() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_filename("  con  "), "con_");
        assert_eq!(sanitize_filename("..."), "Untitled");
        assert_eq!(sanitize_filename("a<b>c:d"), "a_b_c_d");
    }

    #[test]
    fn csv_escapes_quotes_commas_and_formulas() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=HYPERLINK(1)"), "'=HYPERLINK(1)");
    }

    #[test]
    fn export_never_overwrites_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_file(dir.path(), "Notes", "md");
        File::create(&first).unwrap();
        let second = unique_file(dir.path(), "Notes", "md");
        assert_ne!(first, second);
        assert!(second.ends_with("Notes (2).md"));
    }

    #[test]
    fn exports_are_refused_inside_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let ws_root = dir.path().join("ws");
        let active = workspace::create(&ws_root, "W", false).unwrap();
        let inside = ws_root.join("exports");
        assert!(validate_destination(&inside, &ws_root).is_err());
        let outside = dir.path().join("exports");
        assert!(validate_destination(&outside, &ws_root).is_ok());
        drop(active);
    }

    #[test]
    fn import_takes_title_from_leading_heading_and_leaves_source_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let ws_root = dir.path().join("ws");
        let active = workspace::create(&ws_root, "W", false).unwrap();
        let ws = active.info.id.clone();
        let src = dir.path().join("import.md");
        fs::write(&src, "# Kickoff\n\n- [ ] book room\n").unwrap();
        let before = fs::read(&src).unwrap();
        let page = import_markdown(&active.conn, &ws, &src, None).unwrap();
        assert_eq!(page.title, "Kickoff");
        assert_eq!(fs::read(&src).unwrap(), before);
    }

    #[test]
    fn import_rejects_wrong_extension_and_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let active = workspace::create(&dir.path().join("ws"), "W", false).unwrap();
        let ws = active.info.id.clone();
        let exe = dir.path().join("payload.exe");
        fs::write(&exe, "MZ").unwrap();
        assert!(import_markdown(&active.conn, &ws, &exe, None).is_err());
        assert!(import_markdown(&active.conn, &ws, Path::new("relative.md"), None).is_err());
    }

    #[test]
    fn backup_and_restore_round_trip_preserves_content() {
        let dir = tempfile::tempdir().unwrap();
        let ws_root = dir.path().join("ws");
        let active = workspace::create(&ws_root, "Round trip", true).unwrap();
        let ws = active.info.id.clone();
        let before_pages = pages::list(&active.conn, &ws).unwrap();
        let before_tasks = tasks::list_tasks(&active.conn, &ws, None).unwrap();

        let backup_parent = dir.path().join("backups");
        let info = create_backup(&active.conn, &ws_root, &backup_parent).unwrap();
        drop(active);

        let backup_dir = PathBuf::from(&info.path);
        let restored_root = dir.path().join("restored");
        restore_backup(&backup_dir, &restored_root).unwrap();

        let restored = workspace::open(&restored_root).unwrap();
        assert_eq!(restored.info.id, ws);
        assert_eq!(pages::list(&restored.conn, &ws).unwrap(), before_pages);
        let after_tasks = tasks::list_tasks(&restored.conn, &ws, None).unwrap();
        assert_eq!(after_tasks.len(), before_tasks.len());
        assert_eq!(after_tasks[0].title, before_tasks[0].title);
    }

    #[test]
    fn restore_rejects_modified_backup_database() {
        let dir = tempfile::tempdir().unwrap();
        let ws_root = dir.path().join("ws");
        let active = workspace::create(&ws_root, "Tamper", false).unwrap();
        let info = create_backup(&active.conn, &ws_root, &dir.path().join("b")).unwrap();
        drop(active);
        let backup_dir = PathBuf::from(&info.path);
        let db_file = backup_dir.join(BACKUP_DB_FILE);
        let mut bytes = fs::read(&db_file).unwrap();
        bytes.push(0);
        fs::write(&db_file, bytes).unwrap();
        assert!(restore_backup(&backup_dir, &dir.path().join("out")).is_err());
    }

    #[test]
    fn restore_refuses_non_empty_destination() {
        let dir = tempfile::tempdir().unwrap();
        let ws_root = dir.path().join("ws");
        let active = workspace::create(&ws_root, "Keep", false).unwrap();
        let info = create_backup(&active.conn, &ws_root, &dir.path().join("b")).unwrap();
        drop(active);
        let existing = dir.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("note.txt"), "mine").unwrap();
        assert!(restore_backup(Path::new(&info.path), &existing).is_err());
    }
}
