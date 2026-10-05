//! Tauri command surface. Each command validates its inputs, runs against the open
//! workspace under a mutex, and returns serializable data or a typed error.
//!
//! Commands are `async` so SQLite work runs on the async runtime, not the UI thread.
//! Locks are never held across `.await` points.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::error::{validation, AppError, AppResult};
use crate::pages::{self, Page, PageSummary};
use crate::search::{self, SearchHit};
use crate::tasks::{self, NewTask, Project, Task, TaskPatch};
use crate::transfer::{self, BackupInfo};
use crate::util::validate_abs_path;
use crate::workspace::{self, Active, WorkspaceInfo};

const LAST_WORKSPACE_FILE: &str = "last-workspace.txt";

/// Shared application state. The workspace sits behind an `Arc` so AI worker threads can
/// take short locks for tool calls. Locks are never held while waiting on a model.
pub struct AppState {
    pub active: Arc<Mutex<Option<Active>>>,
    pub config_dir: PathBuf,
    pub runs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl AppState {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            config_dir,
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Cancels every in-flight AI run. Used when the workspace changes so runs cannot
    /// write into a different workspace.
    pub fn cancel_all_runs(&self) {
        if let Ok(runs) = self.runs.lock() {
            for flag in runs.values() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

pub fn with_active<T>(
    active: &Mutex<Option<Active>>,
    f: impl FnOnce(&mut Active) -> AppResult<T>,
) -> AppResult<T> {
    let mut guard = active
        .lock()
        .map_err(|_| AppError::Validation("Internal state error. Restart Threadwell.".into()))?;
    let active = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    f(active)
}

fn with_workspace<T>(state: &AppState, f: impl FnOnce(&mut Active) -> AppResult<T>) -> AppResult<T> {
    with_active(&state.active, f)
}

fn remember(state: &AppState, root: &Path) {
    // Failure to remember the workspace is not fatal; the user can open it again.
    let _ = fs::create_dir_all(&state.config_dir)
        .and_then(|_| fs::write(state.config_dir.join(LAST_WORKSPACE_FILE), root.display().to_string()));
}

fn install(state: &AppState, active: Active) -> WorkspaceInfo {
    state.cancel_all_runs();
    remember(state, &active.root);
    let info = active.info.clone();
    if let Ok(mut guard) = state.active.lock() {
        *guard = Some(active);
    }
    info
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub workspace: Option<WorkspaceInfo>,
}

/// Reports the open workspace. On first call after launch, reopens the last workspace
/// if it still exists. A workspace that fails to open is forgotten, not deleted.
#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> AppResult<AppStatus> {
    if let Some(info) = with_workspace(&state, |a| Ok(a.info.clone())).ok() {
        return Ok(AppStatus { workspace: Some(info) });
    }
    let last = fs::read_to_string(state.config_dir.join(LAST_WORKSPACE_FILE)).ok();
    if let Some(path) = last.map(|p| PathBuf::from(p.trim())).filter(|p| p.is_absolute()) {
        if let Ok(active) = workspace::open(&path) {
            return Ok(AppStatus { workspace: Some(install(&state, active)) });
        }
    }
    Ok(AppStatus { workspace: None })
}

#[tauri::command]
pub async fn create_workspace(
    state: State<'_, AppState>,
    path: String,
    name: String,
    with_sample: bool,
) -> AppResult<WorkspaceInfo> {
    let root = validate_abs_path(&path)?;
    let active = workspace::create(&root, &name, with_sample)?;
    Ok(install(&state, active))
}

#[tauri::command]
pub async fn open_workspace(state: State<'_, AppState>, path: String) -> AppResult<WorkspaceInfo> {
    let root = validate_abs_path(&path)?;
    let active = workspace::open(&root)?;
    Ok(install(&state, active))
}

#[tauri::command]
pub async fn list_pages(state: State<'_, AppState>) -> AppResult<Vec<PageSummary>> {
    with_workspace(&state, |a| pages::list(&a.conn, &a.info.id))
}

#[tauri::command]
pub async fn list_trash(state: State<'_, AppState>) -> AppResult<Vec<PageSummary>> {
    with_workspace(&state, |a| pages::list_trash(&a.conn, &a.info.id))
}

#[tauri::command]
pub async fn get_page(state: State<'_, AppState>, id: String) -> AppResult<Page> {
    with_workspace(&state, |a| pages::get(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn create_page(state: State<'_, AppState>, title: String, parent_id: Option<String>) -> AppResult<Page> {
    with_workspace(&state, |a| pages::create(&a.conn, &a.info.id, &title, parent_id.as_deref()))
}

#[tauri::command]
pub async fn save_page(
    state: State<'_, AppState>,
    id: String,
    title: String,
    body: Value,
    expected_revision: i64,
) -> AppResult<Page> {
    with_workspace(&state, |a| pages::update(&a.conn, &a.info.id, &id, &title, &body, expected_revision))
}

#[tauri::command]
pub async fn move_page(state: State<'_, AppState>, id: String, parent_id: Option<String>) -> AppResult<()> {
    with_workspace(&state, |a| pages::move_to(&a.conn, &a.info.id, &id, parent_id.as_deref()))
}

#[tauri::command]
pub async fn set_page_favorite(state: State<'_, AppState>, id: String, favorite: bool) -> AppResult<()> {
    with_workspace(&state, |a| pages::set_favorite(&a.conn, &a.info.id, &id, favorite))
}

#[tauri::command]
pub async fn trash_page(state: State<'_, AppState>, id: String) -> AppResult<usize> {
    with_workspace(&state, |a| pages::trash(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn restore_page(state: State<'_, AppState>, id: String) -> AppResult<()> {
    with_workspace(&state, |a| pages::restore(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn backlinks(state: State<'_, AppState>, id: String) -> AppResult<Vec<PageSummary>> {
    with_workspace(&state, |a| pages::backlinks(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    with_workspace(&state, |a| tasks::list_projects(&a.conn, &a.info.id))
}

#[tauri::command]
pub async fn create_project(state: State<'_, AppState>, name: String) -> AppResult<Project> {
    with_workspace(&state, |a| tasks::create_project(&a.conn, &a.info.id, &name))
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>, project_id: Option<String>) -> AppResult<Vec<Task>> {
    with_workspace(&state, |a| tasks::list_tasks(&a.conn, &a.info.id, project_id.as_deref()))
}

#[tauri::command]
pub async fn create_task(state: State<'_, AppState>, input: NewTask) -> AppResult<Task> {
    with_workspace(&state, |a| tasks::create_task(&a.conn, &a.info.id, input))
}

#[tauri::command]
pub async fn update_task(
    state: State<'_, AppState>,
    id: String,
    patch: TaskPatch,
    expected_revision: i64,
) -> AppResult<Task> {
    with_workspace(&state, |a| tasks::update_task(&a.conn, &a.info.id, &id, patch, expected_revision))
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: String) -> AppResult<()> {
    with_workspace(&state, |a| tasks::delete_task(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn search_workspace(state: State<'_, AppState>, query: String) -> AppResult<Vec<SearchHit>> {
    with_workspace(&state, |a| search::search(&a.conn, &a.info.id, &query))
}

#[tauri::command]
pub async fn rebuild_search_index(state: State<'_, AppState>) -> AppResult<usize> {
    with_workspace(&state, |a| search::rebuild(&a.conn, &a.info.id))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    with_workspace(&state, |a| {
        let theme = workspace::get_setting(&a.conn, "theme")?.unwrap_or_else(|| "system".into());
        Ok(Settings { theme })
    })
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: String) -> AppResult<()> {
    with_workspace(&state, |a| workspace::set_setting(&a.conn, &key, &value))
}

#[tauri::command]
pub async fn export_page_markdown(state: State<'_, AppState>, id: String, dest_dir: String) -> AppResult<String> {
    let dest = validate_abs_path(&dest_dir)?;
    with_workspace(&state, |a| {
        let written = transfer::export_page_markdown(&a.conn, &a.info.id, &a.root, &id, &dest)?;
        Ok(written.display().to_string())
    })
}

#[tauri::command]
pub async fn export_all_markdown(state: State<'_, AppState>, dest_dir: String) -> AppResult<usize> {
    let dest = validate_abs_path(&dest_dir)?;
    with_workspace(&state, |a| transfer::export_all_markdown(&a.conn, &a.info.id, &a.root, &dest))
}

#[tauri::command]
pub async fn export_tasks_csv(state: State<'_, AppState>, dest_file: String) -> AppResult<usize> {
    let dest = validate_abs_path(&dest_file)?;
    with_workspace(&state, |a| transfer::export_tasks_csv(&a.conn, &a.info.id, &a.root, &dest))
}

#[tauri::command]
pub async fn import_markdown(
    state: State<'_, AppState>,
    src_path: String,
    parent_id: Option<String>,
) -> AppResult<Page> {
    let src = validate_abs_path(&src_path)?;
    with_workspace(&state, |a| transfer::import_markdown(&a.conn, &a.info.id, &src, parent_id.as_deref()))
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>, dest_dir: String) -> AppResult<BackupInfo> {
    let dest = validate_abs_path(&dest_dir)?;
    with_workspace(&state, |a| transfer::create_backup(&a.conn, &a.root, &dest))
}

/// Restores a backup into a new folder and makes it the open workspace. The previously
/// open workspace is closed; its files are not modified.
#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    backup_dir: String,
    dest_dir: String,
) -> AppResult<WorkspaceInfo> {
    let backup = validate_abs_path(&backup_dir)?;
    let dest = validate_abs_path(&dest_dir)?;
    if dest.exists() && !dest.is_dir() {
        return validation("Restore destination must be a folder");
    }
    transfer::restore_backup(&backup, &dest)?;
    let active = workspace::open(&dest)?;
    Ok(install(&state, active))
}
