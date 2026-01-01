//! Custom error types for the patcher

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PatcherError {
    #[error("Failed to download build: {0}")]
    DownloadError(String),

    #[error("Failed to extract archive: {0}")]
    ExtractionError(String),

    #[error("Failed to parse YAML: {0}")]
    YamlParseError(String),

    #[error("Invalid build info: {0}")]
    InvalidBuildInfo(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("ASAR extraction failed: {0}")]
    AsarError(String),

    #[error("Patching failed: {0}")]
    PatchError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}
