// file: src/pipeline/progress.rs
// description: progress tracking and statistics reporting for pipeline execution
// reference: uses indicatif for progress bars and tracks processing metrics

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub files_processed: usize,
    pub files_failed: usize,
    pub documents_created: usize,
    pub total_bytes_processed: u64,
    pub duration_secs: u64,
}

impl PipelineStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn files_per_second(&self) -> f64 {
        if self.duration_secs == 0 {
            return 0.0;
        }
        self.files_processed as f64 / self.duration_secs as f64
    }

    pub fn bytes_per_second(&self) -> f64 {
        if self.duration_secs == 0 {
            return 0.0;
        }
        self.total_bytes_processed as f64 / self.duration_secs as f64
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.files_processed + self.files_failed;
        if total == 0 {
            return 0.0;
        }
        (self.files_processed as f64 / total as f64) * 100.0
    }
}

pub struct ProgressTracker {
    main_bar: ProgressBar,
    detail_bar: ProgressBar,
    files_processed: Arc<AtomicUsize>,
    files_failed: Arc<AtomicUsize>,
    documents_created: Arc<AtomicUsize>,
    bytes_processed: Arc<AtomicU64>,
    start_time: Instant,
}

impl ProgressTracker {
    pub fn new(total_files: usize) -> Self {
        Self::with_color(total_files, true)
    }

    pub fn with_color(total_files: usize, colored: bool) -> Self {
        let multi_progress = MultiProgress::new();

        let main_bar = create_progress_bar(&multi_progress, total_files as u64, colored);
        let detail_bar = create_detail_bar(&multi_progress);

        Self {
            main_bar,
            detail_bar,
            files_processed: Arc::new(AtomicUsize::new(0)),
            files_failed: Arc::new(AtomicUsize::new(0)),
            documents_created: Arc::new(AtomicUsize::new(0)),
            bytes_processed: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn inc_files_processed(&self) {
        self.files_processed.fetch_add(1, Ordering::SeqCst);
        self.main_bar.inc(1);
        self.update_detail_bar();
    }

    pub fn inc_files_failed(&self) {
        self.files_failed.fetch_add(1, Ordering::SeqCst);
        self.main_bar.inc(1);
        self.update_detail_bar();
    }

    pub fn add_document(&self) {
        self.documents_created.fetch_add(1, Ordering::SeqCst);
    }

    pub fn add_bytes_processed(&self, bytes: u64) {
        self.bytes_processed.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn set_message(&self, message: String) {
        self.detail_bar.set_message(message);
    }

    pub fn finish(&self) {
        self.main_bar.finish_with_message("Processing complete");
        self.detail_bar.finish_and_clear();
    }

    pub fn get_stats(&self) -> PipelineStats {
        let duration = self.start_time.elapsed().as_secs();

        PipelineStats {
            files_processed: self.files_processed.load(Ordering::SeqCst),
            files_failed: self.files_failed.load(Ordering::SeqCst),
            documents_created: self.documents_created.load(Ordering::SeqCst),
            total_bytes_processed: self.bytes_processed.load(Ordering::SeqCst),
            duration_secs: duration,
        }
    }

    fn update_detail_bar(&self) {
        let documents = self.documents_created.load(Ordering::SeqCst);
        let failed = self.files_failed.load(Ordering::SeqCst);

        let message = format!("Documents: {} | Failed: {}", documents, failed);

        self.detail_bar.set_message(message);
    }
}

impl Drop for ProgressTracker {
    fn drop(&mut self) {
        self.finish();
    }
}

fn create_progress_bar(multi_progress: &MultiProgress, total: u64, colored: bool) -> ProgressBar {
    let bar = multi_progress.add(ProgressBar::new(total));
    if colored {
        bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
                )
                .expect("Failed to create progress bar template")
                .progress_chars("█▓▒░"),
        );
    } else {
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner} [{elapsed_precise}] [{bar:40}] {pos}/{len} ({eta}) {msg}")
                .expect("Failed to create progress bar template")
                .progress_chars("=>-"),
        );
    }
    bar
}

fn create_detail_bar(multi_progress: &MultiProgress) -> ProgressBar {
    let bar = multi_progress.add(ProgressBar::new(0));
    let style = ProgressStyle::default_bar()
        .template("{msg}")
        .expect("Failed to create detail bar template");
    bar.set_style(style);
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stats_calculations() {
        let mut stats = PipelineStats::new();
        stats.files_processed = 100;
        stats.files_failed = 10;
        stats.duration_secs = 10;
        stats.total_bytes_processed = 1000;

        assert_eq!(stats.files_per_second(), 10.0);
        assert_eq!(stats.bytes_per_second(), 100.0);
        assert!((stats.success_rate() - 90.909).abs() < 0.01);
    }

    #[test]
    fn test_pipeline_stats_zero_duration() {
        let stats = PipelineStats::new();
        assert_eq!(stats.files_per_second(), 0.0);
        assert_eq!(stats.bytes_per_second(), 0.0);
    }

    #[test]
    fn test_progress_tracker_increment() {
        let tracker = ProgressTracker::new(100);

        tracker.inc_files_processed();
        tracker.add_bytes_processed(1024);

        let stats = tracker.get_stats();
        assert_eq!(stats.files_processed, 1);
        assert_eq!(stats.total_bytes_processed, 1024);
    }

    #[test]
    fn test_progress_tracker_failures() {
        let tracker = ProgressTracker::new(100);

        tracker.inc_files_failed();
        tracker.inc_files_failed();

        let stats = tracker.get_stats();
        assert_eq!(stats.files_failed, 2);
    }
}
