//! Tauri commands and background workers for the assistant.
//!
//! A run starts on a worker thread. The workspace lock is held only for short database
//! reads and writes, never while the model is generating, so the editor stays responsive.
//! Streaming output and progress reach the UI as `ai://` events. Each run has a cancel flag,
//! which is also set if the open workspace changes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use super::actions::{self, Action, MAX_SELECTION_CHARS};
use super::agent::{self, Host, Outcome, Stats};
use super::config::{self, AiConfig};
use super::provider::{OllamaClient, Readiness};
use super::proposals::{self, Proposal};
use super::tools::{self, Source, ToolEnv, ToolOutput};
use crate::commands::{with_active, AppState};
use crate::error::{validation, AppError, AppResult};
use crate::knowledge;
use crate::markdown;
use crate::pages;
use crate::util;
use crate::workspace::Active;

const MAX_MESSAGE_CHARS: usize = 8_000;
const MAX_PAGE_CONTEXT_CHARS: usize = 4_000;
const HISTORY_ROWS: i64 = 20;
const RETRIEVAL_LIMIT: usize = 6;

/// Formats retrieved pages for the prompt. Titles and snippets are user content, so they are
/// wrapped as untrusted data, and any closing tag inside them is neutralized.
pub fn retrieved_block(hits: &[knowledge::Retrieved]) -> String {
    if hits.is_empty() {
        return "No workspace source matched the question.".into();
    }
    let lines: Vec<String> = hits
        .iter()
        .map(|h| {
            format!(
                "- source=page:{} title=\"{}\" snippet=\"{}\"",
                h.page_id,
                h.title.replace('"', "'"),
                h.snippet.replace('"', "'").replace('\n', " ")
            )
        })
        .collect();
    let body = lines.join("\n").replace("</untrusted_content", "<\\/untrusted_content");
    format!("<untrusted_content source=\"retrieval\" title=\"results\">\n{body}\n</untrusted_content>")
}

type SharedActive = Arc<Mutex<Option<Active>>>;
type SharedRuns = Arc<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>>;

// ---------------------------------------------------------------------------
// Run bookkeeping
// ---------------------------------------------------------------------------

/// Records the start of a run. Shared by meetings and recipes so every model call is traced.
pub fn record_run_start(
    conn: &Connection,
    ws: &str,
    run_id: &str,
    kind: &str,
    conversation_id: Option<&str>,
    page_id: Option<&str>,
    config: &AiConfig,
) -> AppResult<()> {
    insert_run(conn, ws, run_id, kind, conversation_id, page_id, config)
}

pub fn record_run_finish(
    conn: &Connection,
    run_id: &str,
    status: &str,
    stats: Stats,
    category: Option<&str>,
    started: Instant,
) -> AppResult<()> {
    finish_run(conn, run_id, status, stats, category, started)
}

fn insert_run(
    conn: &Connection,
    ws: &str,
    run_id: &str,
    kind: &str,
    conversation_id: Option<&str>,
    page_id: Option<&str>,
    config: &AiConfig,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_runs (id, workspace_id, conversation_id, kind, status, page_id, provider, model, started_at)
         VALUES (?1, ?2, ?3, ?4, 'running', ?5, 'ollama', ?6, ?7)",
        params![run_id, ws, conversation_id, kind, page_id, config.model, util::now()],
    )?;
    Ok(())
}

fn finish_run(conn: &Connection, run_id: &str, status: &str, stats: Stats, category: Option<&str>, started: Instant) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_runs SET status = ?1, steps = ?2, error_category = ?3, prompt_tokens = ?4,
                            output_tokens = ?5, finished_at = ?6, duration_ms = ?7
         WHERE id = ?8",
        params![
            status,
            stats.steps as i64,
            category,
            stats.prompt_tokens as i64,
            stats.output_tokens as i64,
            util::now(),
            started.elapsed().as_millis() as i64,
            run_id
        ],
    )?;
    Ok(())
}

