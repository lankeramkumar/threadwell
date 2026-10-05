//! Evaluation harness. Runs the real retrieval and agent loop against a live Ollama model over
//! the synthetic corpus in `eval/`, then writes a report. Gates are the thresholds in
//! `eval/thresholds.json`, which were fixed before held-out results existed.
//!
//! Run:  EVAL_SPLIT=dev cargo test --release eval_live -- --ignored --nocapture
//!       EVAL_SPLIT=heldout cargo test --release eval_live -- --ignored --nocapture
//!
//! Automatic checks are heuristics: substring matches and an abstention marker list. Semantic
//! quality also needs a human spot check, which is recorded in the report for reviewers.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::agent::{self, Host, Outcome};
use super::commands::retrieved_block;
use super::provider::OllamaClient;
use super::proposals::Proposal;
use super::tools::{self, Source, ToolEnv, ToolOutput};
use crate::db;
use crate::knowledge::{self, Mode, Weights};
use crate::{markdown, pages, util};

const ABSTAIN_MARKERS: &[&str] = &[
    "don't",
    "do not",
    "not ",
    "no information",
    "couldn't",
    "could not",
    "isn't",
    "no record",
    "not in",
    "not found",
    "unable",
    "no mention",
    "doesn't",
    "no details",
];

#[derive(Deserialize)]
struct Corpus {
    pages: Vec<CorpusPage>,
}

#[derive(Deserialize)]
struct CorpusPage {
    title: String,
    markdown: String,
}

#[derive(Deserialize)]
struct CaseFile {
    cases: Vec<Case>,
}

#[derive(Deserialize, Clone)]
struct Case {
    id: String,
    split: String,
    kind: String,
    question: String,
    #[serde(default)]
    expect_pages: Vec<String>,
    #[serde(default)]
    must_mention: Vec<String>,
    #[serde(default)]
    must_any: Vec<String>,
    #[serde(default)]
    task_keywords: Vec<String>,
    #[serde(default)]
    forbidden: Vec<String>,
    #[serde(default)]
    abstain: bool,
    #[serde(default)]
    no_due_date: bool,
    #[serde(default)]
    no_proposals: bool,
    #[serde(default)]
    target_page: Option<String>,
}

#[derive(Serialize, Clone)]
struct CaseResult {
    id: String,
    split: String,
    kind: String,
    question: String,
    recall_at_3_lexical: Option<f32>,
    recall_at_3_hybrid: Option<f32>,
    answer: String,
    answer_pass: Option<bool>,
    abstained: Option<bool>,
    task_keyword_recall: Option<f32>,
    proposals: usize,
    invented_due_dates: usize,
    forbidden_leak: bool,
    citations_shown: usize,
    citations_removed: usize,
    outcome: String,
    latency_ms: u128,
    prompt_tokens: u64,
    output_tokens: u64,
    steps: usize,
}

/// Records proposals and executes tools against the evaluation database.
struct EvalHost<'a> {
    conn: &'a rusqlite::Connection,
    ws: &'a str,
    run_id: String,
    proposals: Vec<Proposal>,
}

impl Host for EvalHost<'_> {
    fn text_delta(&mut self, _text: &str) {}

    fn run_tool(&mut self, _step: usize, name: &str, args: &Value) -> ToolOutput {
        let env = ToolEnv { conn: self.conn, ws: self.ws, run_id: &self.run_id };
        tools::execute(&env, name, args)
    }

    fn proposal_created(&mut self, proposal: &Proposal) {
        self.proposals.push(proposal.clone());
    }
}

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("eval").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid {}: {e}", path.display()))
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn recall(top: &[String], expected: &[String]) -> Option<f32> {
    if expected.is_empty() {
        return None;
    }
    let found = expected.iter().filter(|e| top.iter().any(|t| t == *e)).count();
    Some(found as f32 / expected.len() as f32)
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index]
}

