// file: src/extractor/patterns.rs
// description: compiled regex patterns for entity extraction
// reference: https://docs.rs/regex
//
// This module provides pre-compiled regex patterns for extracting various entities
// from text. Patterns are organized by category and can be used selectively based
// on your use case.

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // ============================================================================
    // NETWORK & WEB PATTERNS
    // ============================================================================

    /// Matches IPv4 addresses (e.g., 192.168.1.1, 8.8.8.8)
    pub static ref IP_ADDRESS: Regex = Regex::new(
        r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b"
    ).expect("IP_ADDRESS regex is valid");

    /// Matches domain names (e.g., example.com, api.github.com)
    pub static ref DOMAIN: Regex = Regex::new(
        r"\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}\b"
    ).expect("DOMAIN regex is valid");

    /// Matches email addresses (e.g., user@example.com)
    pub static ref EMAIL: Regex = Regex::new(
        r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"
    ).expect("EMAIL regex is valid");

    /// Matches URLs with http/https protocol
    pub static ref URL: Regex = Regex::new(
        r"\bhttps?://[^\s<>\"{}|\\^`\[\]]+"
    ).expect("URL regex is valid");

    // ============================================================================
    // CRYPTOGRAPHIC HASHES
    // ============================================================================

    /// Matches MD5 hashes (32 hexadecimal characters)
    pub static ref MD5_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{32}\b"
    ).expect("MD5_HASH regex is valid");

    /// Matches SHA-1 hashes (40 hexadecimal characters)
    pub static ref SHA1_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{40}\b"
    ).expect("SHA1_HASH regex is valid");

    /// Matches SHA-256 hashes (64 hexadecimal characters)
    pub static ref SHA256_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{64}\b"
    ).expect("SHA256_HASH regex is valid");

    // ============================================================================
    // DATE & TIME PATTERNS
    // ============================================================================

    /// Matches ISO 8601 dates (e.g., 2024-01-15)
    pub static ref ISO_DATE: Regex = Regex::new(
        r"\b(\d{4})-(\d{2})-(\d{2})\b"
    ).expect("ISO_DATE regex is valid");

    /// Matches month-year format (e.g., January 2024)
    pub static ref MONTH_YEAR: Regex = Regex::new(
        r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{4})\b"
    ).expect("MONTH_YEAR regex is valid");

    // ============================================================================
    // NUMERIC PATTERNS
    // ============================================================================

    /// Matches currency amounts in USD (e.g., $100, $1.5 million)
    pub static ref AMOUNT_USD: Regex = Regex::new(
        r"\$\s*([0-9,]+(?:\.[0-9]{2})?)\s*(?:million|M|billion|B|thousand|K)?"
    ).expect("AMOUNT_USD regex is valid");

    /// Matches version numbers (e.g., v1.2.3, 2.0.1)
    pub static ref VERSION: Regex = Regex::new(
        r"\bv?(\d+)\.(\d+)\.(\d+)(?:-[a-zA-Z0-9.]+)?\b"
    ).expect("VERSION regex is valid");

    // ============================================================================
    // CODE PATTERNS
    // ============================================================================

    /// Matches GitHub repository references (e.g., owner/repo)
    pub static ref GITHUB_REPO: Regex = Regex::new(
        r"\b([a-zA-Z0-9_-]+)/([a-zA-Z0-9_-]+)\b"
    ).expect("GITHUB_REPO regex is valid");

    /// Matches hex color codes (e.g., #FF5733, #abc)
    pub static ref HEX_COLOR: Regex = Regex::new(
        r"#(?:[0-9a-fA-F]{3}){1,2}\b"
    ).expect("HEX_COLOR regex is valid");
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Checks if an IP address is in a private range (RFC 1918)
pub fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("172.16.")
        || ip.starts_with("127.")
}

/// Validates an email address format
pub fn is_valid_email(email: &str) -> bool {
    EMAIL.is_match(email) && email.contains('@')
}

