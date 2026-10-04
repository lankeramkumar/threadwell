use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors returned to the frontend. Only the code and a user-safe message cross
/// the IPC boundary; database and file details stay in the backend.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0} was not found")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("No workspace is open")]
    NoWorkspace,
    #[error("A database error occurred. Check the diagnostics log for details.")]
    Db(#[from] rusqlite::Error),
    #[error("A file operation failed. Check that the location is accessible.")]
    Io(#[from] std::io::Error),
    #[error("Stored data could not be read as JSON")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "validation",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::NoWorkspace => "no_workspace",
            AppError::Db(_) => "database",
            AppError::Io(_) => "io",
            AppError::Json(_) => "data",
        }
    }
}

#[derive(Serialize)]
struct ErrorPayload {
    code: &'static str,
    message: String,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let AppError::Db(err) = self {
            eprintln!("[threadwell] database error: {err}");
        }
        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;

pub fn validation<T>(message: impl Into<String>) -> AppResult<T> {
    Err(AppError::Validation(message.into()))
}