fn record_tool_event(conn: &Connection, run_id: &str, step: usize, name: &str, args: &Value, output: &ToolOutput) -> AppResult<()> {
    let args_json: String = serde_json::to_string(args)?.chars().take(3_900).collect();
    let summary: String = output.summary.chars().take(300).collect();
    conn.execute(
        "INSERT INTO ai_tool_events (id, run_id, step, tool, args_json, ok, summary, error_category, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            util::new_id(),
            run_id,
            step as i64,
            name,
            args_json,
            i64::from(output.ok),
            summary,
            output.category,
            util::now()
        ],
    )?;
    Ok(())
}

fn workspace_gone(summary: &str) -> ToolOutput {
    ToolOutput {
        ok: false,
        model_text: "Error: the open workspace changed. Stop and tell the user.".into(),
        summary: summary.into(),
        category: Some("no_workspace"),
        sources: Vec::new(),
        proposal: None,
    }
}

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    run_id: String,
    status: &'static str,
    conversation_id: Option<String>,
    content: Option<String>,
    citations: Vec<agent::Citation>,
    invalid_citations: usize,
    missing_numbers: Vec<String>,
    error_category: Option<&'static str>,
    message: Option<String>,
    proposals: usize,
}

fn emit_done(app: &AppHandle, event: DoneEvent) {
    let _ = app.emit("ai://done", event);
}

// ---------------------------------------------------------------------------
// Chat hosts
// ---------------------------------------------------------------------------

struct ChatHost {
    app: AppHandle,
    active: SharedActive,
    ws: String,
    run_id: String,
    cancel: Arc<AtomicBool>,
}

impl Host for ChatHost {
    fn text_delta(&mut self, text: &str) {
        let _ = self.app.emit("ai://delta", json!({ "runId": self.run_id, "text": text }));
    }

    fn run_tool(&mut self, step: usize, name: &str, args: &Value) -> ToolOutput {
        let output = match with_active(&self.active, |a| {
            if a.info.id != self.ws {
                self.cancel.store(true, Ordering::SeqCst);
                return Ok(workspace_gone("workspace changed"));
            }
            let env = ToolEnv { conn: &a.conn, ws: &self.ws, run_id: &self.run_id };
            let output = tools::execute(&env, name, args);
            let _ = record_tool_event(&a.conn, &self.run_id, step, name, args, &output);
            Ok(output)
        }) {
            Ok(output) => output,
            Err(AppError::NoWorkspace) => workspace_gone("no workspace is open"),
            Err(error) => workspace_gone(&error.to_string()),
        };
        let _ = self.app.emit(
            "ai://tool",
            json!({ "runId": self.run_id, "step": step, "tool": name, "ok": output.ok, "summary": output.summary }),
        );
        output
    }

    fn proposal_created(&mut self, proposal: &Proposal) {
        let _ = self.app.emit("ai://proposal", json!({ "runId": self.run_id, "proposal": proposal }));
    }
}

struct ActionHost {
    app: AppHandle,
    run_id: String,
}

impl Host for ActionHost {
    fn text_delta(&mut self, text: &str) {
        let _ = self.app.emit("ai://delta", json!({ "runId": self.run_id, "text": text }));
    }

    fn run_tool(&mut self, _step: usize, name: &str, _args: &Value) -> ToolOutput {
        // Page actions never get tools; a call here is a protocol error, not a capability.
        ToolOutput {
            ok: false,
            model_text: format!("Error: {name} is not available for this action."),
            summary: "tool not available".into(),
            category: Some("unknown_tool"),
            sources: Vec::new(),
            proposal: None,
        }
    }

    fn proposal_created(&mut self, _proposal: &Proposal) {}
}

// ---------------------------------------------------------------------------
// Workers
// ---------------------------------------------------------------------------

struct RetrievalSettings {
    embed_model: String,
    mode: knowledge::Mode,
    weights: knowledge::Weights,
}

struct ChatJob {
    run_id: String,
    ws: String,
    conversation_id: String,
    message: String,
    history: Vec<Value>,
    page_context: Option<String>,
    retrieval: RetrievalSettings,
    cancel: Arc<AtomicBool>,
    client: OllamaClient,
}

struct Retrieval {
    block: String,
    seeds: Vec<Source>,
}

