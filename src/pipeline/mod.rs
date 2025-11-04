// file: src/pipeline/mod.rs
// description: Pipeline module exports and public API
// reference: Lazarus BlueNoroff ingestion pipeline orchestration

mod orchestrator;
mod processor;
mod progress;

pub use orchestrator::PipelineOrchestrator;
pub use processor::{FileProcessor, ProcessingResult};
pub use progress::{PipelineStats, ProgressTracker};
