// file: src/extractor/ioc.rs
// description: indicators of compromise extraction with filtering with safe UTF-8 handling for emoji and multi-byte characters
// reference: threat intelligence ioc standards

use crate::extractor::patterns::{DOMAIN, EMAIL, IP_ADDRESS, SHA256_HASH};
use crate::models::{Ioc, IocType};
use std::collections::HashSet;

pub struct IocExtractor {
    seen_iocs: HashSet<String>,
    common_domains: HashSet<String>,
}

impl IocExtractor {
    pub fn new() -> Self {
        let common_domains = [
            "github.com",
            "google.com",
            "microsoft.com",
            "apple.com",
            "amazon.com",
            "example.com",
            "localhost",
            "archive.ph",
            "archive.org",
            "web.archive.org",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            seen_iocs: HashSet::new(),
            common_domains,
        }
    }

    pub fn extract_from_text(&mut self, text: &str) -> Vec<Ioc> {
        let mut iocs = Vec::new();

        // Extract IP addresses
        for capture in IP_ADDRESS.find_iter(text) {
            let ip = capture.as_str().to_string();
            if !is_private_ip(&ip) && self.seen_iocs.insert(ip.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                iocs.push(Ioc::new(IocType::Ip, ip, context));
            }
        }

        // Extract domains
        for capture in DOMAIN.find_iter(text) {
            let domain = capture.as_str().to_lowercase();
            if !self.common_domains.contains(&domain) && self.seen_iocs.insert(domain.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                iocs.push(Ioc::new(IocType::Domain, domain, context));
            }
        }

        // Extract hashes (SHA256 prioritized)
        for capture in SHA256_HASH.find_iter(text) {
            let hash = capture.as_str().to_lowercase();
            if self.seen_iocs.insert(hash.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                iocs.push(Ioc::new(IocType::Hash, hash, context));
            }
        }

        // Extract emails
        for capture in EMAIL.find_iter(text) {
            let email = capture.as_str().to_lowercase();
            if self.seen_iocs.insert(email.clone()) && !self.is_common_email(&email) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                iocs.push(Ioc::new(IocType::Email, email, context));
            }
        }

        iocs
    }

    fn extract_context_safe(&self, text: &str, start: usize, end: usize) -> String {
        const CONTEXT_WINDOW: usize = 100;

        // Find safe character boundaries
        let context_start =
            self.find_char_boundary_before(text, start.saturating_sub(CONTEXT_WINDOW));
        let context_end =
            self.find_char_boundary_after(text, (end + CONTEXT_WINDOW).min(text.len()));

        text[context_start..context_end]
            .trim()
            .replace('\n', " ")
            .replace("  ", " ")
    }

    fn find_char_boundary_before(&self, text: &str, pos: usize) -> usize {
        let mut pos = pos.min(text.len());
        while pos > 0 && !text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn find_char_boundary_after(&self, text: &str, pos: usize) -> usize {
        let mut pos = pos.min(text.len());
        while pos < text.len() && !text.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }

    fn is_common_email(&self, email: &str) -> bool {
        email.ends_with("@example.com")
            || email.ends_with("@test.com")
            || email.ends_with("@localhost")
    }

    pub fn reset(&mut self) {
        self.seen_iocs.clear();
    }
}

impl Default for IocExtractor {
    fn default() -> Self {
        Self::new()
    }
}

fn is_private_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    if let Ok(first) = parts[0].parse::<u8>() {
        match first {
            10 => true,
            172 => {
                if let Ok(second) = parts[1].parse::<u8>() {
                    (16..=31).contains(&second)
                } else {
                    false
                }
            }
            192 => parts[1] == "168",
            127 => true,
            _ => false,
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_extraction() {
        let mut extractor = IocExtractor::new();
        let text = "C2 server at 1.2.3.4 was identified.";
        let iocs = extractor.extract_from_text(text);

        assert_eq!(iocs.len(), 1);
        assert_eq!(iocs[0].value, "1.2.3.4");
    }

    #[test]
    fn test_private_ip_filtering() {
        let mut extractor = IocExtractor::new();
        let text = "Local server at 192.168.1.1 and 10.0.0.1";
        let iocs = extractor.extract_from_text(text);

        assert_eq!(iocs.len(), 0);
    }

    #[test]
    fn test_emoji_handling() {
        let mut extractor = IocExtractor::new();
        let text = "🚨 Alert! Malicious IP: 1.2.3.4 detected 🚨";
        let iocs = extractor.extract_from_text(text);

        assert_eq!(iocs.len(), 1);
        assert_eq!(iocs[0].value, "1.2.3.4");
        // Should not panic on emoji boundaries
    }

    #[test]
    fn test_unicode_context() {
        let mut extractor = IocExtractor::new();
        let text = "中文字符 IP address 1.2.3.4 日本語 🎌";
        let iocs = extractor.extract_from_text(text);

        assert_eq!(iocs.len(), 1);
        // Context should be extracted safely
        assert!(!iocs[0].context.is_empty());
    }

    #[test]
    fn test_domain_extraction() {
        let mut extractor = IocExtractor::new();
        let text = "Visit malicious.com for more info";
        let iocs = extractor.extract_from_text(text);

        assert_eq!(iocs.len(), 1);
        assert_eq!(iocs[0].value, "malicious.com");
    }

    #[test]
    fn test_hash_extraction() {
        let mut extractor = IocExtractor::new();
        let text = "File hash: a".repeat(64);
        let iocs = extractor.extract_from_text(&text);

        assert_eq!(iocs.len(), 1);
    }
}