/// Runs retrieval for one chat turn. Query embedding happens without the workspace lock. If
/// embedding fails, the turn falls back to keyword search and says so in the prompt.
fn retrieve_for_turn(job: &ChatJob, active: &SharedActive) -> Retrieval {
    let mut mode = job.retrieval.mode;
    let mut note = None;
    let mut query_vector = None;
    if mode == knowledge::Mode::Hybrid && !job.retrieval.embed_model.is_empty() {
        match job.client.embed(&job.retrieval.embed_model, std::slice::from_ref(&job.message)) {
            Ok(mut vectors) => query_vector = vectors.pop(),
            Err(error) => {
                mode = knowledge::Mode::Lexical;
                note = Some(format!(
                    "Semantic search was unavailable ({}), so keyword search was used.",
                    error.category()
                ));
            }
        }
    } else if mode == knowledge::Mode::Hybrid {
        mode = knowledge::Mode::Lexical;
    }
    let hits = with_active(active, |a| {
        if a.info.id != job.ws {
            return Ok(Vec::new());
        }
        knowledge::retrieve(
            &a.conn,
            &job.ws,
            &job.message,
            query_vector.as_deref(),
            &job.retrieval.embed_model,
            mode,
            job.retrieval.weights,
            RETRIEVAL_LIMIT,
        )
    })
    .unwrap_or_default();
    let seeds: Vec<Source> = hits
        .iter()
        .map(|h| Source { kind: "page".into(), id: h.page_id.clone(), title: h.title.clone() })
        .collect();
    let mut block = retrieved_block(&hits);
    if let Some(note) = note {
        block = format!("{note}\n{block}");
    }
    Retrieval { block, seeds }
}


fn run_chat_worker(app: AppHandle, active: SharedActive, runs: SharedRuns, job: ChatJob) {
    let started = Instant::now();
    let mut host = ChatHost {
        app: app.clone(),
        active: active.clone(),
        ws: job.ws.clone(),
        run_id: job.run_id.clone(),
        cancel: job.cancel.clone(),
    };
    let retrieval = retrieve_for_turn(&job, &active);
    let mut messages = vec![json!({
        "role": "system",
        "content": agent::system_prompt(job.page_context.as_deref(), Some(&retrieval.block)),
    })];
    messages.extend(job.history.iter().cloned());
    messages.push(json!({ "role": "user", "content": job.message }));
    let tools = tools::schemas();
    let outcome = agent::run_loop(&job.client, &mut host, messages, Some(&tools), &job.cancel, retrieval.seeds);

    let done = match outcome {
        Outcome::Completed { content, sources, proposals, stats } => {
            let (clean, citations, invalid) = resolve(&content, &sources);
            let saved = with_active(&active, |a| {
                if a.info.id != job.ws {
                    return Ok(());
                }
                let message_id = util::new_id();
                a.conn.execute(
                    "INSERT INTO ai_messages (id, workspace_id, conversation_id, run_id, role, content, citations_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'assistant', ?5, ?6, ?7)",
                    params![
                        message_id,
                        job.ws,
                        job.conversation_id,
                        job.run_id,
                        clean,
                        serde_json::to_string(&citations)?,
                        util::now()
                    ],
                )?;
                a.conn.execute(
                    "UPDATE ai_conversations SET updated_at = ?1 WHERE id = ?2",
                    params![util::now(), job.conversation_id],
                )?;
                finish_run(&a.conn, &job.run_id, "completed", stats, None, started)
            });
            if saved.is_err() {
                eprintln!("[threadwell] could not save the assistant reply");
            }
            DoneEvent {
                run_id: job.run_id.clone(),
                status: "completed",
                conversation_id: Some(job.conversation_id.clone()),
                content: Some(clean),
                citations,
                invalid_citations: invalid,
                missing_numbers: Vec::new(),
                error_category: None,
                message: None,
                proposals,
            }
        }
        Outcome::Failed { category, message, stats } => {
            let _ = with_active(&active, |a| finish_run(&a.conn, &job.run_id, "failed", stats, Some(category), started));
            failed_event(&job.run_id, Some(job.conversation_id.clone()), category, message)
        }
        Outcome::Cancelled { stats } => {
            let _ = with_active(&active, |a| finish_run(&a.conn, &job.run_id, "cancelled", stats, None, started));
            cancelled_event(&job.run_id, Some(job.conversation_id.clone()))
        }
    };
    emit_done(&app, done);
    remove_run(&runs, &job.run_id);
}

