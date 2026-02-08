/// Errors produced by provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("api error: {0}")]
    ApiError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ProviderError>;
