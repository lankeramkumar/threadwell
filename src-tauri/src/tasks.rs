//! Projects group tasks. A task has title, description, status, priority, an optional
//! due date (never inferred), an optional project, and an optional source page.
//!
//! Task edits use the same revision check as pages, so an edit based on an old
//! view is rejected instead of silently overwriting.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{validation, AppError, AppResult};
use crate::util;

pub const STATUSES: &[&str] = &["todo", "doing", "done"];
pub const PRIORITIES: &[&str] = &["low", "medium", "high"];
const MAX_TITLE_CHARS: usize = 200;
const MAX_DESCRIPTION_CHARS: usize = 20_000;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub source_page_id: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub project_id: Option<String>,
    pub source_page_id: Option<String>,
}

/// Partial update. For `dueDate` and `projectId`, an empty string clears the field.
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub project_id: Option<String>,
}

pub fn validate_status(value: &str) -> AppResult<&str> {
    if STATUSES.contains(&value) {
        Ok(value)
    } else {
        validation("Status must be todo, doing or done")
    }
}

pub fn validate_priority(value: &str) -> AppResult<&str> {
    if PRIORITIES.contains(&value) {
        Ok(value)
    } else {
        validation("Priority must be low, medium or high")
    }
}

/// Accepts only real calendar dates in YYYY-MM-DD form.
pub fn validate_due_date(value: &str) -> AppResult<String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .or_else(|_| validation("Due date must be a real date in YYYY-MM-DD format"))
}

fn validate_description(value: &str) -> AppResult<String> {
    if value.chars().count() > MAX_DESCRIPTION_CHARS {
        return validation("Description is too long");
    }
    Ok(value.to_string())
}

fn ensure_project(conn: &Connection, ws: &str, project_id: &str) -> AppResult<()> {
    util::validate_id(project_id)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE id = ?1 AND workspace_id = ?2",
        params![project_id, ws],
        |row| row.get(0),
    )?;
    if count == 0 {
        return Err(AppError::NotFound("Project".into()));
    }
    Ok(())
}

fn ensure_live_page(conn: &Connection, ws: &str, page_id: &str) -> AppResult<()> {
    util::validate_id(page_id)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pages WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL",
        params![page_id, ws],
        |row| row.get(0),
    )?;
    if count == 0 {
        return Err(AppError::NotFound("Source page".into()));
    }
    Ok(())
}

pub fn list_projects(conn: &Connection, ws: &str) -> AppResult<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM projects
         WHERE workspace_id = ?1 ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![ws], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn create_project(conn: &Connection, ws: &str, name: &str) -> AppResult<Project> {
    let name = util::validate_line(name, "Project name", 120)?;
    let id = util::new_id();
    let now = util::now();
    conn.execute(
        "INSERT INTO projects (id, workspace_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, ws, name, now],
    )?;
    Ok(Project { id, name, created_at: now.clone(), updated_at: now })
}

const TASK_COLUMNS: &str = "id, project_id, title, description, status, priority, due_date, source_page_id, revision, created_at, updated_at";

fn task_row(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        priority: row.get(5)?,
        due_date: row.get(6)?,
        source_page_id: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn list_tasks(conn: &Connection, ws: &str, project_id: Option<&str>) -> AppResult<Vec<Task>> {
    if let Some(project_id) = project_id {
        ensure_project(conn, ws, project_id)?;
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks
         WHERE workspace_id = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR project_id = ?2)
         ORDER BY CASE status WHEN 'todo' THEN 0 WHEN 'doing' THEN 1 ELSE 2 END, position, created_at"
    ))?;
    let rows = stmt.query_map(params![ws, project_id], task_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_task(conn: &Connection, ws: &str, id: &str) -> AppResult<Task> {
    util::validate_id(id)?;
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL"),
        params![id, ws],
        task_row,
    )
    .optional()?
    .ok_or(AppError::NotFound("Task".into()))
}

pub fn create_task(conn: &Connection, ws: &str, input: NewTask) -> AppResult<Task> {
    let title = util::validate_line(&input.title, "Task title", MAX_TITLE_CHARS)?;
    let description = validate_description(input.description.as_deref().unwrap_or(""))?;
    let status = validate_status(input.status.as_deref().unwrap_or("todo"))?.to_string();
    let priority = validate_priority(input.priority.as_deref().unwrap_or("medium"))?.to_string();
    let due_date = match input.due_date.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => Some(validate_due_date(d)?),
        None => None,
    };
    let project_id = input.project_id.filter(|p| !p.is_empty());
    if let Some(project) = &project_id {
        ensure_project(conn, ws, project)?;
    }
    let source_page_id = input.source_page_id.filter(|p| !p.is_empty());
    if let Some(page) = &source_page_id {
        ensure_live_page(conn, ws, page)?;
    }
    let id = util::new_id();
    let now = util::now();
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE workspace_id = ?1",
        params![ws],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks (id, workspace_id, project_id, title, description, status, priority, due_date,
                            source_page_id, revision, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11, ?11)",
        params![id, ws, project_id, title, description, status, priority, due_date, source_page_id, position, now],
    )?;
    get_task(conn, ws, &id)
}

