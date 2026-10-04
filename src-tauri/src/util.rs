use crate::error::{validation, AppResult};

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn validate_id(id: &str) -> AppResult<()> {
    match uuid::Uuid::parse_str(id) {
        Ok(_) if id.len() == 36 => Ok(()),
        _ => validation("Invalid identifier"),
    }
}

/// Trims and checks a single-line name such as a page or task title.
pub fn validate_line(value: &str, field: &str, max_chars: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return validation(format!("{field} cannot be empty"));
    }
    if trimmed.chars().count() > max_chars {
        return validation(format!("{field} must be at most {max_chars} characters"));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return validation(format!("{field} cannot contain control characters"));
    }
    Ok(trimmed.to_string())
}

/// Ensures a path supplied by the frontend is absolute before it is used.
pub fn validate_abs_path(path: &str) -> AppResult<std::path::PathBuf> {
    let p = std::path::PathBuf::from(path);
    if path.trim().is_empty() || !p.is_absolute() {
        return validation("A full file path is required");
    }
    Ok(p)
}
