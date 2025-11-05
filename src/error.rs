// file: src/error.rs
// description: custom error types and result type aliases
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

    #[error("Git open error: {0}")]
    GitOpen(#[source] Box<gix::open::Error>),

    #[error("Git clone error: {0}")]
    GitClone(#[source] Box<gix::clone::Error>),

    #[error("Git connect error: {0}")]
    GitConnect(#[source] Box<gix::remote::connect::Error>),

    #[error("Git fetch prepare error: {0}")]
    GitFetchPrepare(#[source] Box<gix::remote::fetch::prepare::Error>),

    #[error("Git find reference error: {0}")]
    GitFindReference(#[source] Box<gix::reference::find::Error>),

    #[error("Git find existing reference error: {0}")]
    GitFindExistingReference(#[source] Box<gix::reference::find::existing::Error>),

    #[error("Git find object error: {0}")]
    GitFindObject(#[source] Box<gix::object::find::existing::Error>),

    #[error("Git object conversion error: {0}")]
    GitObjectConversion(#[source] Box<gix::object::try_into::Error>),

    #[error("Git reference edit error: {0}")]
    GitReferenceEdit(#[source] Box<gix::reference::edit::Error>),

    #[error("Git worktree checkout error: {0}")]
    GitWorktreeCheckout(#[source] Box<gix_worktree_state::checkout::Error>),

    #[error("Git peel error: {0}")]
    GitPeel(#[source] Box<gix::object::peel::to_kind::Error>),

    #[error("Git reference error: {0}")]
    GitReference(String),

    #[error("Git object error: {0}")]
    GitObject(String),

    #[error("Git worktree error: {0}")]
    GitWorktree(String),
}

// Additional helper implementations for better error ergonomics
impl PipelineError {
    pub fn git_ref_error<S: Into<String>>(msg: S) -> Self {
        PipelineError::GitReference(msg.into())
    }

    pub fn git_object_error<S: Into<String>>(msg: S) -> Self {
        PipelineError::GitObject(msg.into())
    }

    pub fn git_worktree_error<S: Into<String>>(msg: S) -> Self {
        PipelineError::GitWorktree(msg.into())
    }
}

macro_rules! impl_from_gix_error {
    ($variant:ident, $err:path) => {
        impl From<$err> for PipelineError {
            fn from(error: $err) -> Self {
                PipelineError::$variant(Box::new(error))
            }
        }
    };
}

impl_from_gix_error!(GitOpen, gix::open::Error);
impl_from_gix_error!(GitClone, gix::clone::Error);
impl_from_gix_error!(GitConnect, gix::remote::connect::Error);
impl_from_gix_error!(
    GitFetchPrepare,
    gix::remote::fetch::prepare::Error
);
impl_from_gix_error!(GitFindReference, gix::reference::find::Error);
impl_from_gix_error!(
    GitFindExistingReference,
    gix::reference::find::existing::Error
);
impl_from_gix_error!(
    GitFindObject,
    gix::object::find::existing::Error
);
impl_from_gix_error!(
    GitObjectConversion,
    gix::object::try_into::Error
);
impl_from_gix_error!(GitReferenceEdit, gix::reference::edit::Error);
impl_from_gix_error!(
    GitWorktreeCheckout,
    gix_worktree_state::checkout::Error
);
impl_from_gix_error!(GitPeel, gix::object::peel::to_kind::Error);