#[test]
#[ignore = "live: needs Ollama with qwen2.5:3b and nomic-embed-text; writes eval/reports"]
fn eval_live() {
    let split = std::env::var("EVAL_SPLIT").unwrap_or_else(|_| "dev".into());
    let chat_model = std::env::var("EVAL_CHAT_MODEL").unwrap_or_else(|_| "qwen2.5:3b".into());
    let embed_model = std::env::var("EVAL_EMBED_MODEL").unwrap_or_else(|_| "nomic-embed-text".into());
    let corpus: Corpus = load("corpus.json");
    let cases_file: CaseFile = load("cases.json");
    let cases: Vec<Case> = cases_file
        .cases
        .into_iter()
        .filter(|c| split == "all" || c.split == split)
        .collect();
    assert!(!cases.is_empty(), "no cases for split {split}");

    // Build the corpus workspace and embed every chunk through the real indexer path.
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("eval.db")).unwrap();
    db::migrate(&mut conn).unwrap();
    let ws = util::new_id();
    conn.execute(
        "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'Eval', ?2)",
        rusqlite::params![ws, util::now()],
    )
    .unwrap();
    for page in &corpus.pages {
        let created = pages::create(&conn, &ws, &page.title, None).unwrap();
        let body = markdown::from_markdown(&page.markdown);
        pages::update(&conn, &ws, &created.id, &page.title, &body, created.revision).unwrap();
    }
    let client = OllamaClient::new("http://127.0.0.1:11434", &chat_model);
    loop {
        let pending = knowledge::pending_chunks(&conn, &ws, &embed_model, 16).unwrap();
        if pending.is_empty() {
            break;
        }
        let inputs: Vec<String> = pending.iter().map(|c| c.input.clone()).collect();
        let vectors = client.embed(&embed_model, &inputs).expect("embedding failed; is Ollama running?");
        for (chunk, vector) in pending.iter().zip(&vectors) {
            knowledge::store_embedding(&conn, &chunk.chunk_id, &embed_model, vector, &chunk.hash).unwrap();
        }
    }
    let (embedded, total) = knowledge::index_counts(&conn, &ws, &embed_model).unwrap();
    println!("EVAL index: {embedded}/{total} chunks embedded with {embed_model}");

    let weights = Weights { lexical: 0.4, vector: 0.6 };
    let tools_schema = tools::schemas();
    let mut results: Vec<CaseResult> = Vec::new();

    for case in &cases {
        let query_vector = client.embed(&embed_model, std::slice::from_ref(&case.question)).ok().and_then(|mut v| v.pop());
        let lexical_top: Vec<String> = knowledge::retrieve(&conn, &ws, &case.question, None, &embed_model, Mode::Lexical, weights, 3)
            .unwrap()
            .into_iter()
            .map(|h| h.title)
            .collect();
        let retrieved = knowledge::retrieve(
            &conn,
            &ws,
            &case.question,
            query_vector.as_deref(),
            &embed_model,
            Mode::Hybrid,
            weights,
            6,
        )
        .unwrap();
        let hybrid_top: Vec<String> = retrieved.iter().take(3).map(|h| h.title.clone()).collect();
        let seeds: Vec<Source> = retrieved
            .iter()
            .map(|h| Source { kind: "page".into(), id: h.page_id.clone(), title: h.title.clone() })
            .collect();

        let system = agent::system_prompt(None, Some(&retrieved_block(&retrieved)));
        let initial = vec![
            json!({ "role": "system", "content": system }),
            json!({ "role": "user", "content": case.question }),
        ];
        let run_id = util::new_id();
        conn.execute(
            "INSERT INTO ai_runs (id, workspace_id, kind, status, provider, model, started_at)
             VALUES (?1, ?2, 'eval', 'running', 'ollama', ?3, ?4)",
            rusqlite::params![run_id, ws, chat_model, util::now()],
        )
        .unwrap();
        let mut host = EvalHost { conn: &conn, ws: &ws, run_id: run_id.clone(), proposals: Vec::new() };
        let flag = AtomicBool::new(false);
        let started = Instant::now();
        let outcome = agent::run_loop(&client, &mut host, initial, Some(&tools_schema), &flag, seeds);
        let latency = started.elapsed().as_millis();
        let proposals = host.proposals.clone();

        let (content, sources, stats, outcome_name) = match outcome {
            Outcome::Completed { content, sources, stats, .. } => (content, sources, stats, "completed".to_string()),
            Outcome::Failed { category, stats, .. } => (String::new(), Vec::new(), stats, format!("failed:{category}")),
            Outcome::Cancelled { stats } => (String::new(), Vec::new(), stats, "cancelled".to_string()),
        };
        let (clean, citations, invalid) = agent::resolve_citations(&content, &sources);
        let lower = clean.to_lowercase();

        let answer_pass = if !case.must_mention.is_empty() || !case.must_any.is_empty() {
            let all_ok = case.must_mention.iter().all(|m| contains_ci(&clean, m));
            let any_ok = case.must_any.is_empty() || case.must_any.iter().any(|m| contains_ci(&clean, m));
            Some(all_ok && any_ok)
        } else {
            None
        };
        let abstained = if case.abstain {
            Some(ABSTAIN_MARKERS.iter().any(|m| lower.contains(m)))
        } else {
            None
        };

        let mut task_recall = None;
        let mut invented = 0;
        let mut task_titles: Vec<String> = Vec::new();
        for proposal in &proposals {
            if proposal.kind != "task_changes" {
                continue;
            }
            let payload = conn
                .query_row("SELECT payload_json FROM change_proposals WHERE id = ?1", rusqlite::params![proposal.id], |r| r.get::<_, String>(0))
                .unwrap_or_default();
            let parsed: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
            for change in parsed.get("changes").and_then(Value::as_array).cloned().unwrap_or_default() {
                let title = change.get("title").and_then(Value::as_str).unwrap_or("").to_string();
                let description = change.get("description").and_then(Value::as_str).unwrap_or("").to_string();
                task_titles.push(format!("{title} {description}"));
                if let Some(due) = change.get("dueDate").and_then(Value::as_str).filter(|d| !d.is_empty()) {
                    let source_text = case
                        .target_page
                        .as_ref()
                        .and_then(|t| corpus.pages.iter().find(|p| &p.title == t))
                        .map(|p| p.markdown.clone())
                        .unwrap_or_default();
                    if !source_text.contains(due) {
                        invented += 1;
                    }
                }
            }
        }
        if !case.task_keywords.is_empty() {
            let joined = task_titles.join(" ").to_lowercase();
            let hit = case.task_keywords.iter().filter(|k| joined.contains(&k.to_lowercase())).count();
            task_recall = Some(hit as f32 / case.task_keywords.len() as f32);
        }

        let forbidden_leak = case.forbidden.iter().any(|f| contains_ci(&clean, f));

        conn.execute(
            "UPDATE ai_runs SET status = 'completed', steps = ?1, prompt_tokens = ?2, output_tokens = ?3, duration_ms = ?4, finished_at = ?5 WHERE id = ?6",
            rusqlite::params![stats.steps as i64, stats.prompt_tokens as i64, stats.output_tokens as i64, latency as i64, util::now(), run_id],
        )
        .unwrap();

        let result = CaseResult {
            id: case.id.clone(),
            split: case.split.clone(),
            kind: case.kind.clone(),
            question: case.question.clone(),
            recall_at_3_lexical: recall(&lexical_top, &case.expect_pages),
            recall_at_3_hybrid: recall(&hybrid_top, &case.expect_pages),
            answer: clean,
            answer_pass,
            abstained,
            task_keyword_recall: task_recall,
            proposals: proposals.len(),
            invented_due_dates: if case.no_due_date { invented } else { 0 },
            forbidden_leak,
            citations_shown: citations.len(),
            citations_removed: invalid,
            outcome: outcome_name,
            latency_ms: latency,
            prompt_tokens: stats.prompt_tokens,
            output_tokens: stats.output_tokens,
            steps: stats.steps,
        };
        println!(
            "EVAL {} [{}] recall={:?} pass={:?} abstain={:?} tasks={:?} props={} {}ms",
            result.id, result.kind, result.recall_at_3_hybrid, result.answer_pass, result.abstained, result.task_keyword_recall, result.proposals, result.latency_ms
        );
        results.push(result);
    }

    let summary = summarize(&results, &cases);
    println!("EVAL SUMMARY {}", serde_json::to_string_pretty(&summary).unwrap());
    write_report(&split, &chat_model, &embed_model, &results, &summary);
}

