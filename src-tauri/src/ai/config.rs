//! Provider settings. Stored in the workspace settings table. Only loopback endpoints are
//! accepted unless the user turns on remote endpoints, and remote ones must use https.

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{validation, AppResult};
use crate::workspace::{get_setting, set_setting};

pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";
const MAX_MODEL_CHARS: usize = 100;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
    pub allow_remote: bool,
}

pub fn load(conn: &Connection) -> AppResult<AiConfig> {
    let endpoint = get_setting(conn, "ai.endpoint")?.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
    let model = get_setting(conn, "ai.model")?.unwrap_or_default();
    let allow_remote = get_setting(conn, "ai.allow_remote")?.as_deref() == Some("true");
    Ok(AiConfig { endpoint, model, allow_remote })
}

pub fn save(conn: &Connection, endpoint: &str, model: &str, allow_remote: bool) -> AppResult<AiConfig> {
    let endpoint = validate_endpoint(endpoint, allow_remote)?;
    let model = validate_model(model)?;
    set_setting(conn, "ai.endpoint", &endpoint)?;
    set_setting(conn, "ai.model", &model)?;
    set_setting(conn, "ai.allow_remote", if allow_remote { "true" } else { "false" })?;
    Ok(AiConfig { endpoint, model, allow_remote })
}

/// Accepts `http://host[:port]` or `https://host[:port]` with no path, query, userinfo or
/// whitespace. Returns the normalized base URL.
pub fn validate_endpoint(raw: &str, allow_remote: bool) -> AppResult<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else {
        return validation("The endpoint must start with http:// or https://");
    };
    if rest.is_empty() || rest.chars().any(|c| c.is_whitespace() || matches!(c, '@' | '?' | '#' | '\\')) {
        return validation("The endpoint contains characters that are not allowed");
    }
    if rest.contains('/') {
        return validation("Enter only the server address, without a path");
    }
    let host = if rest.starts_with('[') {
        match rest.find(']') {
            Some(end) => &rest[..=end],
            None => return validation("The IPv6 address in the endpoint is not closed"),
        }
    } else {
        rest.split(':').next().unwrap_or("")
    };
    let port = &rest[host.len()..];
    if !port.is_empty() {
        match port.strip_prefix(':').map(str::parse::<u16>) {
            Some(Ok(_)) => {}
            _ => return validation("The port in the endpoint is not valid"),
        }
    }
    let lower = host.to_ascii_lowercase();
    let loopback = lower == "localhost" || lower.ends_with(".localhost") || lower == "127.0.0.1" || lower == "[::1]";
    if !loopback {
        if !allow_remote {
            return validation(format!(
                "{host} is not this computer. Turn on remote endpoints in AI settings to use it."
            ));
        }
        if scheme != "https" {
            return validation("Remote endpoints must use https");
        }
    }
    Ok(format!("{scheme}://{rest}"))
}

pub fn validate_model(raw: &str) -> AppResult<String> {
    let model = raw.trim();
    if model.is_empty() {
        return Ok(String::new());
    }
    if model.chars().count() > MAX_MODEL_CHARS {
        return validation("The model name is too long");
    }
    let allowed = model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'));
    if !allowed {
        return validation("The model name may contain only letters, numbers and . _ - : /");
    }
    Ok(model.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_endpoints() {
        assert_eq!(validate_endpoint("http://127.0.0.1:11434/", false).unwrap(), "http://127.0.0.1:11434");
        assert_eq!(validate_endpoint("http://localhost:11434", false).unwrap(), "http://localhost:11434");
        assert_eq!(validate_endpoint("http://[::1]:11434", false).unwrap(), "http://[::1]:11434");
        assert!(validate_endpoint("http://api.localhost:11434", false).is_ok());
    }

    #[test]
    fn refuses_remote_endpoints_by_default() {
        assert!(validate_endpoint("http://example.com:11434", false).is_err());
        assert!(validate_endpoint("https://example.com", false).is_err());
    }

    #[test]
    fn remote_endpoints_need_opt_in_and_https() {
        assert!(validate_endpoint("https://models.example.com", true).is_ok());
        assert!(validate_endpoint("http://models.example.com", true).is_err());
    }

    #[test]
    fn rejects_lookalike_hosts_and_smuggled_parts() {
        // A loopback-looking prefix must not pass as loopback.
        assert!(validate_endpoint("http://127.0.0.1.evil.com:11434", false).is_err());
        assert!(validate_endpoint("http://localhost.evil.com", false).is_err());
        assert!(validate_endpoint("http://127.0.0.1@evil.com", false).is_err());
        assert!(validate_endpoint("http://127.0.0.1:11434/api", false).is_err());
        assert!(validate_endpoint("file:///etc/passwd", false).is_err());
        assert!(validate_endpoint("http://127.0.0.1:99999", false).is_err());
    }

    #[test]
    fn model_names_are_restricted() {
        assert_eq!(validate_model("qwen2.5:3b").unwrap(), "qwen2.5:3b");
        assert!(validate_model("model; rm -rf").is_err());
        assert!(validate_model("a\"b").is_err());
    }
}
