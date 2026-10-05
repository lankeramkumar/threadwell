//! Optional local performance traces, using OpenTelemetry.
//!
//! Off by default. When the user turns on "keep local performance traces" in Settings, the app
//! writes OpenTelemetry spans for runs, agent steps, model calls and tool calls to a JSON-lines
//! file in its data folder. Nothing is sent over the network. The setting takes effect on the
//! next start, because the tracing subscriber is installed once per process.
//!
//! Privacy: spans carry names, step numbers, tool names, token counts, durations and outcomes.
//! Prompts, answers, page text, tool arguments and search snippets are never recorded.

use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::commands::AppState;
use crate::error::AppResult;

use opentelemetry::trace::{SpanId, Status};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{SdkTracerProvider, SimpleSpanProcessor, SpanData, SpanExporter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const SETTINGS_FILE: &str = "telemetry.json";
const TRACE_DIR: &str = "traces";
const TRACE_FILE: &str = "threadwell-traces.jsonl";
const MAX_TRACE_BYTES: u64 = 5 * 1024 * 1024;

static INSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    pub local_traces: bool,
}

pub fn read_settings(config_dir: &Path) -> TelemetrySettings {
    fs::read_to_string(config_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn write_settings(config_dir: &Path, settings: TelemetrySettings) -> std::io::Result<()> {
    fs::create_dir_all(config_dir)?;
    fs::write(config_dir.join(SETTINGS_FILE), serde_json::to_string_pretty(&settings).unwrap_or_default())
}

pub fn trace_file(config_dir: &Path) -> PathBuf {
    config_dir.join(TRACE_DIR).join(TRACE_FILE)
}

/// Writes finished spans to a local JSON-lines file. Rotates to a single backup when the file
/// reaches the size limit, so the disk use is bounded.
#[derive(Debug)]
struct LocalFileExporter {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl LocalFileExporter {
    fn write(&self, batch: &[SpanData]) -> OTelSdkResult {
        let _guard = self.write_lock.lock().map_err(|_| opentelemetry_sdk::error::OTelSdkError::InternalFailure("lock".into()))?;
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if fs::metadata(&self.path).map(|m| m.len() > MAX_TRACE_BYTES).unwrap_or(false) {
            let backup = self.path.with_extension("jsonl.1");
            let _ = fs::rename(&self.path, backup);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| opentelemetry_sdk::error::OTelSdkError::InternalFailure("trace file".into()))?;
        for span in batch {
            let line = span_to_json(span);
            writeln!(file, "{line}").map_err(|_| opentelemetry_sdk::error::OTelSdkError::InternalFailure("write".into()))?;
        }
        Ok(())
    }
}

impl SpanExporter for LocalFileExporter {
    fn export(&self, batch: Vec<SpanData>) -> impl Future<Output = OTelSdkResult> + Send {
        let result = self.write(&batch);
        async move { result }
    }
}

fn unix_nanos(time: std::time::SystemTime) -> u128 {
    time.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

/// OTLP-shaped JSON for one span. Attribute values are whatever the instrumentation recorded.
pub fn span_to_json(span: &SpanData) -> Value {
    let attributes: serde_json::Map<String, Value> = span
        .attributes
        .iter()
        .map(|kv| (kv.key.as_str().to_string(), json!(kv.value.to_string())))
        .collect();
    let status = match &span.status {
        Status::Error { description } => json!({ "code": "error", "message": description.to_string() }),
        Status::Ok => json!({ "code": "ok" }),
        Status::Unset => json!({ "code": "unset" }),
    };
    json!({
        "traceId": span.span_context.trace_id().to_string(),
        "spanId": span.span_context.span_id().to_string(),
        "parentSpanId": if span.parent_span_id == SpanId::INVALID { String::new() } else { span.parent_span_id.to_string() },
        "name": span.name.to_string(),
        "startTimeUnixNano": unix_nanos(span.start_time).to_string(),
        "endTimeUnixNano": unix_nanos(span.end_time).to_string(),
        "attributes": attributes,
        "status": status,
    })
}

/// Installs the tracing subscriber with the OpenTelemetry layer, if local traces are enabled.
/// Returns whether traces are being recorded. Safe to call more than once; only the first call
/// installs anything.
pub fn init(config_dir: &Path) -> bool {
    if !read_settings(config_dir).local_traces {
        return false;
    }
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return true;
    }
    let exporter = LocalFileExporter { path: trace_file(config_dir), write_lock: Mutex::new(()) };
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter))
        .build();
    let tracer = provider.tracer("threadwell");
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let _ = tracing_subscriber::registry().with(layer).try_init();
    opentelemetry::global::set_tracer_provider(provider);
    true
}

/// Removes recorded traces, including the rotated backup.
pub fn delete_traces(config_dir: &Path) -> std::io::Result<()> {
    let path = trace_file(config_dir);
    let _ = fs::remove_file(path.with_extension("jsonl.1"));
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn telemetry_get_settings(state: State<'_, AppState>) -> AppResult<TelemetrySettings> {
    Ok(read_settings(&state.config_dir))
}

/// Saves the setting. It takes effect on the next start.
#[tauri::command]
pub async fn telemetry_set_settings(state: State<'_, AppState>, local_traces: bool) -> AppResult<()> {
    write_settings(&state.config_dir, TelemetrySettings { local_traces })?;
    Ok(())
}

#[tauri::command]
pub async fn telemetry_delete_traces(state: State<'_, AppState>) -> AppResult<()> {
    delete_traces(&state.config_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_is_off_until_enabled() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!read_settings(dir.path()).local_traces);
        write_settings(dir.path(), TelemetrySettings { local_traces: true }).unwrap();
        assert!(read_settings(dir.path()).local_traces);
    }

    #[test]
    fn deleting_traces_removes_the_file_and_its_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = trace_file(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}\n").unwrap();
        fs::write(path.with_extension("jsonl.1"), "{}\n").unwrap();
        delete_traces(dir.path()).unwrap();
        assert!(!path.exists());
        assert!(!path.with_extension("jsonl.1").exists());
        assert!(delete_traces(dir.path()).is_ok(), "deleting twice is not an error");
    }

    #[test]
    fn spans_reach_the_local_file_and_private_text_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = trace_file(dir.path());
        let provider = SdkTracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(LocalFileExporter { path: path.clone(), write_lock: Mutex::new(()) }))
            .build();
        let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            let run = tracing::info_span!("agent.run", architecture = "single");
            let _run = run.enter();
            let tool = tracing::info_span!("tool.call", tool = %"search_workspace", ok = tracing::field::Empty);
            let _tool = tool.enter();
            tool.record("ok", true);
            // Text that must never appear in a trace, even if it were passed to a span.
            tracing::info!(content = "PRIVATE-PAGE-TEXT", "an event, not a span");
        });
        let _ = provider.shutdown();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("tool.call"));
        assert!(text.contains("agent.run"));
        assert!(!text.contains("PRIVATE-PAGE-TEXT"));
        let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert!(first.get("traceId").is_some() && first.get("spanId").is_some());
    }
}
