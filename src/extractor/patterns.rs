// file: src/extractor/patterns.rs
// description: compiled regex patterns for entity extraction
// reference: https://docs.rs/regex

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // Cryptocurrency addresses
    pub static ref BTC_ADDRESS: Regex = Regex::new(
        r"(?i)\b(bc1[a-z0-9]{39,59}|[13][a-km-zA-HJ-NP-Z1-9]{25,34})\b"
    ).expect("BTC_ADDRESS regex is valid");

    pub static ref ETH_ADDRESS: Regex = Regex::new(
        r"(?i)\b0x[a-fA-F0-9]{40}\b"
    ).expect("ETH_ADDRESS regex is valid");

    pub static ref XMR_ADDRESS: Regex = Regex::new(
        r"\b4[0-9AB][1-9A-HJ-NP-Za-km-z]{93,104}\b"
    ).expect("XMR_ADDRESS regex is valid");

    pub static ref TRX_ADDRESS: Regex = Regex::new(
        r"\bT[A-Za-z1-9]{33}\b"
    ).expect("TRX_ADDRESS regex is valid");

    // Network indicators
    pub static ref IP_ADDRESS: Regex = Regex::new(
        r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b"
    ).expect("IP_ADDRESS regex is valid");

    pub static ref DOMAIN: Regex = Regex::new(
        r"\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,}\b"
    ).expect("DOMAIN regex is valid");

    pub static ref EMAIL: Regex = Regex::new(
        r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"
    ).expect("EMAIL regex is valid");

    // File hashes
    pub static ref MD5_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{32}\b"
    ).expect("MD5_HASH regex is valid");

    pub static ref SHA1_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{40}\b"
    ).expect("SHA1_HASH regex is valid");

    pub static ref SHA256_HASH: Regex = Regex::new(
        r"\b[a-fA-F0-9]{64}\b"
    ).expect("SHA256_HASH regex is valid");

    // Financial amounts
    pub static ref AMOUNT_USD: Regex = Regex::new(
        r"\$\s*([0-9,]+(?:\.[0-9]{2})?)\s*(?:million|M|billion|B|thousand|K)?"
    ).expect("AMOUNT_USD regex is valid");

    // Dates
    pub static ref ISO_DATE: Regex = Regex::new(
        r"\b(\d{4})-(\d{2})-(\d{2})\b"
    ).expect("ISO_DATE regex is valid");

    pub static ref MONTH_YEAR: Regex = Regex::new(
        r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{4})\b"
    ).expect("MONTH_YEAR regex is valid");
}

pub fn is_valid_btc_address(address: &str) -> bool {
    BTC_ADDRESS.is_match(address)
}

pub fn is_valid_eth_address(address: &str) -> bool {
    ETH_ADDRESS.is_match(address) && address.len() == 42
}

pub fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("172.16.")
        || ip.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btc_pattern() {
        assert!(BTC_ADDRESS.is_match("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
        assert!(BTC_ADDRESS.is_match("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"));
    }

    #[test]
    fn test_eth_pattern() {
        assert!(ETH_ADDRESS.is_match("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"));
        assert!(!ETH_ADDRESS.is_match("0xinvalid"));
    }

    #[test]
    fn test_ip_pattern() {
        assert!(IP_ADDRESS.is_match("192.168.1.1"));
        assert!(IP_ADDRESS.is_match("8.8.8.8"));
        assert!(!IP_ADDRESS.is_match("999.999.999.999"));
    }

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("10.0.0.1"));
        assert!(!is_private_ip("8.8.8.8"));
    }
}
