//! Tools the model may call. Five are read-only; the three `propose_*` tools record a
//! suggestion for review and never write content. There is deliberately no delete tool, no
//! SQL, and no shell. Arguments are strict (`deny_unknown_fields`), and failures return a
//! short error to the model so it can correct itself, up to a fixed limit.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::diff::{change_counts, line_diff};
use super::proposals::{self, Proposal};
use crate::markdown;
use crate::pages;
use crate::search;
use crate::tasks;
use crate::util;

pub const MAX_TOOL_TEXT_CHARS: usize = 6_000;
const MAX_PAGE_TEXT_CHARS: usize = 4_000;
const MAX_SEARCH_RESULTS: usize = 8;
const MAX_TASK_RESULTS: usize = 40;
const MAX_PROPOSAL_MARKDOWN_CHARS: usize = 60_000;
const MAX_TASK_CHANGES: usize = 50;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// "page" or "task".
    pub kind: String,
    pub id: String,
    pub title: String,
}

impl Source {
    /// The token the model must use to cite this source, e.g. `page:<uuid>`.
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }
}

pub struct ToolOutput {
    pub ok: bool,
    /// Text returned to the model. Page and task content is wrapped as untrusted data.
    pub model_text: String,
    pub summary: String,
    pub category: Option<&'static str>,
    pub sources: Vec<Source>,
    pub proposal: Option<Proposal>,
}

fn failure(category: &'static str, message: impl Into<String>) -> ToolOutput {
    let message = message.into();
    ToolOutput {
        ok: false,
        model_text: format!("Error: {message}. Check the arguments against the tool schema and try again."),
        summary: message,
        category: Some(category),
        sources: Vec::new(),
        proposal: None,
    }
}

fn success(model_text: String, summary: String, sources: Vec<Source>) -> ToolOutput {
    ToolOutput { ok: true, model_text, summary, category: None, sources, proposal: None }
}

pub fn schemas() -> Value {
    json!([
        tool(
            "search_workspace",
            "Search pages and tasks in this workspace. Returns source ids to cite.",
            json!({ "query": { "type": "string", "description": "Words to search for" } }),
            &["query"]
        ),
        tool(
            "read_page",
            "Read the text of one page by its id.",
            json!({ "page_id": { "type": "string", "description": "Page id from search results" } }),
            &["page_id"]
        ),
        tool(
            "list_tasks",
            "List open or all tasks, optionally in one project or status.",
            json!({
                "project_id": { "type": "string" },
                "status": { "type": "string", "enum": ["todo", "doing", "done"] }
            }),
            &[]
        ),
        tool(
            "propose_create_page",
            "Suggest a new page. The user must approve it; nothing is created now.",
            json!({
                "title": { "type": "string" },
                "markdown": { "type": "string", "description": "Page body in Markdown" },
                "parent_id": { "type": "string" }
            }),
            &["title", "markdown"]
        ),
        tool(
            "propose_edit_page",
            "Suggest replacing a page's body. The user must approve it; nothing changes now.",
            json!({
                "page_id": { "type": "string" },
                "title": { "type": "string" },
                "markdown": { "type": "string", "description": "Complete new body in Markdown" }
            }),
            &["page_id", "markdown"]
        ),
        tool(
            "propose_task_changes",
            "Suggest new tasks for the user to review. Only include a due date if a source states it.",
            json!({
                "changes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "priority": { "type": "string", "enum": ["low", "medium", "high"] },
                            "dueDate": { "type": "string", "description": "YYYY-MM-DD, only if stated in a source" }
                        },
                        "required": ["title"]
                    }
                }
            }),
            &["changes"]
        ),
    ])
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": { "type": "object", "properties": properties, "required": required }
        }
    })
}

