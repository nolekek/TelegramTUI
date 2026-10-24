use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("TDLib binary not found: {0}")]
    LibraryNotFound(String),

    #[error("TDLib dynamic load error: {0}")]
    LibraryLoadError(String),

    #[error("Backend not initialized")]
    BackendNotInitialized,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Telegram RPC error {code}: {message}")]
    RpcError { code: i32, message: String },

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
