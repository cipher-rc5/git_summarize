// file: src/lib.rs
// description: Library entry point and public API exports
// reference: Rust library patterns
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/readme.md"))]

pub mod config;
pub mod database;
pub mod error;
pub mod extractor;
pub mod models;
pub mod parser;
pub mod pipeline;
pub mod repository;
pub mod utils;

pub use config::{Config, DatabaseConfig, ExtractionConfig, PipelineConfig, RepositoryConfig};
pub use database::{BatchInserter, ClickHouseClient, InsertStats, SchemaManager};
pub use error::{PipelineError, Result};
pub use extractor::{CryptoExtractor, IncidentExtractor, IocExtractor};
pub use models::{
    ChainType, CryptoAddress, DatePrecision, Document, Incident, IncidentBuilder, Ioc, IocType,
};
pub use parser::{
    Frontmatter, FrontmatterParser, MarkdownNormalizer, MarkdownParser, ParsedMarkdown,
};
pub use pipeline::{
    FileProcessor, PipelineOrchestrator, PipelineStats, ProcessingResult, ProgressTracker,
};
pub use repository::{FileClassifier, FileScanner, RepositorySync, ScannedFile};
pub use utils::{FileTemplate, Validator};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        let _config = Config::default_config();
        let _template = FileTemplate::new();
    }
}
