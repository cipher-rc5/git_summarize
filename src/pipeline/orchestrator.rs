// file: src/pipeline/orchestrator.rs
// description: coordinates repository scanning, processing, and database insertion
// reference: orchestrates asynchronous ingestion workflow

use crate::config::Config;
use crate::database::{BatchInserter, ClickHouseClient, SchemaManager};
use crate::error::{PipelineError, Result};
use crate::pipeline::processor::{FileProcessor, ProcessingResult};
use crate::pipeline::progress::{PipelineStats, ProgressTracker};
use crate::repository::{FileScanner, RepositorySync, ScannedFile};
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

pub struct PipelineOrchestrator {
    config: Config,
    db_client: ClickHouseClient,
    processor: Arc<FileProcessor>,
    max_concurrent_tasks: usize,
}

impl PipelineOrchestrator {
    pub fn new(config: Config) -> Result<Self> {
        let db_client = ClickHouseClient::new(config.database.clone())?;
        let processor = Arc::new(FileProcessor::new(config.clone()));
        let max_concurrent_tasks = config.pipeline.parallel_workers.max(1);

        Ok(Self {
            config,
            db_client,
            processor,
            max_concurrent_tasks,
        })
    }

    pub async fn run(&self, force_reprocess: bool) -> Result<PipelineStats> {
        info!("Starting Lazarus ingestion pipeline");

        if self.config.repository.sync_on_start {
            info!("Syncing repository...");
            let repository_config = self.config.repository.clone();
            tokio::task::spawn_blocking(move || {
                let sync = RepositorySync::new(repository_config);
                sync.sync()
            })
            .await
            .map_err(|e| {
                PipelineError::RepositorySync(format!("Repository sync task failed: {}", e))
            })??;
            info!("Repository sync complete");
        } else {
            info!("Repository sync disabled, using local files only");
        }

        let effective_force = force_reprocess || self.config.pipeline.force_reprocess;

        info!("Scanning for files...");
        let files = self.scan_files(effective_force).await?;
        info!("Found {} files to process", files.len());

        if files.is_empty() {
            warn!("No files found to process");
            return Ok(PipelineStats::new());
        }

        let progress = Arc::new(ProgressTracker::new(files.len()));

        info!(
            "Processing files with {} concurrent tasks...",
            self.max_concurrent_tasks
        );
        let results = self.process_files(files, progress.clone()).await?;

        info!("Inserting data into database...");
        self.insert_results(results, progress.clone()).await?;

        let stats = progress.get_stats();
        progress.finish();

        self.log_final_stats(&stats);

        Ok(stats)
    }

    async fn scan_files(&self, force_reprocess: bool) -> Result<Vec<ScannedFile>> {
        let repo_path = self.config.repository.local_path.clone();
        let pipeline_config = self.config.pipeline.clone();

        let files = tokio::task::spawn_blocking(move || {
            let scanner = FileScanner::new(pipeline_config);
            scanner.scan_directory(&repo_path)
        })
        .await
        .map_err(|e| PipelineError::Validation(format!("File scanning task failed: {}", e)))??;

        if force_reprocess {
            return Ok(files);
        }

        let existing_hashes = self.db_client.get_document_hashes().await?;
        let scanner = FileScanner::new(self.config.pipeline.clone());
        Ok(scanner.filter_by_hash(files, &existing_hashes))
    }

