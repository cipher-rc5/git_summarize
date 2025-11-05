// file: src/pipeline/progress.rs
// description: Progress tracking and statistics reporting for pipeline execution
// reference: Uses indicatif for progress bars and tracks processing metrics

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
    pub crypto_addresses_extracted: usize,
    pub incidents_extracted: usize,
    pub iocs_extracted: usize,
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

    pub fn total_entities_extracted(&self) -> usize {
        self.crypto_addresses_extracted + self.incidents_extracted + self.iocs_extracted
    }
}

pub struct ProgressTracker {
    main_bar: ProgressBar,
    detail_bar: ProgressBar,
    files_processed: Arc<AtomicUsize>,
    files_failed: Arc<AtomicUsize>,
    documents_created: Arc<AtomicUsize>,
    crypto_addresses: Arc<AtomicUsize>,
    incidents: Arc<AtomicUsize>,
    iocs: Arc<AtomicUsize>,
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
        let detail_bar = create_detail_bar(&multi_progress, colored);

        Self {
            main_bar,
            detail_bar,
            files_processed: Arc::new(AtomicUsize::new(0)),
            files_failed: Arc::new(AtomicUsize::new(0)),
            documents_created: Arc::new(AtomicUsize::new(0)),
            crypto_addresses: Arc::new(AtomicUsize::new(0)),
            incidents: Arc::new(AtomicUsize::new(0)),
            iocs: Arc::new(AtomicUsize::new(0)),
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

    pub fn add_crypto_addresses(&self, count: usize) {
        self.crypto_addresses.fetch_add(count, Ordering::SeqCst);
        self.update_detail_bar();
    }

    pub fn add_incidents(&self, count: usize) {
        self.incidents.fetch_add(count, Ordering::SeqCst);
        self.update_detail_bar();
    }

    pub fn add_iocs(&self, count: usize) {
        self.iocs.fetch_add(count, Ordering::SeqCst);
        self.update_detail_bar();
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
            crypto_addresses_extracted: self.crypto_addresses.load(Ordering::SeqCst),
            incidents_extracted: self.incidents.load(Ordering::SeqCst),
            iocs_extracted: self.iocs.load(Ordering::SeqCst),
            total_bytes_processed: self.bytes_processed.load(Ordering::SeqCst),
            duration_secs: duration,
        }
    }

    fn update_detail_bar(&self) {
        let crypto = self.crypto_addresses.load(Ordering::SeqCst);
        let incidents = self.incidents.load(Ordering::SeqCst);
        let iocs = self.iocs.load(Ordering::SeqCst);
        let failed = self.files_failed.load(Ordering::SeqCst);

        let message = format!(
            "Extracted: {} crypto, {} incidents, {} IOCs | Failed: {}",
            crypto, incidents, iocs, failed
        );

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

fn create_detail_bar(multi_progress: &MultiProgress, _colored: bool) -> ProgressBar {
    let bar = multi_progress.add(ProgressBar::new(0));
    let style = ProgressStyle::default_bar()
        .template("{msg}")
        .expect("Failed to create detail bar template");
    bar.set_style(style);
    bar
}

#[allow(dead_code)]
pub fn log_phase(phase: &str, colored: bool) {
    if colored {
        println!("\n{} {}\n", "▶".cyan().bold(), phase.bright_white().bold());
    } else {
        println!("\n> {}\n", phase);
    }
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
    fn test_pipeline_stats_total_entities() {
        let mut stats = PipelineStats::new();
        stats.crypto_addresses_extracted = 10;
        stats.incidents_extracted = 20;
        stats.iocs_extracted = 30;

        assert_eq!(stats.total_entities_extracted(), 60);
    }

    #[test]
    fn test_progress_tracker_increment() {
        let tracker = ProgressTracker::new(100);

        tracker.inc_files_processed();
        tracker.add_crypto_addresses(5);
        tracker.add_incidents(3);
        tracker.add_iocs(10);
        tracker.add_bytes_processed(1024);

        let stats = tracker.get_stats();
        assert_eq!(stats.files_processed, 1);
        assert_eq!(stats.crypto_addresses_extracted, 5);
        assert_eq!(stats.incidents_extracted, 3);
        assert_eq!(stats.iocs_extracted, 10);
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
