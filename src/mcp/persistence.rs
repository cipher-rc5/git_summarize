// file: src/mcp/persistence.rs
// description: Persistent storage for MCP repository metadata
// reference: Production-grade metadata persistence

use crate::error::{PipelineError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Persistent repository metadata (moved from in-memory only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadata {
    pub url: String,
    pub branch: String,
    pub commit_hash: String,
    pub local_path: PathBuf,
    pub subdirectories: Option<Vec<String>>,
    pub file_count: usize,
    pub ingested_at: u64,
}

pub struct MetadataStore {
    storage_path: PathBuf,
    cache: HashMap<String, RepositoryMetadata>,
}

impl MetadataStore {
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        // Ensure storage directory exists
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                PipelineError::Config(format!("Failed to create metadata directory: {}", e))
            })?;
        }

        let mut store = Self {
            storage_path,
            cache: HashMap::new(),
        };

        // Load existing metadata
        store.load().await?;

        Ok(store)
    }

    pub async fn load(&mut self) -> Result<()> {
        if !self.storage_path.exists() {
            debug!("No existing metadata file found at {:?}", self.storage_path);
            return Ok(());
        }

        let contents = fs::read_to_string(&self.storage_path)
            .await
            .map_err(|e| {
                PipelineError::Config(format!("Failed to read metadata file: {}", e))
            })?;

        self.cache = serde_json::from_str(&contents).map_err(|e| {
            warn!("Failed to parse metadata file, starting fresh: {}", e);
            PipelineError::Config(format!("Failed to parse metadata: {}", e))
        })?;

        info!("Loaded {} repository metadata entries", self.cache.len());
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        let contents = serde_json::to_string_pretty(&self.cache).map_err(|e| {
            PipelineError::Config(format!("Failed to serialize metadata: {}", e))
        })?;

        fs::write(&self.storage_path, contents)
            .await
            .map_err(|e| {
                PipelineError::Config(format!("Failed to write metadata file: {}", e))
            })?;

        debug!("Saved {} repository metadata entries", self.cache.len());
        Ok(())
    }

    pub fn insert(&mut self, key: String, metadata: RepositoryMetadata) {
        self.cache.insert(key, metadata);
    }

    pub fn remove(&mut self, key: &str) -> Option<RepositoryMetadata> {
        self.cache.remove(key)
    }

    pub fn get(&self, key: &str) -> Option<&RepositoryMetadata> {
        self.cache.get(key)
    }

    pub fn list(&self) -> &HashMap<String, RepositoryMetadata> {
        &self.cache
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_metadata_store_persistence() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("metadata.json");

        // Create and populate store
        {
            let mut store = MetadataStore::new(store_path.clone()).await.unwrap();
            let metadata = RepositoryMetadata {
                url: "https://github.com/test/repo".to_string(),
                branch: "main".to_string(),
                commit_hash: "abc123".to_string(),
                local_path: PathBuf::from("/tmp/repo"),
                subdirectories: None,
                file_count: 10,
                ingested_at: 1234567890,
            };
            store.insert("repo".to_string(), metadata);
            store.save().await.unwrap();
        }

        // Load and verify
        {
            let store = MetadataStore::new(store_path).await.unwrap();
            assert_eq!(store.len(), 1);
            let meta = store.get("repo").unwrap();
            assert_eq!(meta.url, "https://github.com/test/repo");
            assert_eq!(meta.file_count, 10);
        }
    }
}
