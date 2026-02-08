/// Errors produced by team orchestration operations.
#[derive(Debug, thiserror::Error)]
pub enum TeamError {
    #[error("{0}")]
    InvalidOperation(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, TeamError>;
