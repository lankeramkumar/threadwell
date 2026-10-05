//! Change proposals. Every AI-originated write is stored here first and waits for a user
//! decision. Apply is transactional and checks revisions, so a suggestion based on content
//! that has since changed is marked stale rather than overwriting it. Applied proposals
//! keep enough state to undo, and undo refuses if the content changed afterwards.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::db::Tx;
use crate::error::{validation, AppError, AppResult};
use crate::markdown;
use crate::pages;
use crate::tasks::{self, NewTask, TaskPatch};
use crate::util;

const MAX_SUMMARY_CHARS: usize = 300;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub run_id: Option<String>,
    pub kind: String,
    pub target_page_id: Option<String>,
    pub base_revision: Option<i64>,
    pub status: String,
    pub summary: String,
    pub diff_text: String,
    pub applied_revision: Option<i64>,
    pub created_at: String,
    pub decided_at: Option<String>,
}

struct Row {
    proposal: Proposal,
    payload_json: String,
    undo_json: Option<String>,
}

fn load(conn: &Connection, ws: &str, id: &str) -> AppResult<Row> {
    util::validate_id(id)?;
    conn.query_row(
        "SELECT id, run_id, kind, target_page_id, base_revision, status, summary, diff_text,
                applied_revision, created_at, decided_at, payload_json, undo_json
         FROM change_proposals WHERE id = ?1 AND workspace_id = ?2",
        params![id, ws],
        |row| {
            Ok(Row {
                proposal: Proposal {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    kind: row.get(2)?,
                    target_page_id: row.get(3)?,
                    base_revision: row.get(4)?,
                    status: row.get(5)?,
                    summary: row.get(6)?,
                    diff_text: row.get(7)?,
                    applied_revision: row.get(8)?,
                    created_at: row.get(9)?,
                    decided_at: row.get(10)?,
                },
                payload_json: row.get(11)?,
                undo_json: row.get(12)?,
            })
        },
    )
    .optional()?
    .ok_or(AppError::NotFound("Suggestion".into()))
}

pub fn get(conn: &Connection, ws: &str, id: &str) -> AppResult<Proposal> {
    Ok(load(conn, ws, id)?.proposal)
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    ws: &str,
    run_id: Option<&str>,
    kind: &str,
    target_page_id: Option<&str>,
    base_revision: Option<i64>,
    payload: &Value,
    summary: &str,
    diff_text: &str,
) -> AppResult<Proposal> {
    if summary.chars().count() > MAX_SUMMARY_CHARS {
        return validation("Suggestion summary is too long");
    }
    let id = util::new_id();
    let now = util::now();
    conn.execute(
        "INSERT INTO change_proposals (id, workspace_id, run_id, kind, target_page_id, base_revision, payload_json,
                                       diff_text, summary, status, idempotency_key, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10, ?11)",
        params![
            id,
            ws,
            run_id,
            kind,
            target_page_id,
            base_revision,
            serde_json::to_string(payload)?,
            diff_text,
            summary,
            format!("proposal:{id}"),
            now
        ],
    )?;
    get(conn, ws, &id)
}

pub fn list(conn: &Connection, ws: &str, run_id: Option<&str>) -> AppResult<Vec<Proposal>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM change_proposals
         WHERE workspace_id = ?1 AND (?2 IS NULL OR run_id = ?2)
         ORDER BY created_at DESC LIMIT 100",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![ws, run_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    ids.iter().map(|id| get(conn, ws, id)).collect()
}

fn set_status(conn: &Connection, id: &str, status: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE change_proposals SET status = ?1, decided_at = ?2 WHERE id = ?3",
        params![status, util::now(), id],
    )?;
    Ok(())
}

pub fn reject(conn: &Connection, ws: &str, id: &str) -> AppResult<Proposal> {
    let row = load(conn, ws, id)?;
    if row.proposal.status != "pending" {
        return validation("Only pending suggestions can be rejected");
    }
    set_status(conn, id, "rejected")?;
    get(conn, ws, id)
}

