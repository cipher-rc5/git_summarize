// file: src/models/search_result.rs
// description: Search result model with similarity scores
// reference: Used for vector similarity search results

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Document ID (content hash)
    pub id: String,

    /// File path in the repository
    pub file_path: String,

    /// Relative path from repository root
    pub relative_path: String,

    /// Document content
    pub content: String,

    /// Repository URL
    pub repository_url: String,

    /// Similarity score (higher is more similar, typically 0.0-1.0)
    pub score: f32,

    /// Optional: Distance metric (lower is more similar)
    pub distance: Option<f32>,

    /// File size in bytes
    pub file_size: u64,

    /// Last modified timestamp
    pub last_modified: u64,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(
        id: String,
        file_path: String,
        relative_path: String,
        content: String,
        repository_url: String,
        score: f32,
        distance: Option<f32>,
        file_size: u64,
        last_modified: u64,
    ) -> Self {
        Self {
            id,
            file_path,
            relative_path,
            content,
            repository_url,
            score,
            distance,
            file_size,
            last_modified,
        }
    }

    /// Format as a summary string for display
    pub fn format_summary(&self, max_content_len: usize) -> String {
        let content_preview = if self.content.len() > max_content_len {
            format!("{}...", &self.content[..max_content_len])
        } else {
            self.content.clone()
        };

        format!(
            "Score: {:.4} | {} ({})\n{}\n",
            self.score,
            self.relative_path,
            self.repository_url,
            content_preview
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult::new(
            "abc123".to_string(),
            "/path/to/file.md".to_string(),
            "file.md".to_string(),
            "Test content".to_string(),
            "https://github.com/example/repo".to_string(),
            0.95,
            Some(0.05),
            100,
            1234567890,
        );

        assert_eq!(result.score, 0.95);
        assert_eq!(result.distance, Some(0.05));
        assert_eq!(result.relative_path, "file.md");
    }

    #[test]
    fn test_format_summary() {
        let result = SearchResult::new(
            "abc123".to_string(),
            "/path/to/file.md".to_string(),
            "docs/readme.md".to_string(),
            "This is a very long content that will be truncated".to_string(),
            "https://github.com/example/repo".to_string(),
            0.87,
            None,
            100,
            1234567890,
        );

        let summary = result.format_summary(20);
        assert!(summary.contains("0.8700"));
        assert!(summary.contains("docs/readme.md"));
        assert!(summary.contains("..."));
    }
}
