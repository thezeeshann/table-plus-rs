use serde::Serialize;

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

/// Serialized shape sent to the frontend. Postgres errors carry extra detail so
/// the query tab can show position / hint / detail like TablePlus does.
#[derive(Serialize)]
pub struct ErrorPayload {
    pub message: String,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub position: Option<u32>,
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let payload = match self {
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
        };
        payload.serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
