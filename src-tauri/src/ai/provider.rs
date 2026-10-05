//! Ollama adapter. The rest of the app depends only on `ChatProvider`-shaped behaviour
//! (`check` and `chat_stream`), so a cloud adapter can be added later without touching
//! the agent loop or the editor.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    Unreachable,
    ModelMissing,
    Timeout,
    Cancelled,
    Protocol(String),
}

impl ProviderError {
    pub fn category(&self) -> &'static str {
        match self {
            ProviderError::Unreachable => "provider_unreachable",
            ProviderError::ModelMissing => "model_missing",
            ProviderError::Timeout => "provider_timeout",
            ProviderError::Cancelled => "cancelled",
            ProviderError::Protocol(_) => "provider_protocol",
        }
    }

    pub fn user_message(&self, endpoint: &str, model: &str) -> String {
        match self {
            ProviderError::Unreachable => format!(
                "No model server answered at {endpoint}. Start Ollama, or check the endpoint in AI settings."
            ),
            ProviderError::ModelMissing => {
                format!("The model \"{model}\" is not installed. Run `ollama pull {model}` and try again.")
            }
            ProviderError::Timeout => "The model took too long to respond. Try again or use a smaller model.".into(),
            ProviderError::Cancelled => "The run was cancelled.".into(),
            ProviderError::Protocol(detail) => format!("The model server returned an unexpected response ({detail})."),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Default)]
