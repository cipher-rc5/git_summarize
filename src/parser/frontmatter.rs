// file: src/parser/frontmatter.rs
// description: YAML frontmatter extraction from markdown
// reference: https://docs.rs/yaml-rust

use crate::error::{PipelineError, Result};
use std::collections::HashMap;
use yaml_rust::{Yaml, YamlLoader};

pub struct FrontmatterParser;

#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub fields: HashMap<String, String>,
}

impl FrontmatterParser {
    pub fn new() -> Self {
        Self
    }

    pub fn extract(&self, content: &str) -> Result<Option<(Frontmatter, String)>> {
        if !content.starts_with("---") {
            return Ok(None);
        }

        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Ok(None);
        }

        let yaml_content = parts[1].trim();
        let remaining_content = parts[2].trim();

        let docs =
            YamlLoader::load_from_str(yaml_content).map_err(|e| PipelineError::MarkdownParse {
                file: "frontmatter".to_string(),
                message: format!("YAML parse error: {}", e),
            })?;

        if docs.is_empty() {
            return Ok(None);
        }

        let mut fields = HashMap::new();

        if let Yaml::Hash(hash) = &docs[0] {
            for (key, value) in hash {
                if let (Yaml::String(k), Yaml::String(v)) = (key, value) {
                    fields.insert(k.clone(), v.clone());
                } else if let Yaml::String(k) = key {
                    fields.insert(k.clone(), format!("{:?}", value));
                }
            }
        }

        Ok(Some((
            Frontmatter { fields },
            remaining_content.to_string(),
        )))
    }

    pub fn get_field(&self, frontmatter: &Frontmatter, key: &str) -> Option<String> {
        frontmatter.fields.get(key).cloned()
    }
}

impl Default for FrontmatterParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter_extraction() {
        let parser = FrontmatterParser::new();
        let content = "---\ntitle: Test\ndate: 2024-01-01\n---\n\n# Content";

        let result = parser.extract(content).unwrap();
        assert!(result.is_some());

        let (frontmatter, remaining) = result.unwrap();
        assert_eq!(frontmatter.fields.get("title"), Some(&"Test".to_string()));
        assert!(remaining.contains("# Content"));
    }

    #[test]
    fn test_no_frontmatter() {
        let parser = FrontmatterParser::new();
        let content = "# Just a heading";

        let result = parser.extract(content).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_field() {
        let parser = FrontmatterParser::new();
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), "Test Title".to_string());

        let frontmatter = Frontmatter { fields };
        let title = parser.get_field(&frontmatter, "title");

        assert_eq!(title, Some("Test Title".to_string()));
    }
}
