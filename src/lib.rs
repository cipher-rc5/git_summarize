// file: src/lib.rs
// description: library entry point and public api exports
// reference: rust library patterns
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/readme.md"))]

pub mod cli;
pub mod config;
pub mod database;
pub mod error;
pub mod exporter;
pub mod extractor;
pub mod generation;
pub mod mcp;
pub mod models;
pub mod parser;
pub mod pipeline;
pub mod repository;
pub mod utils;

pub use config::{
    Config, DatabaseConfig, EmbeddingConfig, ExtractionConfig, GenerationConfig, PipelineConfig,
    RepositoryConfig,
};
pub use database::{BatchInserter, EmbeddingClient, InsertStats, LanceDbClient, SchemaManager};
pub use generation::AnswerGenerator;
pub use error::{PipelineError, Result};
pub use exporter::json::{ExportManifest, ExportedDocument, JsonExporter};
pub use models::{Document, SearchResult};
pub use parser::{
    Chunk, ChunkOptions, Frontmatter, FrontmatterParser, MarkdownNormalizer, MarkdownParser,
    ParsedMarkdown, chunk_markdown,
};
pub use pipeline::{PipelineStats, ProgressTracker};
pub use repository::{FileClassifier, FileScanner, RepositorySync, ScannedFile};
pub use utils::{
    FileTemplate, HealthCheck, HealthReport, HealthStatus, OperationTimer, PerformanceMetrics,
    Validator,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        let _config = Config::default_config();
        let _template = FileTemplate::new();
    }
}