/// Validates a URL format
pub fn is_valid_url(url: &str) -> bool {
    URL.is_match(url) && (url.starts_with("http://") || url.starts_with("https://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Network & Web Pattern Tests
    #[test]
    fn test_ip_pattern() {
        assert!(IP_ADDRESS.is_match("192.168.1.1"));
        assert!(IP_ADDRESS.is_match("8.8.8.8"));
        assert!(IP_ADDRESS.is_match("10.0.0.1"));
        assert!(!IP_ADDRESS.is_match("999.999.999.999"));
        assert!(!IP_ADDRESS.is_match("256.1.1.1"));
    }

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("10.0.0.1"));
        assert!(is_private_ip("172.16.0.1"));
        assert!(is_private_ip("127.0.0.1"));
        assert!(!is_private_ip("8.8.8.8"));
        assert!(!is_private_ip("1.1.1.1"));
    }

    #[test]
    fn test_domain_pattern() {
        assert!(DOMAIN.is_match("example.com"));
        assert!(DOMAIN.is_match("api.github.com"));
        assert!(DOMAIN.is_match("sub.domain.example.org"));
        assert!(!DOMAIN.is_match("not_a_domain"));
    }

    #[test]
    fn test_email_pattern() {
        assert!(EMAIL.is_match("user@example.com"));
        assert!(EMAIL.is_match("test.user+tag@domain.co.uk"));
        assert!(is_valid_email("valid@email.com"));
        assert!(!is_valid_email("invalid-email"));
    }

    #[test]
    fn test_url_pattern() {
        assert!(URL.is_match("https://example.com"));
        assert!(URL.is_match("http://github.com/user/repo"));
        assert!(is_valid_url("https://api.example.com/v1/users"));
        assert!(!is_valid_url("ftp://not-http.com"));
    }

    // Hash Pattern Tests
    #[test]
    fn test_hash_patterns() {
        // MD5 (32 hex chars)
        assert!(MD5_HASH.is_match("5d41402abc4b2a76b9719d911017c592"));
        assert!(!MD5_HASH.is_match("5d41402abc4b2a76b9719d911017c59")); // 31 chars

        // SHA-1 (40 hex chars)
        assert!(SHA1_HASH.is_match("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"));

        // SHA-256 (64 hex chars)
        assert!(SHA256_HASH.is_match(
            "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
        ));
    }

    // Date Pattern Tests
    #[test]
    fn test_date_patterns() {
        assert!(ISO_DATE.is_match("2024-01-15"));
        assert!(ISO_DATE.is_match("2025-12-31"));
        assert!(!ISO_DATE.is_match("2024-13-01")); // Invalid month

        assert!(MONTH_YEAR.is_match("January 2024"));
        assert!(MONTH_YEAR.is_match("December 2025"));
    }

    // Numeric Pattern Tests
    #[test]
    fn test_amount_pattern() {
        assert!(AMOUNT_USD.is_match("$100"));
        assert!(AMOUNT_USD.is_match("$1,000,000"));
        assert!(AMOUNT_USD.is_match("$1.5 million"));
        assert!(AMOUNT_USD.is_match("$2.3B"));
    }

    #[test]
    fn test_version_pattern() {
        assert!(VERSION.is_match("1.2.3"));
        assert!(VERSION.is_match("v2.0.1"));
        assert!(VERSION.is_match("3.14.159"));
        assert!(VERSION.is_match("1.0.0-alpha"));
        assert!(VERSION.is_match("2.1.0-beta.1"));
    }

    // Code Pattern Tests
    #[test]
    fn test_github_repo_pattern() {
        assert!(GITHUB_REPO.is_match("owner/repo"));
        assert!(GITHUB_REPO.is_match("rust-lang/rust"));
        assert!(GITHUB_REPO.is_match("user_name/project-name"));
    }

    #[test]
    fn test_hex_color_pattern() {
        assert!(HEX_COLOR.is_match("#FF5733"));
        assert!(HEX_COLOR.is_match("#abc"));
        assert!(HEX_COLOR.is_match("#000000"));
        assert!(!HEX_COLOR.is_match("#GG5733")); // Invalid hex
    }
}
