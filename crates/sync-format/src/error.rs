use thiserror::Error;

/// Errors that can occur when working with `.sync` archives.
#[derive(Debug, Error)]
pub enum Error {
    /// The archive format is invalid.
    #[error("invalid sync format: {0}")]
    InvalidFormat(String),

    /// A required entry is missing from the archive.
    #[error("missing required entry: {0}")]
    MissingEntry(String),

    /// Error parsing or validating the manifest.
    #[error("manifest error: {0}")]
    ManifestError(String),

    /// Payload hash verification failed.
    #[error("payload hash mismatch")]
    HashMismatch,

    /// Payload entry not found in the archive.
    #[error("payload not found in archive")]
    PayloadNotFound,

    /// Decryption error (requires `encryption` feature).
    #[error("decryption error: {0}")]
    DecryptError(String),

    /// Encryption error (requires `encryption` feature).
    #[error("encryption error: {0}")]
    EncryptError(String),

    /// Error from the zip library.
    #[error("zip error: {0}")]
    ZipError(#[from] zip::result::ZipError),

    /// I/O error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// TOML parsing error.
    #[error("toml parsing error: {0}")]
    TomlError(String),

    /// JSON parsing error.
    #[error("json parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Date/time parsing error.
    #[error("chrono error: {0}")]
    ChronoError(#[from] chrono::ParseError),
}

/// Result type for sync-format operations.
pub type Result<T> = std::result::Result<T, Error>;
