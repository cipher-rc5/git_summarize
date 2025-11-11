// file: src/utils/mod.rs
// description: utility functions module exports
// reference: internal module structure

pub mod logging;
pub mod telemetry;
pub mod template;
pub mod validation;

pub use telemetry::{HealthCheck, HealthReport, HealthStatus, OperationTimer, PerformanceMetrics};
pub use template::FileTemplate;
pub use validation::Validator;
