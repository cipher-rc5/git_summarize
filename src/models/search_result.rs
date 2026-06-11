// file: src/models/search_result.rs
// description: Search result model with similarity scores
// reference: Used for vector similarity search results

use serde::{Deserialize, Serialize};

/// Truncate to at most `max` chars (not bytes), appending an ellipsis if cut.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}...")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Document ID (content hash)
    pub id: String,

    /// File path in the repository
    pub file_path: String,

    /// Relative path from repository root
    pub relative_path: String,

    /// Heading breadcrumb of the matched chunk, e.g. "Setup > From source".
    pub heading_path: String,

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

/// Groups disk path information to keep constructor arguments manageable.
#[derive(Debug, Clone)]
pub struct SearchResultPaths {
    pub file_path: String,
    pub relative_path: String,
    pub heading_path: String,
}

/// Groups scoring metadata (score and optional distance).
#[derive(Debug, Clone)]
pub struct SearchResultScoring {
    pub score: f32,
    pub distance: Option<f32>,
}

/// Groups file metadata such as size and timestamps.
#[derive(Debug, Clone)]
pub struct SearchResultFileMetadata {
    pub file_size: u64,
    pub last_modified: u64,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(
        id: String,
        paths: SearchResultPaths,
        content: String,
        repository_url: String,
        scoring: SearchResultScoring,
        metadata: SearchResultFileMetadata,
    ) -> Self {
        Self {
            id,
            file_path: paths.file_path,
            relative_path: paths.relative_path,
            heading_path: paths.heading_path,
            content,
            repository_url,
            score: scoring.score,
            distance: scoring.distance,
            file_size: metadata.file_size,
            last_modified: metadata.last_modified,
        }
    }

    /// A human-readable location: "relative/path.md # Heading > Subheading".
    pub fn location(&self) -> String {
        if self.heading_path.is_empty() {
            self.relative_path.clone()
        } else {
            format!("{} # {}", self.relative_path, self.heading_path)
        }
    }

    /// Format as a summary string for display
    pub fn format_summary(&self, max_content_len: usize) -> String {
        let content_preview = truncate_chars(&self.content, max_content_len);

        format!(
            "Score: {:.4} | {} ({})\n{}\n",
            self.score,
            self.location(),
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
            SearchResultPaths {
                file_path: "/path/to/file.md".to_string(),
                relative_path: "file.md".to_string(),
                heading_path: String::new(),
            },
            "Test content".to_string(),
            "https://github.com/example/repo".to_string(),
            SearchResultScoring {
                score: 0.95,
                distance: Some(0.05),
            },
            SearchResultFileMetadata {
                file_size: 100,
                last_modified: 1_234_567_890,
            },
        );

        assert_eq!(result.score, 0.95);
        assert_eq!(result.distance, Some(0.05));
        assert_eq!(result.relative_path, "file.md");
    }

    #[test]
    fn test_format_summary() {
        let result = SearchResult::new(
            "abc123".to_string(),
            SearchResultPaths {
                file_path: "/path/to/file.md".to_string(),
                relative_path: "docs/readme.md".to_string(),
                heading_path: String::new(),
            },
            "This is a very long content that will be truncated".to_string(),
            "https://github.com/example/repo".to_string(),
            SearchResultScoring {
                score: 0.87,
                distance: None,
            },
            SearchResultFileMetadata {
                file_size: 100,
                last_modified: 1_234_567_890,
            },
        );

        let summary = result.format_summary(20);
        assert!(summary.contains("0.8700"));
        assert!(summary.contains("docs/readme.md"));
        assert!(summary.contains("..."));
    }
}
