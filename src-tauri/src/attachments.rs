//! Files attached to pages. Each attachment is copied into the workspace's `attachments` folder
//! under a generated name, so the original path never matters again. Executable and script types
//! are refused, because attaching them would store something that can run. The original name is
//! kept for display only, after it is sanitized.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::commands::{with_active, AppState};
use crate::db::ATTACHMENTS_DIR;
use crate::error::{validation, AppError, AppResult};
use crate::{pages, transfer, util};

pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "scr", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "msi", "msp", "lnk",
    "reg", "dll", "cpl", "hta", "jar", "app", "sh",
];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub page_id: String,
    pub file_name: String,
    pub size: i64,
    pub sha256: String,
    pub created_at: String,
}

fn stored_extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(|e| format!(".{e}"))
        .unwrap_or_default()
}

pub fn add(conn: &Connection, ws: &str, root: &Path, page_id: &str, src_text: &str) -> AppResult<Attachment> {
    util::validate_id(page_id)?;
    pages::get(conn, ws, page_id)?;
    let src = util::validate_abs_path(src_text)?;
    let meta = fs::symlink_metadata(&src).map_err(|_| AppError::Validation("That file does not exist".into()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return validation("Choose a regular file, not a link or folder");
    }
    if meta.len() > MAX_ATTACHMENT_BYTES {
        return validation("Attachments can be at most 25 MB");
    }
    let original = src.file_name().and_then(|n| n.to_str()).unwrap_or("attachment").to_string();
    let ext = stored_extension(&original);
    if BLOCKED_EXTENSIONS.iter().any(|b| ext == format!(".{b}")) {
        return validation("Programs and scripts cannot be attached. Attach a document or export the data instead.");
    }
    let bytes = fs::read(&src)?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let id = util::new_id();
    let stored_name = format!("{id}{ext}");
    let dir = root.join(ATTACHMENTS_DIR);
    fs::create_dir_all(&dir)?;
    let target = dir.join(&stored_name);
    fs::write(&target, &bytes)?;
    let display: String = transfer::sanitize_filename(&original).chars().take(255).collect();
    let now = util::now();
    conn.execute(
        "INSERT INTO attachments (id, workspace_id, page_id, file_name, stored_name, size, sha256, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, ws, page_id, display, stored_name, bytes.len() as i64, sha256, now],
    )?;
    Ok(Attachment { id, page_id: page_id.into(), file_name: display, size: bytes.len() as i64, sha256, created_at: now })
}

pub fn list(conn: &Connection, ws: &str, page_id: &str) -> AppResult<Vec<Attachment>> {
    util::validate_id(page_id)?;
    let mut stmt = conn.prepare(
        "SELECT id, page_id, file_name, size, sha256, created_at FROM attachments
         WHERE workspace_id = ?1 AND page_id = ?2 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![ws, page_id], |row| {
        Ok(Attachment {
            id: row.get(0)?,
            page_id: row.get(1)?,
            file_name: row.get(2)?,
            size: row.get(3)?,
            sha256: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn stored_path(conn: &Connection, ws: &str, root: &Path, id: &str) -> AppResult<PathBuf> {
    util::validate_id(id)?;
    let stored: String = conn
        .query_row(
            "SELECT stored_name FROM attachments WHERE id = ?1 AND workspace_id = ?2",
            params![id, ws],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(AppError::NotFound("Attachment".into()))?;
    Ok(root.join(ATTACHMENTS_DIR).join(stored))
}

pub fn remove(conn: &Connection, ws: &str, root: &Path, id: &str) -> AppResult<()> {
    let path = stored_path(conn, ws, root, id)?;
    conn.execute("DELETE FROM attachments WHERE id = ?1 AND workspace_id = ?2", params![id, ws])?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn path(conn: &Connection, ws: &str, root: &Path, id: &str) -> AppResult<PathBuf> {
    let p = stored_path(conn, ws, root, id)?;
    if !p.is_file() {
        return Err(AppError::NotFound("Attachment file".into()));
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn attachments_list(state: State<'_, AppState>, page_id: String) -> AppResult<Vec<Attachment>> {
    with_active(&state.active, |a| list(&a.conn, &a.info.id, &page_id))
}

#[tauri::command]
pub async fn attachment_add(state: State<'_, AppState>, page_id: String, src_path: String) -> AppResult<Attachment> {
    with_active(&state.active, |a| add(&a.conn, &a.info.id, &a.root, &page_id, &src_path))
}

#[tauri::command]
pub async fn attachment_remove(state: State<'_, AppState>, id: String) -> AppResult<()> {
    with_active(&state.active, |a| remove(&a.conn, &a.info.id, &a.root, &id))
}

/// Opens File Explorer with the attachment selected. Only the stored path is used; nothing runs.
#[tauri::command]
pub async fn attachment_reveal(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let target = with_active(&state.active, |a| path(&a.conn, &a.info.id, &a.root, &id))?;
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", target.display()))
        .spawn()
        .map_err(|_| AppError::Validation("Could not open File Explorer".into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, pages};

    fn setup() -> (tempfile::TempDir, Connection, String, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("a.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'A', ?2)", params![ws, util::now()]).unwrap();
        let root = dir.path().join("ws");
        fs::create_dir_all(&root).unwrap();
        (dir, conn, ws, root)
    }

    #[test]
    fn attaches_a_copy_and_keeps_the_original_untouched() {
        let (dir, conn, ws, root) = setup();
        let page = pages::create(&conn, &ws, "Quote", None).unwrap();
        let src = dir.path().join("quote.pdf");
        fs::write(&src, b"%PDF-1.4 fake quote").unwrap();
        let before = fs::read(&src).unwrap();
        let attached = add(&conn, &ws, &root, &page.id, &src.display().to_string()).unwrap();
        assert_eq!(attached.file_name, "quote.pdf");
        assert_eq!(fs::read(&src).unwrap(), before);
        let stored = path(&conn, &ws, &root, &attached.id).unwrap();
        assert_ne!(stored, src, "the attachment is a copy inside the workspace");
        assert_eq!(list(&conn, &ws, &page.id).unwrap().len(), 1);
    }

    #[test]
    fn executables_and_scripts_are_refused() {
        let (dir, conn, ws, root) = setup();
        let page = pages::create(&conn, &ws, "Tools", None).unwrap();
        for name in ["setup.exe", "run.ps1", "shortcut.lnk", "macro.JS"] {
            let src = dir.path().join(name);
            fs::write(&src, b"x").unwrap();
            assert!(add(&conn, &ws, &root, &page.id, &src.display().to_string()).is_err(), "{name}");
        }
    }

    #[test]
    fn removing_deletes_the_stored_copy() {
        let (dir, conn, ws, root) = setup();
        let page = pages::create(&conn, &ws, "Notes", None).unwrap();
        let src = dir.path().join("photo.png");
        fs::write(&src, b"png bytes").unwrap();
        let attached = add(&conn, &ws, &root, &page.id, &src.display().to_string()).unwrap();
        let stored = path(&conn, &ws, &root, &attached.id).unwrap();
        remove(&conn, &ws, &root, &attached.id).unwrap();
        assert!(!stored.exists());
        assert!(list(&conn, &ws, &page.id).unwrap().is_empty());
        assert!(src.exists(), "removing an attachment never touches the original");
    }

    #[test]
    fn another_workspaces_attachment_is_not_reachable() {
        let (dir, conn, ws, root) = setup();
        let other = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'B', ?2)", params![other, util::now()]).unwrap();
        let page = pages::create(&conn, &ws, "Mine", None).unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"a").unwrap();
        let attached = add(&conn, &ws, &root, &page.id, &src.display().to_string()).unwrap();
        assert!(path(&conn, &other, &root, &attached.id).is_err());
        assert!(remove(&conn, &other, &root, &attached.id).is_err());
    }
}
