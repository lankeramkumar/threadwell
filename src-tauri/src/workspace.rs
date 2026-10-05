//! A workspace is a folder containing `threadwell.db` (authoritative data) and an
//! `attachments/` directory. Opening a workspace runs pending migrations and an
//! integrity check before anything else touches it.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::{self, ATTACHMENTS_DIR, DB_FILE};
use crate::error::{validation, AppError, AppResult};
use crate::{sample, util};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub schema_version: i64,
}

/// The open workspace. Held behind a mutex in the Tauri state.
pub struct Active {
    pub root: PathBuf,
    pub conn: Connection,
    pub info: WorkspaceInfo,
}

pub fn create(root: &Path, name: &str, with_sample: bool) -> AppResult<Active> {
    let name = util::validate_line(name, "Workspace name", 80)?;
    if root.join(DB_FILE).exists() {
        return validation("That folder already contains a Threadwell workspace. Open it instead.");
    }
    fs::create_dir_all(root.join(ATTACHMENTS_DIR))?;
    let mut conn = db::open(&root.join(DB_FILE))?;
    db::migrate(&mut conn)?;
    let id = util::new_id();
    conn.execute(
        "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, util::now()],
    )?;
    conn.execute("INSERT INTO settings (key, value) VALUES ('theme', 'system')", [])?;
    if with_sample {
        sample::seed(&conn, &id)?;
    }
    finish(root, conn)
}

pub fn open(root: &Path) -> AppResult<Active> {
    if !root.join(DB_FILE).is_file() {
        return Err(AppError::NotFound("Threadwell workspace".into()));
    }
    let mut conn = db::open(&root.join(DB_FILE))?;
    db::quick_check(&conn)?;
    db::migrate(&mut conn)?;
    fs::create_dir_all(root.join(ATTACHMENTS_DIR))?;
    finish(root, conn)
}

fn finish(root: &Path, conn: Connection) -> AppResult<Active> {
    let info = read_info(&conn, root)?;
    Ok(Active { root: root.to_path_buf(), conn, info })
}

/// Workspace identity without a filesystem path, used for backups.
pub fn read_summary(conn: &Connection) -> AppResult<WorkspaceInfo> {
    read_info(conn, Path::new(""))
}

fn read_info(conn: &Connection, root: &Path) -> AppResult<WorkspaceInfo> {
    let meta = conn
        .query_row("SELECT id, name, created_at FROM workspace_meta LIMIT 1", [], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .optional()?;
    let Some((id, name, created_at)) = meta else {
        return validation("This folder does not contain a Threadwell workspace");
    };
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(WorkspaceInfo {
        id,
        name,
        path: root.display().to_string(),
        created_at,
        schema_version,
    })
}

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    let allowed: &[&str] = &[
        "theme",
        "ai.endpoint",
        "ai.model",
        "ai.allow_remote",
        "ai.embed_model",
        "ai.retrieval_mode",
        "ai.weight_lexical",
        "ai.weight_vector",
    ];
    if !allowed.contains(&key) {
        return validation("Unknown setting");
    }
    if key == "theme" && !matches!(value, "system" | "light" | "dark") {
        return validation("Theme must be system, light or dark");
    }
    if key == "ai.retrieval_mode" && !matches!(value, "lexical" | "hybrid") {
        return validation("Retrieval mode must be lexical or hybrid");
    }
    if key.starts_with("ai.weight_") {
        match value.parse::<f32>() {
            Ok(w) if (0.0..=1.0).contains(&w) => {}
            _ => return validation("Weights must be numbers between 0 and 1"),
        }
    }
    if key == "ai.allow_remote" && !matches!(value, "true" | "false") {
        return validation("ai.allow_remote must be true or false");
    }
    if value.len() > 2048 {
        return validation("Setting value is too long");
    }
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_then_reopen_keeps_workspace_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        let created = create(&root, "  Notes  ", false).unwrap();
        let id = created.info.id.clone();
        drop(created);

        let reopened = open(&root).unwrap();
        assert_eq!(reopened.info.id, id);
        assert_eq!(reopened.info.name, "Notes");
        assert_eq!(reopened.info.schema_version, db::schema_version_latest());
    }

    #[test]
    fn refuses_to_create_over_existing_workspace() {
        let dir = tempfile::tempdir().unwrap();
        create(dir.path(), "One", false).unwrap();
        assert!(create(dir.path(), "Two", false).is_err());
    }

    #[test]
    fn open_missing_folder_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(open(dir.path()), Err(AppError::NotFound(_))));
    }

    #[test]
    fn refuses_database_from_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let active = create(dir.path(), "Future", false).unwrap();
        active.conn.execute_batch("PRAGMA user_version = 99").unwrap();
        drop(active);
        assert!(open(dir.path()).is_err());
    }

    #[test]
    fn settings_reject_unknown_keys_and_values() {
        let dir = tempfile::tempdir().unwrap();
        let active = create(dir.path(), "Settings", false).unwrap();
        assert!(set_setting(&active.conn, "theme", "neon").is_err());
        assert!(set_setting(&active.conn, "api_key", "x").is_err());
        set_setting(&active.conn, "theme", "dark").unwrap();
        assert_eq!(get_setting(&active.conn, "theme").unwrap().as_deref(), Some("dark"));
    }
}
