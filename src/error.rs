// file: src/error.rs
// description: Custom error types and result type aliases
// reference: https://docs.rs/thiserror

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PipelineError>;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Repository sync failed: {0}")]
    RepositorySync(String),

    #[error("File operation failed for {path}: {source}")]
    FileOperation {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Markdown parsing error in {file}: {message}")]
    MarkdownParse { file: String, message: String },

    #[error("Database error: {0}")]
    Database(#[from] clickhouse::error::Error),

    #[error("Extraction error: {0}")]
    Extraction(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
}
