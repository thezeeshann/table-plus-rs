use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("database error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("connection not found: {0}")]
    ConnectionNotFound(String),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Message(s.into())
    }
}

/// Display shape for errors. Postgres errors carry extra detail so the query tab can
/// show position / hint / detail like TablePlus does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub message: String,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub position: Option<u32>,
}

impl AppError {
    pub fn payload(&self) -> ErrorPayload {
        match self {
            AppError::Postgres(e) => {
                if let Some(db) = e.as_db_error() {
                    ErrorPayload {
                        message: db.message().to_string(),
                        code: Some(db.code().code().to_string()),
                        detail: db.detail().map(|s| s.to_string()),
                        hint: db.hint().map(|s| s.to_string()),
                        position: db.position().and_then(|p| match p {
                            tokio_postgres::error::ErrorPosition::Original(n) => Some(*n),
                            tokio_postgres::error::ErrorPosition::Internal { position, .. } => Some(*position),
                        }),
                    }
                } else {
                    ErrorPayload { message: e.to_string(), code: None, detail: None, hint: None, position: None }
                }
            }
            other => ErrorPayload { message: other.to_string(), code: None, detail: None, hint: None, position: None },
        }
    }
}

impl ErrorPayload {
    /// One line for toasts: message plus hint when there is one.
    pub fn one_line(&self) -> String {
        match &self.hint {
            Some(h) => format!("{} (hint: {h})", self.message),
            None => self.message.clone(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
