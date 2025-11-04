// file: src/pipeline/processor.rs
// description: converts scanned markdown files into structured ingestion artifacts
// reference: parses markdown, runs extractors, and prepares database models

use crate::config::Config;
use crate::error::{PipelineError, Result};
use crate::extractor::{CryptoExtractor, IncidentExtractor, IocExtractor};
use crate::models::{CryptoAddress, Document, Incident, Ioc};
use crate::parser::{MarkdownNormalizer, MarkdownParser};
use crate::repository::ScannedFile;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

pub struct ProcessingResult {
    pub document: Document,
    pub crypto_addresses: Vec<CryptoAddress>,
    pub incidents: Vec<Incident>,
    pub iocs: Vec<Ioc>,
}

pub struct FileProcessor {
    config: Config,
    parser: MarkdownParser,
    normalizer: Option<MarkdownNormalizer>,
}

impl FileProcessor {
    pub fn new(config: Config) -> Self {
        let normalizer = if config.extraction.normalize_markdown {
            Some(MarkdownNormalizer::new())
        } else {
            None
        };

        Self {
            config,
            parser: MarkdownParser::new(),
            normalizer,
        }
    }

    pub fn process(&self, scanned_file: &ScannedFile) -> Result<ProcessingResult> {
        info!("Processing file: {}", scanned_file.relative_path);

        let content = self.read_file_content(&scanned_file.path)?;
        let max_bytes = (self.config.pipeline.max_file_size_mb as u64) * 1_048_576;
        if max_bytes > 0 && content.len() as u64 > max_bytes {
            warn!(
                "File too large ({} bytes), skipping: {}",
                content.len(),
                scanned_file.relative_path
            );
            return Err(PipelineError::Validation(format!(
                "File too large: {}",
                scanned_file.relative_path
            )));
        }

        let normalized_content = if let Some(ref normalizer) = self.normalizer {
            normalizer.normalize(&content)?
        } else {
            content.clone()
        };

        let parsed = self.parser.parse(&normalized_content)?;

        let mut document = Document::new(
            scanned_file.path.display().to_string(),
            scanned_file.relative_path.clone(),
            normalized_content.clone(),
            scanned_file.modified,
        );

        if self.normalizer.is_some() {
            document.mark_normalized();
        }

        let file_path_str = scanned_file.path.display().to_string();

        let crypto_addresses = if self.config.extraction.extract_crypto_addresses {
            self.extract_crypto_addresses(&parsed.plain_text, &file_path_str)
        } else {
            Vec::new()
        };

        let incidents = if self.config.extraction.extract_incidents {
            self.extract_incidents(&content, &file_path_str)
        } else {
            Vec::new()
        };

        let iocs = if self.config.extraction.extract_iocs {
            self.extract_iocs(&parsed.plain_text)
        } else {
            Vec::new()
        };

        debug!(
            "Extracted {} crypto addresses, {} incidents, {} IOCs from {}",
            crypto_addresses.len(),
            incidents.len(),
            iocs.len(),
            scanned_file.relative_path
        );

        Ok(ProcessingResult {
            document,
            crypto_addresses,
            incidents,
            iocs,
        })
    }

    fn read_file_content(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|source| PipelineError::FileOperation {
            path: path.to_path_buf(),
            source,
        })
    }

    fn extract_crypto_addresses(&self, text: &str, file_path: &str) -> Vec<CryptoAddress> {
        let mut extractor = CryptoExtractor::new();
        extractor.extract_from_text(text, file_path, "")
    }

    fn extract_incidents(&self, content: &str, file_path: &str) -> Vec<Incident> {
        let extractor = IncidentExtractor::new();
        extractor.extract_from_markdown(content, file_path)
    }

    fn extract_iocs(&self, text: &str) -> Vec<Ioc> {
        let mut extractor = IocExtractor::new();
        extractor.extract_from_text(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, ExtractionConfig, PipelineConfig, RepositoryConfig};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            repository: RepositoryConfig {
                source_url: "https://example.com/repo.git".to_string(),
                local_path: PathBuf::from("/tmp/test"),
                branch: "main".to_string(),
                sync_on_start: false,
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

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_process_simple_markdown() {
        let config = create_test_config();
        let processor = FileProcessor::new(config);
        let temp_dir = TempDir::new().unwrap();

        let content = r#"---
title: Test Document
---

# Test Document

This is a test with a Bitcoin address: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa

And an IP address: 192.168.1.1
"#;

        let file_path = create_test_file(&temp_dir, "test.md", content);
        let scanned_file = ScannedFile {
            path: file_path.clone(),
            relative_path: "test.md".to_string(),
            size: content.len() as u64,
            modified: 0,
        };

        let result = processor.process(&scanned_file).unwrap();

        assert_eq!(result.document.file_path, file_path.display().to_string());
        assert_eq!(result.document.relative_path, "test.md");
        assert!(!result.crypto_addresses.is_empty());
        assert!(!result.iocs.is_empty());
    }

    #[test]
    fn test_process_file_too_large() {
        let mut config = create_test_config();
        config.pipeline.max_file_size_mb = 1;
        let processor = FileProcessor::new(config);
        let temp_dir = TempDir::new().unwrap();

        let content = "a".repeat(2 * 1_048_576);
        let file_path = create_test_file(&temp_dir, "large.md", &content);
        let scanned_file = ScannedFile {
            path: file_path,
            relative_path: "large.md".to_string(),
            size: content.len() as u64,
            modified: 0,
        };

        let result = processor.process(&scanned_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_with_normalization() {
        let mut config = create_test_config();
        config.extraction.normalize_markdown = true;
        let processor = FileProcessor::new(config);
        let temp_dir = TempDir::new().unwrap();

        let content = r#"# Heading

Some content with **bold** and *italic* text.
"#;

        let file_path = create_test_file(&temp_dir, "test.md", content);
        let scanned_file = ScannedFile {
            path: file_path,
            relative_path: "test.md".to_string(),
            size: content.len() as u64,
            modified: 0,
        };

        let result = processor.process(&scanned_file).unwrap();
        assert!(result.document.normalized);
        assert!(!result.document.content.is_empty());
    }
}
