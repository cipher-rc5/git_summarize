// file: src/utils/validation.rs
// description: data validation utilities and helpers
// reference: input validation patterns

use crate::error::{PipelineError, Result};
use std::fs;
use std::path::Path;

pub struct Validator;

impl Validator {
    pub fn validate_file_path(path: &Path) -> Result<()> {
        let canonical = fs::canonicalize(path).map_err(|e| {
            PipelineError::Validation(format!(
                "Cannot canonicalize path {}: {}",
                path.display(),
                e
            ))
        })?;

        if !canonical.is_absolute() {
            return Err(PipelineError::Validation(format!(
                "Path must be absolute: {}",
                canonical.display()
            )));
        }

        if !canonical.is_file() {
            return Err(PipelineError::Validation(format!(
                "Path is not a file: {}",
                canonical.display()
            )));
        }

        Ok(())
    }

    pub fn validate_directory(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(PipelineError::Validation(format!(
                "Directory does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(PipelineError::Validation(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        Ok(())
    }

    pub fn validate_markdown_extension(path: &Path) -> Result<()> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => Ok(()),
            _ => Err(PipelineError::Validation(format!(
                "File is not a markdown file: {}",
                path.display()
            ))),
        }
    }

    pub fn validate_content_not_empty(content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(PipelineError::Validation("Content is empty".to_string()));
        }
        Ok(())
    }

    pub fn validate_url(url: &str) -> Result<()> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(PipelineError::Validation(format!(
                "Invalid URL format: {}",
                url
            )));
        }
        Ok(())
    }

    pub fn validate_port(port: u16) -> Result<()> {
        if port == 0 {
            return Err(PipelineError::Validation("Port cannot be 0".to_string()));
        }
        Ok(())
    }

    pub fn validate_batch_size(size: usize) -> Result<()> {
        if size == 0 {
            return Err(PipelineError::Validation(
                "Batch size must be greater than 0".to_string(),
            ));
        }

        if size > 10000 {
            return Err(PipelineError::Validation(
                "Batch size too large (max 10000)".to_string(),
            ));
        }

        Ok(())
    }

    pub fn sanitize_file_path(path: &str) -> String {
        path.replace('\\', "/")
            .replace("//", "/")
            .trim()
            .to_string()
    }

    pub fn truncate_text(text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            text.to_string()
        } else {
            format!("{}...", &text[..max_length])
        }
    }

    pub fn validate_within_base_dir(path: &Path, base_dir: &Path) -> Result<()> {
        let canonical_path = fs::canonicalize(path).map_err(|e| {
            PipelineError::Validation(format!(
                "Cannot canonicalize path {}: {}",
                path.display(),
                e
            ))
        })?;

        let canonical_base = fs::canonicalize(base_dir).map_err(|e| {
            PipelineError::Validation(format!(
                "Cannot canonicalize base dir {}: {}",
                base_dir.display(),
                e
            ))
        })?;

        if !canonical_path.starts_with(&canonical_base) {
            return Err(PipelineError::Validation(format!(
                "Path traversal detected ({} outside {})",
                canonical_path.display(),
                canonical_base.display()
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_file_path() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.md");
        fs::write(&file_path, "test").unwrap();

        assert!(Validator::validate_file_path(&file_path).is_ok());
        assert!(Validator::validate_file_path(Path::new("/nonexistent")).is_err());
    }

    #[test]
    fn test_validate_directory() {
        let temp = TempDir::new().unwrap();
        assert!(Validator::validate_directory(temp.path()).is_ok());
        assert!(Validator::validate_directory(Path::new("/nonexistent")).is_err());
    }

    #[test]
    fn test_validate_markdown_extension() {
        assert!(Validator::validate_markdown_extension(Path::new("test.md")).is_ok());
        assert!(Validator::validate_markdown_extension(Path::new("test.markdown")).is_ok());
        assert!(Validator::validate_markdown_extension(Path::new("test.txt")).is_err());
    }

    #[test]
    fn test_validate_content_not_empty() {
        assert!(Validator::validate_content_not_empty("content").is_ok());
        assert!(Validator::validate_content_not_empty("").is_err());
        assert!(Validator::validate_content_not_empty("   ").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(Validator::validate_url("https://example.com").is_ok());
        assert!(Validator::validate_url("http://example.com").is_ok());
        assert!(Validator::validate_url("example.com").is_err());
        assert!(Validator::validate_url("ftp://example.com").is_err());
    }

    #[test]
    fn test_validate_batch_size() {
        assert!(Validator::validate_batch_size(100).is_ok());
        assert!(Validator::validate_batch_size(0).is_err());
        assert!(Validator::validate_batch_size(10001).is_err());
    }

    #[test]
    fn test_sanitize_file_path() {
        assert_eq!(
            Validator::sanitize_file_path("path\\to\\file"),
            "path/to/file"
        );
        assert_eq!(
            Validator::sanitize_file_path("path//to//file"),
            "path/to/file"
        );
        assert_eq!(
            Validator::sanitize_file_path("  path/to/file  "),
            "path/to/file"
        );
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(Validator::truncate_text("short", 10), "short");
        assert_eq!(
            Validator::truncate_text("this is a very long text", 10),
            "this is a ..."
        );
    }

    #[test]
    fn test_validate_within_base_dir() {
        let base = TempDir::new().unwrap();
        let file_path = base.path().join("nested/file.md");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "# Test").unwrap();

        assert!(Validator::validate_within_base_dir(&file_path, base.path()).is_ok());

        let outside = TempDir::new().unwrap();
        let outside_file = outside.path().join("test.md");
        std::fs::write(&outside_file, "# Test").unwrap();

        assert!(Validator::validate_within_base_dir(&outside_file, base.path()).is_err());
    }
}