pub fn apply(conn: &Connection, ws: &str, id: &str) -> AppResult<Proposal> {
    let row = load(conn, ws, id)?;
    match row.proposal.status.as_str() {
        // Applying twice returns the first result instead of writing again.
        "applied" => return Ok(row.proposal),
        "pending" => {}
        _ => return validation("This suggestion is no longer pending. Ask for a new one."),
    }
    let payload: Value = serde_json::from_str(&row.payload_json)?;

    let tx = Tx::begin(conn)?;
    let outcome = match row.proposal.kind.as_str() {
        "create_page" => apply_create(&tx, ws, &payload),
        "edit_page" => apply_edit(&tx, ws, &row.proposal, &payload),
        "task_changes" => apply_tasks(&tx, ws, &payload),
        _ => validation("Unknown suggestion type"),
    };
    match outcome {
        Ok((applied_revision, undo)) => {
            let changed = tx.execute(
                "UPDATE change_proposals SET status = 'applied', applied_revision = ?1, undo_json = ?2, decided_at = ?3
                 WHERE id = ?4 AND status = 'pending'",
                params![applied_revision, undo.to_string(), util::now(), id],
            )?;
            if changed != 1 {
                return validation("This suggestion was changed by another action. Refresh and try again.");
            }
            tx.commit()?;
            get(conn, ws, id)
        }
        Err(error) => {
            drop(tx);
            if matches!(error, AppError::Conflict(_)) {
                set_status(conn, id, "stale")?;
            }
            Err(error)
        }
    }
}

fn apply_create(conn: &Connection, ws: &str, payload: &Value) -> AppResult<(Option<i64>, Value)> {
    let title = payload_str(payload, "title")?;
    let markdown_text = payload_str(payload, "markdown")?;
    let parent = payload.get("parentId").and_then(Value::as_str);
    let page = pages::create(conn, ws, title, parent)?;
    let body = markdown::from_markdown(markdown_text);
    let saved = pages::update(conn, ws, &page.id, title, &body, page.revision)?;
    Ok((Some(saved.revision), json!({ "kind": "create", "pageId": page.id })))
}

fn apply_edit(conn: &Connection, ws: &str, proposal: &Proposal, payload: &Value) -> AppResult<(Option<i64>, Value)> {
    let page_id = proposal
        .target_page_id
        .clone()
        .ok_or_else(|| AppError::Validation("Suggestion has no target page".into()))?;
    let current = pages::get(conn, ws, &page_id)?;
    if Some(current.revision) != proposal.base_revision {
        return Err(AppError::Conflict(
            "The page changed after this suggestion was made. Ask for a new suggestion.".into(),
        ));
    }
    let title = payload.get("title").and_then(Value::as_str).unwrap_or(&current.title).to_string();
    let body = markdown::from_markdown(payload_str(payload, "markdown")?);
    let saved = pages::update(conn, ws, &page_id, &title, &body, current.revision)?;
    let undo = json!({
        "kind": "edit",
        "pageId": page_id,
        "prevTitle": current.title,
        "prevBody": current.body,
    });
    Ok((Some(saved.revision), undo))
}

