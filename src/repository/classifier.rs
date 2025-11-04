// file: src/repository/classifier.rs
// description: File classification and attribution extraction
// reference: Internal classification logic

use std::path::Path;

pub struct FileClassifier;

impl FileClassifier {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_attribution(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();

        if path_str.contains("hacks_and_thefts") || path_str.contains("hacks-and-thefts") {
            "hacks_and_thefts".to_string()
        } else if path_str.contains("dprk_it_workers") || path_str.contains("dprk-it-workers") {
            "dprk_it_workers".to_string()
        } else if path_str.contains("lazarus") {
            "lazarus_group".to_string()
        } else if path_str.contains("bluenoroff") {
            "bluenoroff_group".to_string()
        } else if path_str.contains("apt38") {
            "apt38".to_string()
        } else {
            "general".to_string()
        }
    }

    pub fn extract_topic(&self, path: &Path) -> Option<String> {
        let path_str = path.to_string_lossy().to_lowercase();

        let topics = [
            ("exchange", "cryptocurrency_exchange"),
            ("defi", "defi_protocol"),
            ("wallet", "wallet_compromise"),
            ("supply_chain", "supply_chain_attack"),
            ("malware", "malware_campaign"),
            ("phishing", "phishing_campaign"),
        ];

        for (keyword, topic) in &topics {
            if path_str.contains(keyword) {
                return Some(topic.to_string());
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
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribution_extraction() {
        let classifier = FileClassifier::new();

        let path = Path::new("/repo/hacks_and_thefts/ronin_hack.md");
        assert_eq!(classifier.extract_attribution(path), "hacks_and_thefts");

        let path = Path::new("/repo/dprk_it_workers/infiltration.md");
        assert_eq!(classifier.extract_attribution(path), "dprk_it_workers");
    }

    #[test]
    fn test_topic_extraction() {
        let classifier = FileClassifier::new();

        let path = Path::new("/repo/exchange_hack.md");
        assert_eq!(
            classifier.extract_topic(path),
            Some("cryptocurrency_exchange".to_string())
        );
    }

    #[test]
    fn test_summary_detection() {
        let classifier = FileClassifier::new();

        assert!(classifier.is_summary_file(Path::new("README.md")));
        assert!(classifier.is_summary_file(Path::new("summary.md")));
        assert!(!classifier.is_summary_file(Path::new("incident.md")));
    }
}
