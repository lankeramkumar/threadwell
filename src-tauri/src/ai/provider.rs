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