    async fn process_files(
        &self,
        files: Vec<ScannedFile>,
        progress: Arc<ProgressTracker>,
    ) -> Result<Vec<ProcessingResult>> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_tasks));
        let processor = self.processor.clone();

        let tasks = files.into_iter().map(|file| {
            let semaphore = semaphore.clone();
            let processor = processor.clone();
            let progress = progress.clone();

            async move {
                let permit = semaphore.acquire_owned().await.ok()?;

                let file_size = file.size;
                let file_path = file.relative_path.clone();
                let processed = tokio::task::spawn_blocking({
                    let processor = processor.clone();
                    let file_clone = file.clone();
                    move || processor.process(&file_clone)
                })
                .await;

                drop(permit);

                match processed {
                    Ok(Ok(result)) => {
                        progress.inc_files_processed();
                        progress.add_bytes_processed(file_size);
                        Some(result)
                    }
                    Ok(Err(e)) => {
                        progress.inc_files_failed();
                        warn!("Failed to process file {}: {}", file_path, e);
                        None
                    }
                    Err(e) => {
                        progress.inc_files_failed();
                        error!("Processing task panicked: {}", e);
                        None
                    }
                }
            }
        });

        let results: Vec<ProcessingResult> = stream::iter(tasks)
            .buffer_unordered(self.max_concurrent_tasks)
            .filter_map(|result| async move { result })
            .collect()
            .await;

        Ok(results)
    }

    async fn insert_results(
        &self,
        results: Vec<ProcessingResult>,
        progress: Arc<ProgressTracker>,
    ) -> Result<()> {
        if results.is_empty() {
            warn!("No results to insert");
            return Ok(());
        }

        let inserter = BatchInserter::new(&self.db_client);

        for result in results {
            progress.set_message(format!("Inserting {}", result.document.relative_path));

            let stats = inserter
                .insert_complete_batch(
                    result.document,
                    result.incidents,
                    result.crypto_addresses,
                    result.iocs,
                )
                .await?;

            for _ in 0..stats.documents_inserted {
                progress.add_document();
            }
            progress.add_crypto_addresses(stats.addresses_inserted);
            progress.add_incidents(stats.incidents_inserted);
            progress.add_iocs(stats.iocs_inserted);
        }

        progress.set_message("Insertion complete".to_string());
        Ok(())
    }

    fn log_final_stats(&self, stats: &PipelineStats) {
        info!("=== Pipeline Execution Summary ===");
        info!("Duration: {} seconds", stats.duration_secs);
        info!("Files processed: {}", stats.files_processed);
        info!("Files failed: {}", stats.files_failed);
        info!("Success rate: {:.2}%", stats.success_rate());
        info!("Documents created: {}", stats.documents_created);
        info!(
            "Crypto addresses extracted: {}",
            stats.crypto_addresses_extracted
        );
        info!("Incidents extracted: {}", stats.incidents_extracted);
        info!("IOCs extracted: {}", stats.iocs_extracted);
        info!("Total entities: {}", stats.total_entities_extracted());
        info!(
            "Processing speed: {:.2} files/sec",
            stats.files_per_second()
        );
        info!(
            "Throughput: {:.2} MB/sec",
            stats.bytes_per_second() / 1_048_576.0
        );
        info!("=================================");
    }

    pub async fn verify_schema(&self) -> Result<()> {
        info!("Verifying database schema...");
        let manager = SchemaManager::new(&self.db_client);
        if manager.verify_schema().await? {
            info!("Database schema verified successfully");
        } else {
            warn!("Database schema verification failed; missing tables detected");
        }
        Ok(())
    }

    pub async fn get_stats(&self) -> Result<PipelineStats> {
        info!("Fetching database statistics...");

        let documents_count = self.count_rows("documents").await?;
        let crypto_count = self.count_rows("crypto_addresses").await?;
        let incidents_count = self.count_rows("incidents").await?;
        let iocs_count = self.count_rows("iocs").await?;

        let mut stats = PipelineStats::new();
        stats.documents_created = documents_count as usize;
        stats.crypto_addresses_extracted = crypto_count as usize;
        stats.incidents_extracted = incidents_count as usize;
        stats.iocs_extracted = iocs_count as usize;

        info!("Database contains:");
        info!("  Documents: {}", documents_count);
        info!("  Crypto addresses: {}", crypto_count);
        info!("  Incidents: {}", incidents_count);
        info!("  IOCs: {}", iocs_count);

        Ok(stats)
    }

    async fn count_rows(&self, table: &str) -> Result<u64> {
        if !self.db_client.table_exists(table).await? {
            return Ok(0);
        }

        let query = format!("SELECT count() FROM {}", table);
        self.db_client
            .get_client()
            .query(&query)
            .fetch_one::<u64>()
            .await
            .map_err(PipelineError::Database)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, ExtractionConfig, PipelineConfig, RepositoryConfig};
    use std::path::PathBuf;

    fn base_config(sync_on_start: bool) -> Config {
        Config {
            repository: RepositoryConfig {
                source_url: "https://example.com/repo.git".to_string(),
                local_path: PathBuf::from("/tmp/test"),
                branch: "main".to_string(),
                sync_on_start,
            },
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 8123,
                database: "test".to_string(),
                username: None,
                password: None,
                batch_size: 1000,
            },
            pipeline: PipelineConfig {
                parallel_workers: 2,
                skip_patterns: vec![],
                force_reprocess: false,
                max_file_size_mb: 10,
            },
            extraction: ExtractionConfig {
                extract_crypto_addresses: true,
                extract_incidents: true,
                extract_iocs: true,
                normalize_markdown: false,
            },
        }
    }

    #[test]
    fn test_orchestrator_creation() {
        let config = base_config(false);
        let orchestrator = PipelineOrchestrator::new(config).unwrap();
        assert_eq!(orchestrator.max_concurrent_tasks, 2);
    }

    #[test]
    fn test_orchestrator_with_sync_disabled() {
        let config = base_config(false);
        let orchestrator = PipelineOrchestrator::new(config).unwrap();
        assert!(!orchestrator.config.repository.sync_on_start);
    }

    #[test]
    fn test_orchestrator_with_sync_enabled() {
        let config = base_config(true);
        let orchestrator = PipelineOrchestrator::new(config).unwrap();
        assert!(orchestrator.config.repository.sync_on_start);
    }
}
