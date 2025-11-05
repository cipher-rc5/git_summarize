// file: src/repository/scanner.rs
// description: Directory walking and file discovery with filtering
// reference: https://docs.rs/walkdir

use crate::config::PipelineConfig;
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use walkdir::WalkDir;

pub struct FileScanner {
    config: PipelineConfig,
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub size: u64,
    pub modified: u64,
}

impl FileScanner {
    pub fn new(config: PipelineConfig) -> Self {
        Self { config }
    }

    pub fn scan_directory(&self, root: &Path) -> Result<Vec<ScannedFile>> {
        info!("Scanning directory: {}", root.display());
        let mut files = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            if self.should_skip(path) {
                debug!("Skipping file: {}", path.display());
                continue;
            }

            if let Some(extension) = path.extension()
                && extension == "md"
                && let Ok(metadata) = entry.metadata()
            {
                let size = metadata.len();
                let max_size = (self.config.max_file_size_mb * 1024 * 1024) as u64;

                if size > max_size {
                    debug!(
                        "Skipping large file ({} MB): {}",
                        size / 1024 / 1024,
                        path.display()
                    );
                    continue;
                }

                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let relative_path = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                files.push(ScannedFile {
                    path: path.to_path_buf(),
                    relative_path,
                    size,
                    modified,
                });
            }
        }

        info!("Found {} markdown files", files.len());
        Ok(files)
    }

    fn should_skip(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.config.skip_patterns {
            if pattern.contains('*') {
                let pattern_without_star = pattern.replace("*.", ".");
                if path_str.ends_with(&pattern_without_star) {
                    return true;
                }
            } else if path_str.contains(pattern) {
                return true;
            }
        }

        false
    }

    pub fn filter_by_hash(
        &self,
        files: Vec<ScannedFile>,
        existing_hashes: &[String],
    ) -> Vec<ScannedFile> {
        if self.config.force_reprocess {
            return files;
        }

        files
            .into_iter()
            .filter(|file| self.should_include_by_hash(file, existing_hashes))
            .collect()
    }

    fn should_include_by_hash(&self, file: &ScannedFile, existing_hashes: &[String]) -> bool {
        match Self::compute_file_hash(&file.path) {
            Ok(hash) => {
                let seen = existing_hashes.iter().any(|existing| existing == &hash);
                if seen {
                    debug!(
                        "Skipping already ingested file: {} (hash {})",
                        file.path.display(),
                        hash
                    );
                }
                !seen
            }
            Err(err) => {
                debug!(
                    "Failed to hash file {}; including for processing. Error: {}",
                    file.path.display(),
                    err
                );
                true
            }
        }
    }

    fn compute_file_hash(path: &Path) -> std::io::Result<String> {
        let content = fs::read_to_string(path)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_scan_directory() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.md");
        fs::write(&test_file, "# Test").unwrap();

        let config = PipelineConfig {
            parallel_workers: 1,
            skip_patterns: vec![],
            force_reprocess: false,
            max_file_size_mb: 10,
        };

        let scanner = FileScanner::new(config);
        let files = scanner.scan_directory(temp.path()).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "test.md");
    }

    #[test]
    fn test_skip_patterns() {
        let config = PipelineConfig {
            parallel_workers: 1,
            skip_patterns: vec!["*.zip".to_string(), ".git/*".to_string()],
            force_reprocess: false,
            max_file_size_mb: 10,
        };

        let scanner = FileScanner::new(config);

        assert!(scanner.should_skip(Path::new("test.zip")));
        assert!(scanner.should_skip(Path::new(".git/config")));
        assert!(!scanner.should_skip(Path::new("test.md")));
    }
}