pub fn update_task(
    conn: &Connection,
    ws: &str,
    id: &str,
    patch: TaskPatch,
    expected_revision: i64,
) -> AppResult<Task> {
    let current = get_task(conn, ws, id)?;
    if current.revision != expected_revision {
        return Err(AppError::Conflict(
            "This task changed elsewhere. Refresh the list and try again.".into(),
        ));
    }
    let title = match patch.title {
        Some(t) => util::validate_line(&t, "Task title", MAX_TITLE_CHARS)?,
        None => current.title,
    };
    let description = match patch.description {
        Some(d) => validate_description(&d)?,
        None => current.description,
    };
    let status = match patch.status {
        Some(s) => validate_status(&s)?.to_string(),
        None => current.status,
    };
    let priority = match patch.priority {
        Some(p) => validate_priority(&p)?.to_string(),
        None => current.priority,
    };
    let due_date = match patch.due_date {
        Some(d) if d.is_empty() => None,
        Some(d) => Some(validate_due_date(&d)?),
        None => current.due_date,
    };
    let project_id = match patch.project_id {
        Some(p) if p.is_empty() => None,
        Some(p) => {
            ensure_project(conn, ws, &p)?;
            Some(p)
        }
        None => current.project_id,
    };
    conn.execute(
        "UPDATE tasks SET title = ?1, description = ?2, status = ?3, priority = ?4, due_date = ?5,
                          project_id = ?6, revision = revision + 1, updated_at = ?7
         WHERE id = ?8",
        params![title, description, status, priority, due_date, project_id, util::now(), id],
    )?;
    get_task(conn, ws, id)
}

pub fn delete_task(conn: &Connection, ws: &str, id: &str) -> AppResult<()> {
    get_task(conn, ws, id)?;
    conn.execute(
        "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2",
        params![util::now(), id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, pages};

    fn setup() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("t.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute(
            "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'W', ?2)",
            params![ws, util::now()],
        )
        .unwrap();
        (dir, conn, ws)
    }

    #[test]
    fn unknown_due_date_stays_unset() {
        let (_d, conn, ws) = setup();
        let task = create_task(&conn, &ws, NewTask { title: "Ship".into(), ..Default::default() }).unwrap();
        assert_eq!(task.due_date, None);
        assert_eq!(task.status, "todo");
        assert_eq!(task.priority, "medium");
    }

    #[test]
    fn rejects_invalid_enum_and_date_values() {
        let (_d, conn, ws) = setup();
        let bad_status = NewTask { title: "x".into(), status: Some("blocked".into()), ..Default::default() };
        assert!(create_task(&conn, &ws, bad_status).is_err());
        let bad_date = NewTask { title: "x".into(), due_date: Some("2026-02-30".into()), ..Default::default() };
        assert!(create_task(&conn, &ws, bad_date).is_err());
        let good_date = NewTask { title: "x".into(), due_date: Some("2026-02-28".into()), ..Default::default() };
        assert_eq!(create_task(&conn, &ws, good_date).unwrap().due_date.as_deref(), Some("2026-02-28"));
    }

    #[test]
    fn stale_task_edit_is_rejected() {
        let (_d, conn, ws) = setup();
        let task = create_task(&conn, &ws, NewTask { title: "A".into(), ..Default::default() }).unwrap();
        let first = TaskPatch { status: Some("doing".into()), ..Default::default() };
        update_task(&conn, &ws, &task.id, first, 1).unwrap();
        let stale = TaskPatch { status: Some("done".into()), ..Default::default() };
        assert!(matches!(update_task(&conn, &ws, &task.id, stale, 1), Err(AppError::Conflict(_))));
        assert_eq!(get_task(&conn, &ws, &task.id).unwrap().status, "doing");
    }

    #[test]
    fn empty_string_clears_optional_fields() {
        let (_d, conn, ws) = setup();
        let project = create_project(&conn, &ws, "Atlas").unwrap();
        let task = create_task(
            &conn,
            &ws,
            NewTask {
                title: "T".into(),
                due_date: Some("2027-01-01".into()),
                project_id: Some(project.id.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let patch = TaskPatch { due_date: Some(String::new()), project_id: Some(String::new()), ..Default::default() };
        let cleared = update_task(&conn, &ws, &task.id, patch, task.revision).unwrap();
        assert_eq!(cleared.due_date, None);
        assert_eq!(cleared.project_id, None);
    }

    #[test]
    fn source_page_must_be_live_and_in_workspace() {
        let (_d, conn, ws) = setup();
        let page = pages::create(&conn, &ws, "Notes", None).unwrap();
        pages::trash(&conn, &ws, &page.id).unwrap();
        let input = NewTask { title: "x".into(), source_page_id: Some(page.id), ..Default::default() };
        assert!(create_task(&conn, &ws, input).is_err());
    }

    #[test]
    fn deleted_tasks_disappear_from_lists() {
        let (_d, conn, ws) = setup();
        let task = create_task(&conn, &ws, NewTask { title: "gone".into(), ..Default::default() }).unwrap();
        delete_task(&conn, &ws, &task.id).unwrap();
        assert!(list_tasks(&conn, &ws, None).unwrap().is_empty());
    }

    #[test]
    fn project_filter_limits_results() {
        let (_d, conn, ws) = setup();
        let project = create_project(&conn, &ws, "Atlas").unwrap();
        create_task(&conn, &ws, NewTask { title: "in".into(), project_id: Some(project.id.clone()), ..Default::default() }).unwrap();
        create_task(&conn, &ws, NewTask { title: "out".into(), ..Default::default() }).unwrap();
        assert_eq!(list_tasks(&conn, &ws, Some(&project.id)).unwrap().len(), 1);
        assert_eq!(list_tasks(&conn, &ws, None).unwrap().len(), 2);
    }
}
