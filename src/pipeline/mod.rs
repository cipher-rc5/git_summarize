// file: src/pipeline/mod.rs
// description: pipeline module exports and public api
// reference: pipeline orchestration

// These modules are currently disabled as they depend on removed infosec extractors
// mod orchestrator;
// mod processor;
mod progress;

// pub use orchestrator::PipelineOrchestrator;
// pub use processor::{FileProcessor, ProcessingResult};
pub use progress::{PipelineStats, ProgressTracker};
