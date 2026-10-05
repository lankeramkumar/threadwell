//! Several workspaces per installation. Each workspace is its own folder and database, so pages
//! never mix between workspaces. This module keeps the list of known workspaces in the app's
//! config folder and provides switch, rename and forget. It also decides which workspaces an
//! assistant question covers, and opens the others read-only for retrieval.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{install, with_active, AppState};
use crate::error::{validation, AppError, AppResult};
use crate::workspace::{self, WorkspaceInfo};
use crate::{db, util};

const REGISTRY_FILE: &str = "workspaces.json";
pub const MAX_WORKSPACES: usize = 50;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListItem {
    pub name: String,
    pub path: String,
    pub active: bool,
    /// False when the folder or its database is missing now.
    pub available: bool,
}

pub fn load(config_dir: &Path) -> Vec<Entry> {
    fs::read_to_string(config_dir.join(REGISTRY_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(config_dir: &Path, entries: &[Entry]) -> AppResult<()> {
    fs::create_dir_all(config_dir)?;
    fs::write(config_dir.join(REGISTRY_FILE), serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

/// Adds a workspace or updates its name. Keeps the list bounded.
pub fn upsert(config_dir: &Path, name: &str, path: &str) -> AppResult<()> {
    let mut entries = load(config_dir);
    entries.retain(|e| e.path != path);
    entries.insert(0, Entry { name: name.to_string(), path: path.to_string() });
    entries.truncate(MAX_WORKSPACES);
    save(config_dir, &entries)
}

/// Removes a workspace from the list. Its folder and data are not touched.
pub fn forget(config_dir: &Path, path: &str) -> AppResult<()> {
    let mut entries = load(config_dir);
    entries.retain(|e| e.path != path);
    save(config_dir, &entries)
}

pub fn list(config_dir: &Path, active_path: Option<&str>) -> Vec<ListItem> {
    load(config_dir)
        .into_iter()
        .map(|e| ListItem {
            available: Path::new(&e.path).join(db::DB_FILE).is_file(),
            active: active_path == Some(e.path.as_str()),
            name: e.name,
            path: e.path,
        })
        .collect()
}

/// Which workspaces an assistant question covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The open workspace only.
    Current,
    /// Every available workspace, read-only.
    All,
    /// The named workspaces (by name or path), read-only unless one is the open workspace.
    Named(Vec<Entry>),
}

/// Chooses the scope. An explicit selection from the UI wins. Otherwise the message is searched
/// for "all workspaces" or for the name of any known workspace, so the user can say it in words.
pub fn scope_for(explicit: Option<&str>, message: &str, entries: &[Entry], active_path: &str) -> Scope {
    match explicit.map(str::trim) {
        Some("all") => return Scope::All,
        Some("" | "current") | None => {}
        Some(value) => {
            let picked: Vec<Entry> = entries.iter().filter(|e| e.path == value || e.name == value).cloned().collect();
            return if picked.is_empty() { Scope::Current } else { Scope::Named(picked) };
        }
    }
    let lower = message.to_lowercase();
    if lower.contains("all workspaces") || lower.contains("every workspace") {
        return Scope::All;
    }
    let named: Vec<Entry> = entries
        .iter()
        .filter(|e| mentions_word(&lower, &e.name.trim().to_lowercase()))
        .cloned()
        .collect();
    if named.is_empty() || named.iter().all(|e| e.path == active_path) {
        Scope::Current
    } else {
        Scope::Named(named)
    }
}

/// True when `name` appears in `text` as whole words, so "work" does not match "workspace".
fn mentions_word(text: &str, name: &str) -> bool {
    if name.chars().count() < 2 {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric();
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// Entries a scope covers. Unavailable workspaces are skipped.
pub fn scope_entries(scope: &Scope, entries: &[Entry]) -> Vec<Entry> {
    match scope {
        Scope::Current => Vec::new(),
        Scope::All => entries.iter().filter(|e| Path::new(&e.path).join(db::DB_FILE).is_file()).cloned().collect(),
        Scope::Named(named) => named.clone(),
    }
}

/// Opens another workspace's database read-only. Retrieval never writes to it.
pub fn open_read_only(path: &str) -> AppResult<Connection> {
    let db_path: PathBuf = Path::new(path).join(db::DB_FILE);
    if !db_path.is_file() {
        return Err(AppError::NotFound("Workspace".into()));
    }
    Ok(Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?)
}

/// The workspace id stored in a database, read without changing anything.
pub fn workspace_id(conn: &Connection) -> AppResult<String> {
    Ok(conn.query_row("SELECT id FROM workspace_meta LIMIT 1", [], |r| r.get(0))?)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn workspaces_list(state: State<'_, AppState>) -> AppResult<Vec<ListItem>> {
    let active_path = with_active(&state.active, |a| Ok(a.root.display().to_string())).ok();
    Ok(list(&state.config_dir, active_path.as_deref()))
}

/// Makes another workspace the open one. Runs in progress are cancelled first.
#[tauri::command]
pub async fn workspace_switch(state: State<'_, AppState>, path: String) -> AppResult<WorkspaceInfo> {
    let root = crate::util::validate_abs_path(&path)?;
    let active = workspace::open(&root)?;
    Ok(install(&state, active))
}

#[tauri::command]
pub async fn workspace_rename(state: State<'_, AppState>, name: String) -> AppResult<WorkspaceInfo> {
    let name = util::validate_line(&name, "Workspace name", 80)?;
    let info = with_active(&state.active, |a| {
        a.conn.execute("UPDATE workspace_meta SET name = ?1", params![name])?;
        let mut info = a.info.clone();
        info.name = name.clone();
        Ok(info)
    })?;
    upsert(&state.config_dir, &info.name, &info.path)?;
    with_active(&state.active, |a| {
        a.info.name = info.name.clone();
        Ok(())
    })?;
    Ok(info)
}

/// Removes a workspace from the list. The open workspace cannot be forgotten, and data is kept.
#[tauri::command]
pub async fn workspace_forget(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let open_path = with_active(&state.active, |a| Ok(a.root.display().to_string())).ok();
    if open_path.as_deref() == Some(path.as_str()) {
        return validation("Switch to another workspace before forgetting this one");
    }
    forget(&state.config_dir, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<Entry> {
        vec![
            Entry { name: "Work".into(), path: "C:/w".into() },
            Entry { name: "Home".into(), path: "C:/h".into() },
        ]
    }

    #[test]
    fn explicit_scope_wins_over_message_text() {
        assert_eq!(scope_for(Some("all"), "about Home", &entries(), "C:/w"), Scope::All);
        assert_eq!(scope_for(Some("Home"), "about work", &entries(), "C:/w"), Scope::Named(vec![entries()[1].clone()]));
        assert_eq!(scope_for(Some("current"), "about Home", &entries(), "C:/w"), Scope::Named(vec![entries()[1].clone()]));
    }

    #[test]
    fn workspace_names_in_the_message_select_the_scope() {
        assert_eq!(scope_for(None, "What is in my Home workspace?", &entries(), "C:/w"), Scope::Named(vec![entries()[1].clone()]));
        assert_eq!(scope_for(None, "Search all workspaces for passkeys", &entries(), "C:/w"), Scope::All);
        assert_eq!(scope_for(None, "What did we decide?", &entries(), "C:/w"), Scope::Current);
    }

    #[test]
    fn naming_the_open_workspace_stays_current() {
        assert_eq!(scope_for(None, "In Work, what is due?", &entries(), "C:/w"), Scope::Current);
    }

    #[test]
    fn registry_round_trips_and_forgets_without_touching_data() {
        let dir = tempfile::tempdir().unwrap();
        upsert(dir.path(), "Work", "C:/w").unwrap();
        upsert(dir.path(), "Home", "C:/h").unwrap();
        upsert(dir.path(), "Work renamed", "C:/w").unwrap();
        let loaded = load(dir.path());
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Work renamed", "the newest entry comes first");
        forget(dir.path(), "C:/h").unwrap();
        assert_eq!(load(dir.path()).len(), 1);
    }

    #[test]
    fn two_workspaces_keep_their_pages_apart_and_search_reads_each_one() {
        let base = tempfile::tempdir().unwrap();
        let mut made = Vec::new();
        for (name, text) in [("Work", "passkeys were chosen for work"), ("Home", "fingerprint unlock at home")] {
            let root = base.path().join(name);
            let active = workspace::create(&root, name, false).unwrap();
            let page = crate::pages::create(&active.conn, &active.info.id, &format!("{name} note"), None).unwrap();
            crate::pages::update(&active.conn, &active.info.id, &page.id, &format!("{name} note"), &crate::markdown::from_markdown(text), page.revision).unwrap();
            made.push((root.display().to_string(), active.info.id.clone(), page.id.clone()));
        }
        let (work_path, work_ws, _) = &made[0];
        let (home_path, home_ws, _) = &made[1];
        let work = open_read_only(work_path).unwrap();
        let home = open_read_only(home_path).unwrap();
        assert_eq!(&workspace_id(&work).unwrap(), work_ws);
        assert_eq!(&workspace_id(&home).unwrap(), home_ws);
        let weights = crate::knowledge::Weights { lexical: 1.0, vector: 0.0 };
        let in_work = crate::knowledge::retrieve(&work, work_ws, "passkeys", None, "m", crate::knowledge::Mode::Lexical, weights, 5).unwrap();
        let in_home = crate::knowledge::retrieve(&home, home_ws, "passkeys", None, "m", crate::knowledge::Mode::Lexical, weights, 5).unwrap();
        assert_eq!(in_work.len(), 1);
        assert!(in_home.is_empty(), "the home workspace never sees work pages");
    }
}
