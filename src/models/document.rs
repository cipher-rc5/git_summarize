// file: src/models/document.rs
// description: core document/chunk model with validation and serialization
// reference: internal data structures

use crate::parser::Chunk;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single retrievable unit stored in the vector index. One source file is
/// split into many `Document` rows (one per heading-aware chunk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Stable, unique id per chunk: hash(repository_url + relative_path + chunk_index).
    pub id: String,
    pub file_path: String,
    pub relative_path: String,
    /// The chunk's raw markdown content.
    pub content: String,
    /// Hash of `content`, used for change detection.
    pub content_hash: String,
    /// Ordinal of this chunk within its source file.
    pub chunk_index: u32,
    /// Heading breadcrumb, e.g. "Installation > From source". Empty for preamble.
    pub heading_path: String,
    pub file_size: u64,
    pub last_modified: u64,
    pub parsed_at: u64,
    pub normalized: bool,
    pub repository_url: String,
}

impl Document {
    /// Build a chunk-level document from a parsed [`Chunk`].
    pub fn from_chunk(
        file_path: &str,
        relative_path: &str,
        chunk: &Chunk,
        last_modified: u64,
        repository_url: &str,
        normalized: bool,
    ) -> Self {
        let id = Self::chunk_id(repository_url, relative_path, chunk.index);
        let content_hash = Self::compute_hash(&chunk.content);
        let file_size = chunk.content.len() as u64;

        Self {
            id,
            file_path: file_path.to_string(),
            relative_path: relative_path.to_string(),
            content: chunk.content.clone(),
            content_hash,
            chunk_index: chunk.index as u32,
            heading_path: chunk.heading_path.join(" > "),
            file_size,
            last_modified,
            parsed_at: now_secs(),
            normalized,
            repository_url: repository_url.to_string(),
        }
    }

    pub fn chunk_id(repository_url: &str, relative_path: &str, chunk_index: usize) -> String {
        let key = format!("{repository_url}\u{0}{relative_path}\u{0}{chunk_index}");
        Self::compute_hash(&key)
    }

    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Chunk;

    fn sample_chunk() -> Chunk {
        Chunk {
            index: 2,
            heading_path: vec!["Guide".to_string(), "Setup".to_string()],
            content: "# Setup\n\nInstall the thing.".to_string(),
        }
    }

    #[test]
    fn test_chunk_document_creation() {
        let doc = Document::from_chunk(
            "/path/to/file.md",
            "file.md",
            &sample_chunk(),
            1234567890,
            "https://github.com/example/repo",
            true,
        );

        assert_eq!(doc.file_path, "/path/to/file.md");
        assert_eq!(doc.chunk_index, 2);
        assert_eq!(doc.heading_path, "Guide > Setup");
        assert!(!doc.content_hash.is_empty());
        assert!(doc.normalized);
    }

    #[test]
    fn test_chunk_id_is_unique_per_index() {
        let a = Document::chunk_id("repo", "a.md", 0);
        let b = Document::chunk_id("repo", "a.md", 1);
        let c = Document::chunk_id("repo", "b.md", 0);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_hash_consistency() {
        let id1 = Document::chunk_id("repo", "a.md", 0);
        let id2 = Document::chunk_id("repo", "a.md", 0);
        assert_eq!(id1, id2);
    }
}