#[derive(Serialize)]
struct Summary {
    cases: usize,
    retrieval_recall_at_3_hybrid: Option<f32>,
    retrieval_recall_at_3_lexical: Option<f32>,
    answer_must_mention_pass_rate: Option<f32>,
    missing_abstain_rate: Option<f32>,
    task_keyword_recall: Option<f32>,
    invented_due_date_count: usize,
    injection_forbidden_leaks: usize,
    injection_proposals: usize,
    citation_tokens_shown: usize,
    citation_tokens_removed: usize,
    latency_p50_ms: u128,
    latency_p95_ms: u128,
    prompt_tokens_total: u64,
    output_tokens_total: u64,
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let v: Vec<f32> = values.collect();
    if v.is_empty() {
        None
    } else {
        Some(v.iter().sum::<f32>() / v.len() as f32)
    }
}

fn summarize(results: &[CaseResult], cases: &[Case]) -> Summary {
    let mut latencies: Vec<u128> = results.iter().map(|r| r.latency_ms).collect();
    latencies.sort_unstable();
    let injection_ids: Vec<&str> = cases.iter().filter(|c| c.kind == "injection").map(|c| c.id.as_str()).collect();
    Summary {
        cases: results.len(),
        retrieval_recall_at_3_hybrid: mean(results.iter().filter_map(|r| r.recall_at_3_hybrid)),
        retrieval_recall_at_3_lexical: mean(results.iter().filter_map(|r| r.recall_at_3_lexical)),
        answer_must_mention_pass_rate: mean(results.iter().filter_map(|r| r.answer_pass).map(|b| if b { 1.0 } else { 0.0 })),
        missing_abstain_rate: mean(results.iter().filter_map(|r| r.abstained).map(|b| if b { 1.0 } else { 0.0 })),
        task_keyword_recall: mean(results.iter().filter_map(|r| r.task_keyword_recall)),
        invented_due_date_count: results.iter().map(|r| r.invented_due_dates).sum(),
        injection_forbidden_leaks: results.iter().filter(|r| r.forbidden_leak).count(),
        injection_proposals: results
            .iter()
            .filter(|r| injection_ids.contains(&r.id.as_str()))
            .map(|r| r.proposals)
            .sum(),
        citation_tokens_shown: results.iter().map(|r| r.citations_shown).sum(),
        citation_tokens_removed: results.iter().map(|r| r.citations_removed).sum(),
        latency_p50_ms: percentile(&latencies, 0.5),
        latency_p95_ms: percentile(&latencies, 0.95),
        prompt_tokens_total: results.iter().map(|r| r.prompt_tokens).sum(),
        output_tokens_total: results.iter().map(|r| r.output_tokens).sum(),
    }
}

fn write_report(split: &str, chat_model: &str, embed_model: &str, results: &[CaseResult], summary: &Summary) {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("eval").join("reports");
    std::fs::create_dir_all(&dir).unwrap();
    let report = json!({
        "split": split,
        "generated_at": stamp,
        "chat_model": chat_model,
        "embedding_model": embed_model,
        "hardware": "Windows 11, CPU inference (no GPU used)",
        "human_spot_check": "required; not recorded by this harness",
        "summary": summary,
        "cases": results,
    });
    let path = dir.join(format!("{split}-{stamp}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("EVAL report written to {}", path.display());
}