pub struct ChatReply {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub prompt_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum Readiness {
    Ready,
    ModelMissing,
    Unreachable,
}

pub struct OllamaClient {
    base: String,
    model: String,
    agent: ureq::Agent,
}

impl OllamaClient {
    pub fn new(base: &str, model: &str) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(3)))
            .timeout_recv_response(Some(Duration::from_secs(180)))
            .timeout_recv_body(Some(Duration::from_secs(180)))
            .http_status_as_error(false)
            .build();
        Self {
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            agent: config.into(),
        }
    }

    /// Embeds each input with `model` through `/api/embed`. Every vector must have the same
    /// positive dimension, or the reply is rejected.
    pub fn embed(&self, model: &str, inputs: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        let body = json!({ "model": model, "input": inputs });
        let mut response = self
            .agent
            .post(&format!("{}/api/embed", self.base))
            .send_json(&body)
            .map_err(map_transport)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Err(ProviderError::ModelMissing);
        }
        if !(200..300).contains(&status) {
            return Err(ProviderError::Protocol(format!("HTTP {status}")));
        }
        let text = response.body_mut().read_to_string().map_err(|_| ProviderError::Unreachable)?;
        let parsed: Value = serde_json::from_str(&text).map_err(|_| ProviderError::Protocol("invalid embedding reply".into()))?;
        let rows = parsed
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Protocol("embedding reply has no vectors".into()))?;
        if rows.len() != inputs.len() {
            return Err(ProviderError::Protocol("embedding count does not match input count".into()));
        }
        let mut out = Vec::with_capacity(rows.len());
        let mut dims = None;
        for row in rows {
            let vector: Vec<f32> = row
                .as_array()
                .ok_or_else(|| ProviderError::Protocol("embedding is not an array".into()))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(f64::NAN) as f32)
                .collect();
            if vector.is_empty() || vector.iter().any(|f| !f.is_finite()) {
                return Err(ProviderError::Protocol("embedding contains invalid values".into()));
            }
            match dims {
                None => dims = Some(vector.len()),
                Some(d) if d != vector.len() => return Err(ProviderError::Protocol("mixed embedding sizes".into())),
                _ => {}
            }
            out.push(vector);
        }
        Ok(out)
    }

    /// Non-streaming chat for structured work. With `json_mode`, the server is asked for a JSON
    /// object. Returns the full reply and token counts.
    pub fn chat_once(&self, messages: &[Value], json_mode: bool, cancel: &AtomicBool) -> Result<ChatReply, ProviderError> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
            "options": { "temperature": 0.1 },
        });
        if json_mode {
            body["format"] = json!("json");
        }
        let mut response = self
            .agent
            .post(&format!("{}/api/chat", self.base))
            .send_json(&body)
            .map_err(map_transport)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Err(ProviderError::ModelMissing);
        }
        if !(200..300).contains(&status) {
            return Err(ProviderError::Protocol(format!("HTTP {status}")));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(ProviderError::Cancelled);
        }
        let text = response.body_mut().read_to_string().map_err(|_| ProviderError::Unreachable)?;
        let parsed: Value = serde_json::from_str(&text).map_err(|_| ProviderError::Protocol("invalid reply".into()))?;
        if let Some(error) = parsed.get("error").and_then(Value::as_str) {
            return Err(ProviderError::Protocol(truncate(error, 120)));
        }
        Ok(ChatReply {
            content: parsed.pointer("/message/content").and_then(Value::as_str).unwrap_or("").to_string(),
            tool_calls: Vec::new(),
            prompt_tokens: parsed.get("prompt_eval_count").and_then(Value::as_u64),
            output_tokens: parsed.get("eval_count").and_then(Value::as_u64),
        })
    }

    pub fn describe(&self) -> (String, String) {
        (self.base.clone(), self.model.clone())
    }

    /// Checks that the server answers and that the configured model is installed.
    pub fn check(&self) -> Readiness {
        let Ok(mut response) = self.agent.get(&format!("{}/api/tags", self.base)).call() else {
            return Readiness::Unreachable;
        };
        if response.status().as_u16() != 200 {
            return Readiness::Unreachable;
        }
        let text = response.body_mut().read_to_string().unwrap_or_default();
        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let installed = parsed
            .get("models")
            .and_then(Value::as_array)
            .map(|models| {
                models.iter().any(|m| {
                    let name = m.get("name").and_then(Value::as_str).unwrap_or("");
                    name == self.model || (!self.model.contains(':') && name == format!("{}:latest", self.model))
                })
            })
            .unwrap_or(false);
        if installed {
            Readiness::Ready
        } else {
            Readiness::ModelMissing
        }
    }

    /// Streams one chat completion. `on_text` receives text deltas as they arrive. Tool
    /// calls are collected and returned when the stream finishes.
    pub fn chat_stream(
        &self,
        messages: &[Value],
        tools: Option<&Value>,
        cancel: &AtomicBool,
        mut on_text: impl FnMut(&str),
    ) -> Result<ChatReply, ProviderError> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "options": { "temperature": 0.2 },
        });
        if let Some(tools) = tools {
            body["tools"] = tools.clone();
        }
        let response = self
            .agent
            .post(&format!("{}/api/chat", self.base))
            .send_json(&body)
            .map_err(map_transport)?;
        let status = response.status().as_u16();
        if status == 404 {
            return Err(ProviderError::ModelMissing);
        }
        if !(200..300).contains(&status) {
            return Err(ProviderError::Protocol(format!("HTTP {status}")));
        }

        let reader = BufReader::new(response.into_body().into_reader());
        let mut reply = ChatReply::default();
        for line in reader.lines() {
            if cancel.load(Ordering::SeqCst) {
                return Err(ProviderError::Cancelled);
            }
            let line = line.map_err(|_| ProviderError::Unreachable)?;
            if line.trim().is_empty() {
                continue;
            }
            let chunk: Value = serde_json::from_str(&line)
                .map_err(|_| ProviderError::Protocol("invalid stream line".into()))?;
            if let Some(error) = chunk.get("error").and_then(Value::as_str) {
                return Err(ProviderError::Protocol(truncate(error, 120)));
            }
            if let Some(text) = chunk.pointer("/message/content").and_then(Value::as_str) {
                if !text.is_empty() {
                    reply.content.push_str(text);
                    on_text(text);
                }
            }
            if let Some(calls) = chunk.pointer("/message/tool_calls").and_then(Value::as_array) {
                for call in calls {
                    let name = call.pointer("/function/name").and_then(Value::as_str).unwrap_or("").to_string();
                    let arguments = match call.pointer("/function/arguments") {
                        Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or(Value::Null),
                        Some(other) => other.clone(),
                        None => Value::Null,
                    };
                    reply.tool_calls.push(ToolCall { name, arguments });
                }
            }
            if chunk.get("done").and_then(Value::as_bool) == Some(true) {
                reply.prompt_tokens = chunk.get("prompt_eval_count").and_then(Value::as_u64);
                reply.output_tokens = chunk.get("eval_count").and_then(Value::as_u64);
                break;
            }
        }
        Ok(reply)
    }
}

fn map_transport(error: ureq::Error) -> ProviderError {
    match error {
        ureq::Error::Timeout(_) => ProviderError::Timeout,
        _ => ProviderError::Unreachable,
    }
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