fn apply_tasks(conn: &Connection, ws: &str, payload: &Value) -> AppResult<(Option<i64>, Value)> {
    let changes = payload
        .get("changes")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("Suggestion has no changes".into()))?;
    let mut undo_items = Vec::new();
    for change in changes {
        match change.get("op").and_then(Value::as_str) {
            Some("create") => {
                let input: NewTask = serde_json::from_value(change.clone())?;
                let task = tasks::create_task(conn, ws, input)?;
                undo_items.push(json!({ "op": "create", "taskId": task.id, "appliedRevision": task.revision }));
            }
            Some("update") => {
                let id = payload_str(change, "id")?.to_string();
                let expected = change
                    .get("expectedRevision")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| AppError::Validation("Update is missing its revision".into()))?;
                let prev = tasks::get_task(conn, ws, &id)?;
                let patch: TaskPatch = serde_json::from_value(change.clone())?;
                let updated = tasks::update_task(conn, ws, &id, patch, expected)?;
                undo_items.push(json!({
                    "op": "update",
                    "taskId": id,
                    "appliedRevision": updated.revision,
                    "prev": {
                        "title": prev.title,
                        "description": prev.description,
                        "status": prev.status,
                        "priority": prev.priority,
                        "dueDate": prev.due_date.unwrap_or_default(),
                        "projectId": prev.project_id.unwrap_or_default(),
                    }
                }));
            }
            _ => return validation("Unknown task change"),
        }
    }
    Ok((None, json!({ "kind": "tasks", "items": undo_items })))
}

pub fn undo(conn: &Connection, ws: &str, id: &str) -> AppResult<Proposal> {
    let row = load(conn, ws, id)?;
    if row.proposal.status != "applied" {
        return validation("Only applied suggestions can be undone");
    }
    let undo: Value = serde_json::from_str(row.undo_json.as_deref().unwrap_or("{}"))?;
    let tx = Tx::begin(conn)?;
    match row.proposal.kind.as_str() {
        "create_page" => {
            let page_id = payload_str(&undo, "pageId")?;
            let page = pages::get(&tx, ws, page_id)
                .map_err(|_| AppError::Conflict("The created page was already removed.".into()))?;
            if Some(page.revision) != row.proposal.applied_revision {
                return Err(AppError::Conflict(
                    "This page changed after it was created. Review it in the editor instead.".into(),
                ));
            }
            pages::trash(&tx, ws, page_id)?;
        }
        "edit_page" => {
            let page_id = payload_str(&undo, "pageId")?;
            let current = pages::get(&tx, ws, page_id)?;
            if Some(current.revision) != row.proposal.applied_revision {
                return Err(AppError::Conflict(
                    "The page changed after this suggestion was applied. Restore it by hand if needed.".into(),
                ));
            }
            let prev_body: Value = undo.get("prevBody").cloned().unwrap_or_else(markdown::empty_doc);
            let prev_title = payload_str(&undo, "prevTitle")?;
            pages::update(&tx, ws, page_id, prev_title, &prev_body, current.revision)?;
        }
        "task_changes" => {
            let items = undo.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
            for item in items.iter().rev() {
                let task_id = payload_str(item, "taskId")?;
                let current = tasks::get_task(&tx, ws, task_id)
                    .map_err(|_| AppError::Conflict("A task from this suggestion was already removed.".into()))?;
                let applied = item.get("appliedRevision").and_then(Value::as_i64);
                if Some(current.revision) != applied {
                    return Err(AppError::Conflict(
                        "A task from this suggestion changed afterwards. Review it in the task view.".into(),
                    ));
                }
                if item.get("op").and_then(Value::as_str) == Some("create") {
                    tasks::delete_task(&tx, ws, task_id)?;
                } else {
                    let prev = item.get("prev").cloned().unwrap_or(Value::Null);
                    let text = |key: &str| prev.get(key).and_then(Value::as_str).unwrap_or("").to_string();
                    let patch = TaskPatch {
                        title: Some(text("title")),
                        description: Some(text("description")),
                        status: Some(text("status")),
                        priority: Some(text("priority")),
                        due_date: Some(text("dueDate")),
                        project_id: Some(text("projectId")),
                    };
                    tasks::update_task(&tx, ws, task_id, patch, current.revision)?;
                }
            }
        }
        _ => return validation("Unknown suggestion type"),
    }
    tx.execute(
        "UPDATE change_proposals SET status = 'undone', decided_at = ?1 WHERE id = ?2",
        params![util::now(), id],
    )?;
    tx.commit()?;
    get(conn, ws, id)
}

fn payload_str<'a>(value: &'a Value, key: &str) -> AppResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation(format!("Suggestion is missing {key}")))
}
