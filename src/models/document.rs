// file: src/models/document.rs
// description: core document model with validation and serialization
// reference: internal data structures

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct Document {
    pub file_path: String,
    pub relative_path: String,
    pub content: String,
    pub content_hash: String,
    pub file_size: u64,
    pub last_modified: u64,
    pub parsed_at: u64,
    pub normalized: bool,
}

impl Document {
    pub fn new(
        file_path: String,
        relative_path: String,
        content: String,
        last_modified: u64,
    ) -> Self {
        let content_hash = Self::compute_hash(&content);
        let file_size = content.len() as u64;
        let parsed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            file_path,
            relative_path,
            content,
            content_hash,
            file_size,
            last_modified,
            parsed_at,
            normalized: false,
        }
    }

    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn mark_normalized(&mut self) {
        self.normalized = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let doc = Document::new(
            "/path/to/file.md".to_string(),
            "file.md".to_string(),
            "# Test Content".to_string(),
            1234567890,
        );

        assert_eq!(doc.file_path, "/path/to/file.md");
        assert!(!doc.content_hash.is_empty());
        assert_eq!(doc.file_size, 15);
        assert!(!doc.normalized);
    }

    #[test]
    fn test_hash_consistency() {
        let content = "Test content";
        let hash1 = Document::compute_hash(content);
        let hash2 = Document::compute_hash(content);
        assert_eq!(hash1, hash2);
    }
}
