// file: src/repository/mod.rs
// description: repository operations module exports
// reference: internal module structure

pub mod classifier;
pub mod scanner;
pub mod sync;

pub use classifier::FileClassifier;
pub use scanner::{FileScanner, ScannedFile};
pub use sync::RepositorySync;
