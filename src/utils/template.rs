// file: src/utils/template.rs
// description: File header template generation and formatting
// reference: Internal code standards

use std::collections::HashMap;

pub struct FileTemplate {
    template: String,
}

impl FileTemplate {
    pub fn new() -> Self {
        Self {
            template:
                "// file: {file_path}\n// description: {description}\n// reference: {reference}"
                    .to_string(),
        }
    }

    pub fn with_custom_template(template: String) -> Self {
        Self { template }
    }

    pub fn generate(&self, file_path: &str, description: &str, reference: &str) -> String {
        self.template
            .replace("{file_path}", file_path)
            .replace("{description}", description)
            .replace("{reference}", reference)
    }

    pub fn generate_with_map(&self, values: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();

        for (key, value) in values {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }

        result
    }

    pub fn parse_header(content: &str) -> Option<HashMap<String, String>> {
        let lines: Vec<&str> = content.lines().take(3).collect();

        if lines.len() < 3 {
            return None;
        }

        let mut map = HashMap::new();

        for line in lines {
            if let Some(stripped) = line.strip_prefix("// ")
                && let Some(colon_pos) = stripped.find(':')
            {
                let key = stripped[..colon_pos].trim().to_string();
                let value = stripped[colon_pos + 1..].trim().to_string();
                map.insert(key, value);
            }
        }

        if map.is_empty() { None } else { Some(map) }
    }

    pub fn validate_header(content: &str) -> bool {
        let lines: Vec<&str> = content.lines().take(3).collect();

        if lines.len() < 3 {
            return false;
        }

        lines[0].starts_with("// file:")
            && lines[1].starts_with("// description:")
            && lines[2].starts_with("// reference:")
    }
}

impl Default for FileTemplate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_generation() {
        let template = FileTemplate::new();
        let result = template.generate("src/main.rs", "Main entry point", "Application bootstrap");

        assert!(result.contains("src/main.rs"));
        assert!(result.contains("Main entry point"));
        assert!(result.contains("Application bootstrap"));
    }

    #[test]
    fn test_template_with_map() {
        let template = FileTemplate::new();
        let mut values = HashMap::new();
        values.insert("file_path".to_string(), "src/lib.rs".to_string());
        values.insert("description".to_string(), "Library root".to_string());
        values.insert("reference".to_string(), "Public API".to_string());

        let result = template.generate_with_map(&values);

        assert!(result.contains("src/lib.rs"));
        assert!(result.contains("Library root"));
        assert!(result.contains("Public API"));
    }

    #[test]
    fn test_parse_header() {
        let content = "// file: src/main.rs\n// description: Main entry point\n// reference: Bootstrap\n\nfn main() {}";
        let parsed = FileTemplate::parse_header(content);

        assert!(parsed.is_some());
        let map = parsed.unwrap();
        assert_eq!(map.get("file"), Some(&"src/main.rs".to_string()));
        assert_eq!(
            map.get("description"),
            Some(&"Main entry point".to_string())
        );
    }

    #[test]
    fn test_validate_header() {
        let valid = "// file: src/main.rs\n// description: Test\n// reference: None\n";
        assert!(FileTemplate::validate_header(valid));

        let invalid = "// something else\n// another line\n";
        assert!(!FileTemplate::validate_header(invalid));
    }
}