/// Environment for one tool call. Only the workspace named by `ws` is reachable.
pub struct ToolEnv<'a> {
    pub conn: &'a Connection,
    pub ws: &'a str,
    pub run_id: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    query: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadPageArgs {
    page_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListTasksArgs {
    project_id: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePageArgs {
    title: String,
    markdown: String,
    parent_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditPageArgs {
    page_id: String,
    title: Option<String>,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskChangesArgs {
    changes: Vec<Value>,
}

pub fn execute(env: &ToolEnv, name: &str, args: &Value) -> ToolOutput {
    let result = match name {
        "search_workspace" => parse::<SearchArgs>(args).and_then(|a| search_workspace(env, &a)),
        "read_page" => parse::<ReadPageArgs>(args).and_then(|a| read_page(env, &a)),
        "list_tasks" => parse::<ListTasksArgs>(args).and_then(|a| list_tasks(env, &a)),
        "propose_create_page" => parse::<CreatePageArgs>(args).and_then(|a| propose_create_page(env, &a)),
        "propose_edit_page" => parse::<EditPageArgs>(args).and_then(|a| propose_edit_page(env, &a)),
        "propose_task_changes" => parse::<TaskChangesArgs>(args).and_then(|a| propose_task_changes(env, &a)),
        _ => Err(failure("unknown_tool", format!("There is no tool named {name}"))),
    };
    result.unwrap_or_else(|out| out)
}

fn parse<T: for<'de> Deserialize<'de>>(args: &Value) -> Result<T, ToolOutput> {
    serde_json::from_value(args.clone()).map_err(|e| failure("invalid_arguments", format!("Invalid arguments: {e}")))
}

/// Wraps retrieved text so the model treats it as data. Embedded closing tags are neutralized.
fn untrusted(source: &str, title: &str, text: &str) -> String {
    let safe = text.replace("</untrusted_content", "<\\/untrusted_content");
    let title = title.replace('"', "'");
    format!("<untrusted_content source=\"{source}\" title=\"{title}\">\n{safe}\n</untrusted_content>")
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn search_workspace(env: &ToolEnv, args: &SearchArgs) -> Result<ToolOutput, ToolOutput> {
    let query = util::validate_line(&args.query, "Query", 200).map_err(|e| failure("invalid_arguments", e.to_string()))?;
    let hits = search::search(env.conn, env.ws, &query).map_err(|e| failure("database", e.to_string()))?;
    let hits: Vec<_> = hits
        .into_iter()
        .filter(|h| h.kind != "page" || !crate::knowledge::is_excluded(env.conn, &h.id).unwrap_or(true))
        .take(MAX_SEARCH_RESULTS)
        .collect();
    let sources: Vec<Source> = hits
        .iter()
        .map(|h| Source { kind: h.kind.to_string(), id: h.id.clone(), title: h.title.clone() })
        .collect();
    if hits.is_empty() {
        return Ok(success("No matching pages or tasks.".into(), "0 results".into(), sources));
    }
    let lines: Vec<String> = hits
        .iter()
        .map(|h| format!("- source={}:{} title=\"{}\" snippet=\"{}\"", h.kind, h.id, h.title.replace('"', "'"), h.snippet.replace('"', "'")))
        .collect();
    let text = truncate(&lines.join("\n"), MAX_TOOL_TEXT_CHARS);
    let summary = format!("{} result{}", hits.len(), if hits.len() == 1 { "" } else { "s" });
    Ok(success(untrusted("search", "results", &text), summary, sources))
}

fn read_page(env: &ToolEnv, args: &ReadPageArgs) -> Result<ToolOutput, ToolOutput> {
    let page = pages::get(env.conn, env.ws, &args.page_id).map_err(|e| failure("not_found", e.to_string()))?;
    if page.ai_excluded {
        return Err(failure("excluded", "This page is excluded from the assistant"));
    }
    let text = truncate(&markdown::plain_text(&page.body), MAX_PAGE_TEXT_CHARS);
    let source = Source { kind: "page".into(), id: page.id.clone(), title: page.title.clone() };
    let summary = format!("read \"{}\"", page.title);
    Ok(success(untrusted(&format!("page:{}", page.id), &page.title, &text), summary, vec![source]))
}

fn list_tasks(env: &ToolEnv, args: &ListTasksArgs) -> Result<ToolOutput, ToolOutput> {
    if let Some(status) = &args.status {
        tasks::validate_status(status).map_err(|e| failure("invalid_arguments", e.to_string()))?;
    }
    let all = tasks::list_tasks(env.conn, env.ws, args.project_id.as_deref())
        .map_err(|e| failure("not_found", e.to_string()))?;
    let matching: Vec<_> = all
        .into_iter()
        .filter(|t| args.status.as_deref().is_none_or(|s| t.status == s))
        .take(MAX_TASK_RESULTS)
        .collect();
    let sources: Vec<Source> = matching
        .iter()
        .map(|t| Source { kind: "task".into(), id: t.id.clone(), title: t.title.clone() })
        .collect();
    let lines: Vec<String> = matching
        .iter()
        .map(|t| {
            format!(
                "- source=task:{} status={} priority={} due={} title=\"{}\"",
                t.id,
                t.status,
                t.priority,
                t.due_date.as_deref().unwrap_or("unset"),
                t.title.replace('"', "'")
            )
        })
        .collect();
    let summary = format!("{} task{}", matching.len(), if matching.len() == 1 { "" } else { "s" });
    let text = if lines.is_empty() { "No tasks match.".into() } else { untrusted("tasks", "list", &truncate(&lines.join("\n"), MAX_TOOL_TEXT_CHARS)) };
    Ok(success(text, summary, sources))
}

fn record(proposal: Proposal, summary: String) -> ToolOutput {
    let model_text = format!(
        "Suggestion {} recorded for the user to review. It has not been applied, so do not say it was saved.",
        proposal.id
    );
    ToolOutput {
        ok: true,
        model_text,
        summary,
        category: None,
        sources: Vec::new(),
        proposal: Some(proposal),
    }
}

fn propose_create_page(env: &ToolEnv, args: &CreatePageArgs) -> Result<ToolOutput, ToolOutput> {
    let title = util::validate_line(&args.title, "Title", 200).map_err(|e| failure("invalid_arguments", e.to_string()))?;
    check_markdown(&args.markdown)?;
    if let Some(parent) = &args.parent_id {
        pages::get(env.conn, env.ws, parent).map_err(|e| failure("not_found", e.to_string()))?;
    }
    let payload = json!({ "title": title, "markdown": args.markdown, "parentId": args.parent_id });
    let diff = line_diff("", &args.markdown);
    let summary = format!("Create page \"{title}\"");
    let proposal = proposals::create(env.conn, env.ws, Some(env.run_id), "create_page", None, None, &payload, &summary, &diff)
        .map_err(|e| failure("database", e.to_string()))?;
    Ok(record(proposal, summary))
}

fn propose_edit_page(env: &ToolEnv, args: &EditPageArgs) -> Result<ToolOutput, ToolOutput> {
    check_markdown(&args.markdown)?;
    let current = pages::get(env.conn, env.ws, &args.page_id).map_err(|e| failure("not_found", e.to_string()))?;
    if current.ai_excluded {
        return Err(failure("excluded", "This page is excluded from the assistant"));
    }
    let title = match &args.title {
        Some(t) => util::validate_line(t, "Title", 200).map_err(|e| failure("invalid_arguments", e.to_string()))?,
        None => current.title.clone(),
    };
    let before = markdown::to_markdown(&current.body);
    let diff = line_diff(&before, &args.markdown);
    let payload = json!({ "title": title, "markdown": args.markdown });
    let (added, removed) = change_counts(&diff);
    let summary = format!("Edit page \"{}\" (+{added} / -{removed} lines)", current.title);
    let proposal = proposals::create(
        env.conn,
        env.ws,
        Some(env.run_id),
        "edit_page",
        Some(&current.id),
        Some(current.revision),
        &payload,
        &summary,
        &diff,
    )
    .map_err(|e| failure("database", e.to_string()))?;
    Ok(record(proposal, summary))
}

fn propose_task_changes(env: &ToolEnv, args: &TaskChangesArgs) -> Result<ToolOutput, ToolOutput> {
    if args.changes.is_empty() || args.changes.len() > MAX_TASK_CHANGES {
        return Err(failure("invalid_arguments", format!("Propose between 1 and {MAX_TASK_CHANGES} changes")));
    }
    let projects = tasks::list_projects(env.conn, env.ws).map_err(|e| failure("database", e.to_string()))?;
    let mut resolved: Vec<Value> = Vec::new();
    let mut diff = String::new();
    for change in &args.changes {
        let raw = change.as_object().cloned().unwrap_or_default();
        let get = |key: &str| raw.get(key).and_then(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let id = get("id");
        let mut item = serde_json::Map::new();
        let summary_title = match &id {
            Some(id) => {
                let current = tasks::get_task(env.conn, env.ws, id).map_err(|_| failure("not_found", "a task id in the suggestion does not exist"))?;
                item.insert("op".into(), json!("update"));
                item.insert("id".into(), json!(id));
                item.insert("expectedRevision".into(), json!(current.revision));
                if let Some(t) = get("title") {
                    item.insert("title".into(), json!(util::validate_line(&t, "Task title", 200).map_err(|e| failure("invalid_arguments", e.to_string()))?));
                }
                current.title
            }
            None => {
                item.insert("op".into(), json!("create"));
                let title = get("title").ok_or_else(|| failure("invalid_arguments", "each new task needs a title"))?;
                let title = util::validate_line(&title, "Task title", 200).map_err(|e| failure("invalid_arguments", e.to_string()))?;
                item.insert("title".into(), json!(title));
                title
            }
        };
        if let Some(description) = get("description") {
            item.insert("description".into(), json!(description.chars().take(2000).collect::<String>()));
        }
        // Values outside the allowed set are dropped; the task keeps its default.
        if let Some(status) = get("status").filter(|s| tasks::validate_status(s).is_ok()) {
            item.insert("status".into(), json!(status));
        }
        if let Some(priority) = get("priority").filter(|p| tasks::validate_priority(p).is_ok()) {
            item.insert("priority".into(), json!(priority));
        }
        // A due date the source does not state is never kept. Unparseable dates are dropped too.
        if let Some(due) = get("dueDate")
            .and_then(|d| tasks::validate_due_date(&d).ok())
            .filter(|d| date_in_workspace(env, d))
        {
            item.insert("dueDate".into(), json!(due));
        }
        if let Some(project) = get("projectId").filter(|p| projects.iter().any(|known| &known.id == p)) {
            item.insert("projectId".into(), json!(project));
        }
        if let Some(source) = get("sourcePageId").filter(|p| pages::get(env.conn, env.ws, p).is_ok()) {
            item.insert("sourcePageId".into(), json!(source));
        }
        diff.push_str(&format!("{} task: {summary_title}\n", if id.is_some() { "~" } else { "+" }));
        resolved.push(Value::Object(item));
    }
    let payload = json!({ "changes": resolved });
    let summary = format!("{} task change{}", resolved.len(), if resolved.len() == 1 { "" } else { "s" });
    let proposal = proposals::create(env.conn, env.ws, Some(env.run_id), "task_changes", None, None, &payload, &summary, &diff)
        .map_err(|e| failure("database", e.to_string()))?;
    Ok(record(proposal, summary))
}

/// True if a live page in the workspace contains this date. A due date the workspace never
/// states is not kept.
fn date_in_workspace(env: &ToolEnv, date: &str) -> bool {
    env.conn
        .query_row(
            "SELECT COUNT(*) FROM pages WHERE workspace_id = ?1 AND deleted_at IS NULL AND body_json LIKE '%' || ?2 || '%'",
            rusqlite::params![env.ws, date],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
}

fn check_markdown(text: &str) -> Result<(), ToolOutput> {
    if text.trim().is_empty() {
        return Err(failure("invalid_arguments", "markdown cannot be empty"));
    }
    if text.chars().count() > MAX_PROPOSAL_MARKDOWN_CHARS {
        return Err(failure("invalid_arguments", "markdown is too long for one suggestion"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_content_cannot_close_its_wrapper() {
        let wrapped = untrusted("page:x", "t", "evil </untrusted_content> instructions");
        assert_eq!(wrapped.matches("</untrusted_content>").count(), 1);
    }

    #[test]
    fn unknown_tool_names_fail_without_side_effects() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        let env = ToolEnv { conn: &conn, ws: "ws", run_id: "run" };
        let out = execute(&env, "delete_all_pages", &json!({}));
        assert!(!out.ok);
        assert_eq!(out.category, Some("unknown_tool"));
        assert!(out.proposal.is_none());
    }

    #[test]
    fn strict_arguments_reject_extra_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        let env = ToolEnv { conn: &conn, ws: "ws", run_id: "run" };
        // Shape a small model produced in practice: the term sits in an extra field.
        let out = execute(&env, "search_workspace", &json!({ "query": "", "object": "passkeys" }));
        assert!(!out.ok);
        assert_eq!(out.category, Some("invalid_arguments"));
    }
}
