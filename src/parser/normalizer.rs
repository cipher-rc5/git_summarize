// file: src/parser/normalizer.rs
// description: Markdown normalization for structural consistency
// reference: Markdown specification

use crate::error::Result;

pub struct MarkdownNormalizer;

impl MarkdownNormalizer {
    pub fn new() -> Self {
        Self
    }

    pub fn normalize(&self, content: &str) -> Result<String> {
        let mut normalized = content.to_string();

        normalized = self.normalize_headings(&normalized);
        normalized = self.normalize_lists(&normalized);
        normalized = self.normalize_line_breaks(&normalized);
        normalized = self.normalize_code_blocks(&normalized);

        Ok(normalized)
    }

    fn normalize_headings(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                let text = trimmed.trim_start_matches('#').trim();

                if !text.is_empty() && level <= 6 {
                    result.push(format!("{} {}", "#".repeat(level), text));
                } else {
                    result.push(line.to_string());
                }
            } else {
                result.push(line.to_string());
            }
        }

        result.join("\n")
    }

    fn normalize_lists(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();

        for line in lines {
            let trimmed = line.trim_start();

            if let Some(stripped) = trimmed
                .strip_prefix("* ")
                .or_else(|| trimmed.strip_prefix("- "))
                .or_else(|| trimmed.strip_prefix("+ "))
            {
                let indent = line.len() - trimmed.len();
                let text = stripped.trim();
                result.push(format!("{}- {}", " ".repeat(indent), text));
            } else {
                result.push(line.to_string());
            }
        }

        result.join("\n")
    }

    fn normalize_line_breaks(&self, content: &str) -> String {
        content
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .replace("\n\n\n", "\n\n")
    }

    fn normalize_code_blocks(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut in_code_block = false;

        for line in lines {
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
            }
            result.push(line.to_string());
        }

        result.join("\n")
    }
}

impl Default for MarkdownNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_normalization() {
        let normalizer = MarkdownNormalizer::new();
        let content = "#Title\n##  Subtitle  ";
        let normalized = normalizer.normalize(content).unwrap();

        assert!(normalized.contains("# Title"));
        assert!(normalized.contains("## Subtitle"));
    }

    #[test]
    fn test_list_normalization() {
        let normalizer = MarkdownNormalizer::new();
        let content = "* Item 1\n+ Item 2\n- Item 3";
        let normalized = normalizer.normalize(content).unwrap();

        assert!(normalized.contains("- Item 1"));
        assert!(normalized.contains("- Item 2"));
        assert!(normalized.contains("- Item 3"));
    }

    #[test]
    fn test_line_break_normalization() {
        let normalizer = MarkdownNormalizer::new();
        let content = "Line 1\n\n\nLine 2";
        let normalized = normalizer.normalize(content).unwrap();

        assert_eq!(normalized.matches("\n\n").count(), 1);
    }
}