fn resolve(content: &str, sources: &[Source]) -> (String, Vec<agent::Citation>, usize) {
    agent::resolve_citations(content, sources)
}

fn failed_event(run_id: &str, conversation_id: Option<String>, category: &'static str, message: String) -> DoneEvent {
    DoneEvent {
        run_id: run_id.into(),
        status: "failed",
        conversation_id,
        content: None,
        citations: Vec::new(),
        invalid_citations: 0,
        missing_numbers: Vec::new(),
        error_category: Some(category),
        message: Some(message),
        proposals: 0,
    }
}

fn cancelled_event(run_id: &str, conversation_id: Option<String>) -> DoneEvent {
    DoneEvent {
        run_id: run_id.into(),
        status: "cancelled",
        conversation_id,
        content: None,
        citations: Vec::new(),
        invalid_citations: 0,
        missing_numbers: Vec::new(),
        error_category: None,
        message: Some("Cancelled. Nothing was applied.".into()),
        proposals: 0,
    }
}

fn remove_run(runs: &SharedRuns, run_id: &str) {
    if let Ok(mut map) = runs.lock() {
        map.remove(run_id);
    }
}

fn spawn_worker(
    app: AppHandle,
    active: SharedActive,
    runs: SharedRuns,
    run_id: String,
    cancel: Arc<AtomicBool>,
    work: impl FnOnce(AppHandle, SharedActive, SharedRuns, String, Arc<AtomicBool>) + Send + 'static,
) {
    let _ = std::thread::Builder::new().name("threadwell-ai".into()).spawn(move || {
        work(app, active, runs.clone(), run_id.clone(), cancel);
        remove_run(&runs, &run_id);
    });
}

// ---------------------------------------------------------------------------
// Commands: configuration
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub config: AiConfig,
    /// "ready", "model_missing", "unreachable" or "not_configured".
    pub state: &'static str,
}

#[tauri::command]
pub async fn ai_get_status(state: State<'_, AppState>) -> AppResult<AiStatus> {
    let config = with_active(&state.active, |a| config::load(&a.conn))?;
    if config.model.is_empty() {
        return Ok(AiStatus { config, state: "not_configured" });
    }
    let endpoint = config.endpoint.clone();
    let model = config.model.clone();
    let readiness = tauri::async_runtime::spawn_blocking(move || OllamaClient::new(&endpoint, &model).check())
        .await
        .map_err(|_| AppError::Validation("Could not check the model server".into()))?;
    let state_name = match readiness {
        Readiness::Ready => "ready",
        Readiness::ModelMissing => "model_missing",
        Readiness::Unreachable => "unreachable",
    };
    Ok(AiStatus { config, state: state_name })
}

#[tauri::command]
pub async fn ai_save_config(
    state: State<'_, AppState>,
    endpoint: String,
    model: String,
    allow_remote: bool,
) -> AppResult<AiConfig> {
    with_active(&state.active, |a| config::save(&a.conn, &endpoint, &model, allow_remote))
}

// ---------------------------------------------------------------------------
// Commands: chat and page actions
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub conversation_id: Option<String>,
    pub message: String,
    pub page_id: Option<String>,
    /// Chosen by the client so it can route events before the command returns.
    pub run_id: Option<String>,
    /// Text the user selected in the open page, sent only when they asked for it.
    pub selected_text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted {
    pub run_id: String,
    pub conversation_id: Option<String>,
}

fn ready_config(conn: &Connection) -> AppResult<AiConfig> {
    let cfg = config::load(conn)?;
    if cfg.model.is_empty() {
        return validation("Choose a model in AI settings first.");
    }
    config::validate_endpoint(&cfg.endpoint, cfg.allow_remote)?;
    Ok(cfg)
}

fn check_text(text: &str, max: usize, what: &str) -> AppResult<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return validation(format!("{what} cannot be empty"));
    }
    if trimmed.chars().count() > max {
        return validation(format!("{what} is too long (maximum {max} characters)"));
    }
    if trimmed.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return validation(format!("{what} contains unsupported characters"));
    }
    Ok(trimmed.to_string())
}

