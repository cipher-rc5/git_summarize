// file: src/pipeline/mod.rs
// description: pipeline module exports and public api
// reference: lazarus ingest pipeline orchestration

mod orchestrator;
mod processor;
mod progress;

pub use orchestrator::PipelineOrchestrator;
pub use processor::{FileProcessor, ProcessingResult};
pub use progress::{PipelineStats, ProgressTracker};
