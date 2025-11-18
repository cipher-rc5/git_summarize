// file: src/repository/classifier.rs
// description: file classification and category extraction
// reference: configurable path-based classification

use crate::config::{CategoryRule, TopicRule};
use std::path::Path;

pub struct FileClassifier {
    categories: Vec<CategoryRule>,
    topics: Vec<TopicRule>,
}

impl FileClassifier {
    pub fn new(categories: Vec<CategoryRule>, topics: Vec<TopicRule>) -> Self {
        Self { categories, topics }
    }

    /// Extract category from file path based on configured rules.
    /// Returns the first matching category or "general" as default.
    pub fn extract_category(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();

        for rule in &self.categories {
            for keyword in &rule.keywords {
                if path_str.contains(keyword) {
                    return rule.category.clone();
                }
            }
        }

        "general".to_string()
    }

    /// Extract topic from file path based on configured rules.
    /// Returns the first matching topic or None.
    pub fn extract_topic(&self, path: &Path) -> Option<String> {
        let path_str = path.to_string_lossy().to_lowercase();

        for rule in &self.topics {
            if path_str.contains(&rule.keyword.to_lowercase()) {
                return Some(rule.topic.clone());
            }
        }

        None
    }

    pub fn is_summary_file(&self, path: &Path) -> bool {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        file_name.contains("readme") || file_name.contains("summary") || file_name.contains("index")
    }
}

impl Default for FileClassifier {
    fn default() -> Self {
        Self::new(vec![], vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_extraction_with_rules() {
        let categories = vec![
            CategoryRule {
                keywords: vec!["frontend".to_string(), "ui".to_string()],
                category: "frontend".to_string(),
            },
            CategoryRule {
                keywords: vec!["backend".to_string(), "api".to_string()],
                category: "backend".to_string(),
            },
        ];

        let classifier = FileClassifier::new(categories, vec![]);

        let path = Path::new("/repo/frontend/components/button.tsx");
        assert_eq!(classifier.extract_category(path), "frontend");

        let path = Path::new("/repo/backend/api/users.rs");
        assert_eq!(classifier.extract_category(path), "backend");

        let path = Path::new("/repo/docs/readme.md");
        assert_eq!(classifier.extract_category(path), "general");
    }

    #[test]
    fn test_category_extraction_no_rules() {
        let classifier = FileClassifier::new(vec![], vec![]);

        let path = Path::new("/repo/anything/file.md");
        assert_eq!(classifier.extract_category(path), "general");
    }

    #[test]
    fn test_topic_extraction_with_rules() {
        let topics = vec![
            TopicRule {
                keyword: "authentication".to_string(),
                topic: "auth".to_string(),
            },
            TopicRule {
                keyword: "database".to_string(),
                topic: "data".to_string(),
            },
        ];

        let classifier = FileClassifier::new(vec![], topics);

        let path = Path::new("/repo/authentication/login.md");
        assert_eq!(classifier.extract_topic(path), Some("auth".to_string()));

        let path = Path::new("/repo/docs/database_schema.md");
        assert_eq!(classifier.extract_topic(path), Some("data".to_string()));

        let path = Path::new("/repo/frontend/button.tsx");
        assert_eq!(classifier.extract_topic(path), None);
    }

    #[test]
    fn test_topic_extraction_no_rules() {
        let classifier = FileClassifier::new(vec![], vec![]);

        let path = Path::new("/repo/anything/file.md");
        assert_eq!(classifier.extract_topic(path), None);
    }

    #[test]
    fn test_summary_detection() {
        let classifier = FileClassifier::new(vec![], vec![]);

        assert!(classifier.is_summary_file(Path::new("README.md")));
        assert!(classifier.is_summary_file(Path::new("summary.md")));
        assert!(classifier.is_summary_file(Path::new("index.md")));
        assert!(!classifier.is_summary_file(Path::new("implementation.md")));
    }

    #[test]
    fn test_multiple_keywords_same_category() {
        let categories = vec![CategoryRule {
            keywords: vec!["tests".to_string(), "spec".to_string(), "__tests__".to_string()],
            category: "testing".to_string(),
        }];

        let classifier = FileClassifier::new(categories, vec![]);

        assert_eq!(
            classifier.extract_category(Path::new("/repo/tests/unit.rs")),
            "testing"
        );
        assert_eq!(
            classifier.extract_category(Path::new("/repo/component.spec.ts")),
            "testing"
        );
        assert_eq!(
            classifier.extract_category(Path::new("/repo/__tests__/app.test.js")),
            "testing"
        );
    }
}
