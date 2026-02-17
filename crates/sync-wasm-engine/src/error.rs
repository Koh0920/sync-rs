use thiserror::Error;

/// Errors from the WASM execution engine.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// WASM compilation or execution error.
    #[error("WASM error: {0}")]
    Wasm(String),

    /// Error from the sync-format crate.
    #[error("Sync error: {0}")]
    Sync(#[from] sync_format::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(String),

    /// Permission denied for a host function operation.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Error within a host function.
    #[error("Host function error: {0}")]
    HostFunction(String),

    /// Execution timed out.
    #[error("Execution timeout")]
    Timeout,

    /// Invalid input parameter.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Result type for sync-wasm-engine operations.
pub type Result<T> = std::result::Result<T, Error>;