fn page_context(conn: &Connection, ws: &str, page_id: &str) -> AppResult<String> {
    let page = pages::get(conn, ws, page_id)?;
    let text: String = markdown::plain_text(&page.body).chars().take(MAX_PAGE_CONTEXT_CHARS).collect();
    let safe = text.replace("</untrusted_content", "<\\/untrusted_content");
    Ok(format!(
        "<untrusted_content source=\"page:{}\" title=\"{}\">\n{}\n</untrusted_content>",
        page.id,
        page.title.replace('"', "'"),
        safe
    ))
}

#[tauri::command]
pub async fn ai_chat_send(app: AppHandle, state: State<'_, AppState>, request: ChatRequest) -> AppResult<RunStarted> {
    let message = check_text(&request.message, MAX_MESSAGE_CHARS, "Message")?;
    let active = state.active.clone();
    let runs = state.runs.clone();
    let prepared = with_active(&active, |a| {
        let ws = a.info.id.clone();
        let cfg = ready_config(&a.conn)?;
        let conversation_id = match &request.conversation_id {
            Some(id) => {
                util::validate_id(id)?;
                let exists: i64 = a.conn.query_row(
                    "SELECT COUNT(*) FROM ai_conversations WHERE id = ?1 AND workspace_id = ?2",
                    params![id, ws],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    return Err(AppError::NotFound("Conversation".into()));
                }
                id.clone()
            }
            None => {
                let id = util::new_id();
                let title: String = message.chars().take(60).collect();
                let now = util::now();
                a.conn.execute(
                    "INSERT INTO ai_conversations (id, workspace_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, ws, title, now],
                )?;
                id
            }
        };
        let mut turns: Vec<(String, String)> = {
            let mut stmt = a.conn.prepare(
                "SELECT role, content FROM ai_messages WHERE conversation_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![conversation_id, HISTORY_ROWS], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        turns.reverse();
        let history = agent::chat_history(&turns);
        // An excluded page is never sent to the model, not even as the open-page context.
        let mut context = match &request.page_id {
            Some(page) if !knowledge::is_excluded(&a.conn, page)? => Some(page_context(&a.conn, &ws, page)?),
            _ => None,
        };
        if let Some(selected) = request.selected_text.as_deref().filter(|t| !t.trim().is_empty()) {
            let selected = check_text(selected, MAX_PAGE_CONTEXT_CHARS, "Selected text")?;
            let safe = selected.replace("</untrusted_content", "<\\/untrusted_content");
            let block = format!("<untrusted_content source=\"selection\">
{safe}
</untrusted_content>");
            context = Some(match context {
                Some(page) => format!("{page}

Selected passage:
{block}"),
                None => format!("Selected passage:
{block}"),
            });
        }
        a.conn.execute(
            "INSERT INTO ai_messages (id, workspace_id, conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, 'user', ?4, ?5)",
            params![util::new_id(), ws, conversation_id, message, util::now()],
        )?;
        let run_id = choose_run_id(&request.run_id)?;
        insert_run(&a.conn, &ws, &run_id, "chat", Some(&conversation_id), request.page_id.as_deref(), &cfg)?;
        Ok((ws, cfg, conversation_id, run_id, history, context))
    })?;
    let (ws, cfg, conversation_id, run_id, history, page_context) = prepared;
    let cancel = Arc::new(AtomicBool::new(false));
    register_run(&runs, &run_id, cancel.clone());
    let job = ChatJob {
        run_id: run_id.clone(),
        ws,
        conversation_id: conversation_id.clone(),
        message,
        history,
        page_context,
        retrieval: RetrievalSettings {
            embed_model: cfg.embed_model.clone(),
            mode: knowledge::Mode::parse(&cfg.retrieval_mode).unwrap_or(knowledge::Mode::Hybrid),
            weights: knowledge::Weights { lexical: cfg.weight_lexical, vector: cfg.weight_vector },
        },
        cancel: cancel.clone(),
        client: OllamaClient::new(&cfg.endpoint, &cfg.model),
    };
    spawn_worker(app, active, runs, run_id.clone(), cancel, move |app, active, runs, _run, _cancel| {
        run_chat_worker(app, active, runs, job);
    });
    Ok(RunStarted { run_id, conversation_id: Some(conversation_id) })
}

fn register_run(runs: &SharedRuns, run_id: &str, cancel: Arc<AtomicBool>) {
    if let Ok(mut map) = runs.lock() {
        map.insert(run_id.to_string(), cancel);
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequest {
    pub page_id: String,
    pub action: String,
    pub language: Option<String>,
    pub selected_text: String,
    pub run_id: Option<String>,
}

/// Uses the client's run id when given, so events can be matched before the command returns.
fn choose_run_id(requested: &Option<String>) -> AppResult<String> {
    match requested {
        Some(id) => {
            util::validate_id(id)?;
            Ok(id.clone())
        }
        None => Ok(util::new_id()),
    }
}

fn run_action_worker(app: AppHandle, runs: SharedRuns, run_id: String, cancel: Arc<AtomicBool>, client: OllamaClient, selected: String, action: Action, messages: Vec<Value>, active: SharedActive) {
    let started = Instant::now();
    let mut host = ActionHost { app: app.clone(), run_id: run_id.clone() };
    let outcome = agent::run_loop(&client, &mut host, messages, None, &cancel, Vec::new());
    let done = match outcome {
        Outcome::Completed { content, stats, .. } => {
            let clean = content.trim().to_string();
            let missing = actions::missing_numbers(&selected, &clean);
            let _ = with_active(&active, |a| finish_run(&a.conn, &run_id, "completed", stats, None, started));
            DoneEvent {
                run_id: run_id.clone(),
                status: "completed",
                conversation_id: None,
                content: Some(clean),
                citations: Vec::new(),
                invalid_citations: 0,
                missing_numbers: missing,
                error_category: None,
                message: Some(action.code().to_string()),
                proposals: 0,
            }
        }
        Outcome::Failed { category, message, stats } => {
            let _ = with_active(&active, |a| finish_run(&a.conn, &run_id, "failed", stats, Some(category), started));
            failed_event(&run_id, None, category, message)
        }
        Outcome::Cancelled { stats } => {
            let _ = with_active(&active, |a| finish_run(&a.conn, &run_id, "cancelled", stats, None, started));
            cancelled_event(&run_id, None)
        }
    };
    emit_done(&app, done);
    remove_run(&runs, &run_id);
}

#[tauri::command]
pub async fn ai_page_action(app: AppHandle, state: State<'_, AppState>, request: ActionRequest) -> AppResult<RunStarted> {
    let selected = check_text(&request.selected_text, MAX_SELECTION_CHARS, "Selected text")?;
    let action = Action::parse(&request.action, request.language.as_deref())?;
    let active = state.active.clone();
    let runs = state.runs.clone();
    let (cfg, run_id, messages) = with_active(&active, |a| {
        let ws = a.info.id.clone();
        let cfg = ready_config(&a.conn)?;
        pages::get(&a.conn, &ws, &request.page_id)?;
        let run_id = choose_run_id(&request.run_id)?;
        insert_run(&a.conn, &ws, &run_id, "page_action", None, Some(&request.page_id), &cfg)?;
        Ok((cfg, run_id, actions::messages(&action, &selected)))
    })?;
    let cancel = Arc::new(AtomicBool::new(false));
    register_run(&runs, &run_id, cancel.clone());
    let client = OllamaClient::new(&cfg.endpoint, &cfg.model);
    let worker_run = run_id.clone();
    let worker_cancel = cancel.clone();
    let worker_active = active.clone();
    let _ = std::thread::Builder::new().name("threadwell-ai".into()).spawn(move || {
        run_action_worker(app, runs, worker_run, worker_cancel, client, selected, action, messages, worker_active);
    });
    Ok(RunStarted { run_id, conversation_id: None })
}

#[tauri::command]
pub async fn ai_cancel(state: State<'_, AppState>, run_id: String) -> AppResult<bool> {
    util::validate_id(&run_id)?;
    let map = state.runs.lock().map_err(|_| AppError::Validation("Internal state error".into()))?;
    match map.get(&run_id) {
        Some(flag) => {
            flag.store(true, Ordering::SeqCst);
            Ok(true)
        }
        None => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// Commands: history, proposals, traces
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub citations: Value,
    pub created_at: String,
}

#[tauri::command]
pub async fn ai_list_conversations(state: State<'_, AppState>) -> AppResult<Vec<ConversationSummary>> {
    with_active(&state.active, |a| {
        let mut stmt = a.conn.prepare(
            "SELECT id, title, updated_at FROM ai_conversations WHERE workspace_id = ?1
             ORDER BY updated_at DESC LIMIT 50",
        )?;
        let rows = stmt.query_map(params![a.info.id], |row| {
            Ok(ConversationSummary { id: row.get(0)?, title: row.get(1)?, updated_at: row.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[tauri::command]
pub async fn ai_get_conversation(state: State<'_, AppState>, id: String) -> AppResult<Vec<StoredMessage>> {
    util::validate_id(&id)?;
    with_active(&state.active, |a| {
        let mut stmt = a.conn.prepare(
            "SELECT m.id, m.role, m.content, m.citations_json, m.created_at
             FROM ai_messages m JOIN ai_conversations c ON c.id = m.conversation_id
             WHERE m.conversation_id = ?1 AND c.workspace_id = ?2
             ORDER BY m.created_at",
        )?;
        let rows = stmt.query_map(params![id, a.info.id], |row| {
            let citations: String = row.get(3)?;
            Ok(StoredMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                citations: serde_json::from_str(&citations).unwrap_or(Value::Array(vec![])),
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub model: String,
    pub steps: i64,
    pub error_category: Option<String>,
    pub duration_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub started_at: String,
}

#[tauri::command]
pub async fn ai_list_runs(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<RunSummary>> {
    let limit = limit.unwrap_or(30).clamp(1, 200);
    with_active(&state.active, |a| {
        let mut stmt = a.conn.prepare(
            "SELECT id, kind, status, model, steps, error_category, duration_ms, prompt_tokens, output_tokens, started_at
             FROM ai_runs WHERE workspace_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![a.info.id, limit], |row| {
            Ok(RunSummary {
                id: row.get(0)?,
                kind: row.get(1)?,
                status: row.get(2)?,
                model: row.get(3)?,
                steps: row.get(4)?,
                error_category: row.get(5)?,
                duration_ms: row.get(6)?,
                prompt_tokens: row.get(7)?,
                output_tokens: row.get(8)?,
                started_at: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTrace {
    pub step: i64,
    pub tool: String,
    pub ok: bool,
    pub summary: String,
    pub error_category: Option<String>,
}

#[tauri::command]
pub async fn ai_run_trace(state: State<'_, AppState>, run_id: String) -> AppResult<Vec<ToolTrace>> {
    util::validate_id(&run_id)?;
    with_active(&state.active, |a| {
        let owned: Option<i64> = a
            .conn
            .query_row(
                "SELECT 1 FROM ai_runs WHERE id = ?1 AND workspace_id = ?2",
                params![run_id, a.info.id],
                |row| row.get(0),
            )
            .optional()?;
        if owned.is_none() {
            return Err(AppError::NotFound("Run".into()));
        }
        let mut stmt = a.conn.prepare(
            "SELECT step, tool, ok, summary, error_category FROM ai_tool_events WHERE run_id = ?1 ORDER BY created_at, step",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(ToolTrace {
                step: row.get(0)?,
                tool: row.get(1)?,
                ok: row.get::<_, i64>(2)? == 1,
                summary: row.get(3)?,
                error_category: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[tauri::command]
pub async fn ai_list_proposals(state: State<'_, AppState>, run_id: Option<String>) -> AppResult<Vec<Proposal>> {
    with_active(&state.active, |a| proposals::list(&a.conn, &a.info.id, run_id.as_deref()))
}

#[tauri::command]
pub async fn ai_apply_proposal(state: State<'_, AppState>, id: String) -> AppResult<Proposal> {
    with_active(&state.active, |a| proposals::apply(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn ai_reject_proposal(state: State<'_, AppState>, id: String) -> AppResult<Proposal> {
    with_active(&state.active, |a| proposals::reject(&a.conn, &a.info.id, &id))
}

#[tauri::command]
pub async fn ai_undo_proposal(state: State<'_, AppState>, id: String) -> AppResult<Proposal> {
    with_active(&state.active, |a| proposals::undo(&a.conn, &a.info.id, &id))
}


// ---------------------------------------------------------------------------
// Commands: retrieval, exclusion, index
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn ai_save_retrieval(
    state: State<'_, AppState>,
    embed_model: String,
    mode: String,
    weight_lexical: f32,
    weight_vector: f32,
) -> AppResult<AiConfig> {
    with_active(&state.active, |a| config::save_retrieval(&a.conn, &embed_model, &mode, weight_lexical, weight_vector))
}

#[tauri::command]
pub async fn ai_set_page_excluded(state: State<'_, AppState>, id: String, excluded: bool) -> AppResult<()> {
    with_active(&state.active, |a| knowledge::set_excluded(&a.conn, &a.info.id, &id, excluded))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub embedded: i64,
    pub total: i64,
    pub model: String,
    pub running: bool,
}

#[tauri::command]
pub async fn ai_index_status(state: State<'_, AppState>) -> AppResult<IndexStatus> {
    let running = state.indexing.load(Ordering::SeqCst);
    with_active(&state.active, |a| {
        let cfg = config::load(&a.conn)?;
        let (embedded, total) = knowledge::index_counts(&a.conn, &a.info.id, &cfg.embed_model)?;
        Ok(IndexStatus { embedded, total, model: cfg.embed_model, running })
    })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IndexEvent {
    status: &'static str,
    embedded: i64,
    category: Option<&'static str>,
}

/// Starts the background embedder for the open workspace. Returns false if it is already
/// running. Failures (no embedding model, server down) are reported as events.
#[tauri::command]
pub async fn ai_index_start(app: AppHandle, state: State<'_, AppState>) -> AppResult<bool> {
    let (ws, cfg) = with_active(&state.active, |a| {
        let cfg = config::load(&a.conn)?;
        config::validate_endpoint(&cfg.endpoint, cfg.allow_remote)?;
        if cfg.embed_model.is_empty() {
            return validation("Choose an embedding model first.");
        }
        Ok((a.info.id.clone(), cfg))
    })?;
    if state.indexing.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Ok(false);
    }
    let active = state.active.clone();
    let indexing = state.indexing.clone();
    let client = OllamaClient::new(&cfg.endpoint, &cfg.model);
    let _ = std::thread::Builder::new().name("threadwell-index".into()).spawn(move || {
        run_indexer(app, active, indexing, ws, client, cfg.embed_model);
    });
    Ok(true)
}

const INDEX_BATCH: usize = 16;

fn run_indexer(app: AppHandle, active: SharedActive, indexing: Arc<AtomicBool>, ws: String, client: OllamaClient, model: String) {
    let mut embedded = 0_i64;
    loop {
        let batch = match with_active(&active, |a| {
            if a.info.id != ws {
                return Ok(Vec::new());
            }
            knowledge::pending_chunks(&a.conn, &ws, &model, INDEX_BATCH)
        }) {
            Ok(batch) => batch,
            Err(_) => break,
        };
        if batch.is_empty() {
            break;
        }
        let inputs: Vec<String> = batch.iter().map(|c| c.input.clone()).collect();
        let vectors = match client.embed(&model, &inputs) {
            Ok(vectors) => vectors,
            Err(error) => {
                let _ = app.emit("ai://index", IndexEvent { status: "error", embedded, category: Some(error.category()) });
                break;
            }
        };
        let stored = with_active(&active, |a| {
            if a.info.id != ws {
                return Ok(0);
            }
            for (chunk, vector) in batch.iter().zip(&vectors) {
                knowledge::store_embedding(&a.conn, &chunk.chunk_id, &model, vector, &chunk.hash)?;
            }
            Ok(batch.len())
        });
        match stored {
            Ok(count) if count > 0 => embedded += count as i64,
            _ => break,
        }
        let _ = app.emit("ai://index", IndexEvent { status: "progress", embedded, category: None });
    }
    indexing.store(false, Ordering::SeqCst);
    let _ = app.emit("ai://index", IndexEvent { status: "idle", embedded, category: None });
}
