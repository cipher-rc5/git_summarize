// file: src/utils/telemetry.rs
// description: Telemetry and observability utilities for production monitoring
// reference: Production observability best practices

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Health status for various system components
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Health check result for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub component: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

impl HealthCheck {
    pub fn healthy(component: &str, response_time: Duration) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Healthy,
            message: None,
            response_time_ms: response_time.as_millis() as u64,
        }
    }

    pub fn degraded(component: &str, message: String, response_time: Duration) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Degraded,
            message: Some(message),
            response_time_ms: response_time.as_millis() as u64,
        }
    }

    pub fn unhealthy(component: &str, message: String, response_time: Duration) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(message),
            response_time_ms: response_time.as_millis() as u64,
        }
    }
}

/// Overall system health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub overall_status: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub timestamp: u64,
    pub version: String,
}

impl HealthReport {
    pub fn new(checks: Vec<HealthCheck>, version: String) -> Self {
        let overall_status = if checks.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if checks.iter().any(|c| c.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        Self {
            overall_status,
            checks,
            timestamp,
            version,
        }
    }

    pub fn format(&self) -> String {
        let status_icon = match self.overall_status {
            HealthStatus::Healthy => "✓",
            HealthStatus::Degraded => "⚠",
            HealthStatus::Unhealthy => "✗",
        };

        let mut output = format!(
            "{} System Health: {:?}\n\
             Version: {}\n\
             Timestamp: {}\n\n",
            status_icon,
            self.overall_status,
            self.version,
            chrono::DateTime::from_timestamp(self.timestamp as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );

        for check in &self.checks {
            let check_icon = match check.status {
                HealthStatus::Healthy => "✓",
                HealthStatus::Degraded => "⚠",
                HealthStatus::Unhealthy => "✗",
            };

            output.push_str(&format!(
                "{} {} ({:?}) - {}ms",
                check_icon,
                check.component,
                check.status,
                check.response_time_ms
            ));

            if let Some(ref msg) = check.message {
                output.push_str(&format!("\n  {}", msg));
            }

            output.push('\n');
        }

        output
    }
}

/// Operation timer for performance tracking
pub struct OperationTimer {
    operation: String,
    start: Instant,
}

impl OperationTimer {
    pub fn new(operation: &str) -> Self {
        info!("Starting operation: {}", operation);
        Self {
            operation: operation.to_string(),
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn finish(self) -> Duration {
        let elapsed = self.elapsed();
        info!(
            "Completed operation: {} in {:.2}s",
            self.operation,
            elapsed.as_secs_f64()
        );
        elapsed
    }

    pub fn finish_with_count(self, count: usize) -> Duration {
        let elapsed = self.elapsed();
        info!(
            "Completed operation: {} - {} items in {:.2}s ({:.2} items/sec)",
            self.operation,
            count,
            elapsed.as_secs_f64(),
            if elapsed.as_secs_f64() > 0.0 {
                count as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            }
        );
        elapsed
    }

    pub fn checkpoint(&self, message: &str) {
        let elapsed = self.elapsed();
        info!(
            "Operation checkpoint [{}]: {} at {:.2}s",
            self.operation,
            message,
            elapsed.as_secs_f64()
        );
    }

    pub fn warn_if_slow(&self, threshold: Duration, message: &str) {
        let elapsed = self.elapsed();
        if elapsed > threshold {
            warn!(
                "Slow operation [{}]: {} took {:.2}s (threshold: {:.2}s)",
                self.operation,
                message,
                elapsed.as_secs_f64(),
                threshold.as_secs_f64()
            );
        }
    }
}

/// Performance metrics for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub count: usize,
    pub duration_ms: u64,
    pub throughput: f64, // items per second
    pub avg_item_time_ms: f64,
}

impl PerformanceMetrics {
    pub fn new(operation: &str, count: usize, duration: Duration) -> Self {
        let duration_ms = duration.as_millis() as u64;
        let duration_secs = duration.as_secs_f64();

        let throughput = if duration_secs > 0.0 {
            count as f64 / duration_secs
        } else {
            0.0
        };

        let avg_item_time_ms = if count > 0 {
            duration_ms as f64 / count as f64
        } else {
            0.0
        };

        Self {
            operation: operation.to_string(),
            count,
            duration_ms,
            throughput,
            avg_item_time_ms,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "{}: {} items in {}ms ({:.2} items/sec, {:.2}ms per item)",
            self.operation,
            self.count,
            self.duration_ms,
            self.throughput,
            self.avg_item_time_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_creation() {
        let check = HealthCheck::healthy("database", Duration::from_millis(50));
        assert_eq!(check.component, "database");
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.response_time_ms, 50);
    }

    #[test]
    fn test_health_report_overall_status() {
        let checks = vec![
            HealthCheck::healthy("db", Duration::from_millis(10)),
            HealthCheck::degraded("cache", "slow".to_string(), Duration::from_millis(100)),
        ];

        let report = HealthReport::new(checks, "0.1.0".to_string());
        assert_eq!(report.overall_status, HealthStatus::Degraded);
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new("test_op", 100, Duration::from_secs(10));
        assert_eq!(metrics.count, 100);
        assert_eq!(metrics.throughput, 10.0);
        assert_eq!(metrics.avg_item_time_ms, 100.0);
    }

    #[test]
    fn test_operation_timer() {
        let timer = OperationTimer::new("test");
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.finish();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
