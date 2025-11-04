// file: src/extractor/crypto.rs
// description: cryptocurrency address extraction with context
// reference: blockchain address validation standards

use crate::extractor::patterns::*;
use crate::models::CryptoAddress;
use std::collections::HashSet;

pub struct CryptoExtractor {
    seen_addresses: HashSet<String>,
}

impl CryptoExtractor {
    pub fn new() -> Self {
        Self {
            seen_addresses: HashSet::new(),
        }
    }

    pub fn extract_from_text(
        &mut self,
        text: &str,
        file_path: &str,
        attribution: &str,
    ) -> Vec<CryptoAddress> {
        let mut addresses = Vec::new();

        // Extract BTC addresses
        for capture in BTC_ADDRESS.find_iter(text) {
            let addr = capture.as_str().to_string();
            if self.seen_addresses.insert(addr.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                addresses.push(CryptoAddress::new(
                    addr,
                    file_path.to_string(),
                    context,
                    attribution.to_string(),
                ));
            }
        }

        // Extract ETH addresses
        for capture in ETH_ADDRESS.find_iter(text) {
            let addr = capture.as_str().to_string();
            if self.seen_addresses.insert(addr.clone()) && is_valid_eth_address(&addr) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                addresses.push(CryptoAddress::new(
                    addr,
                    file_path.to_string(),
                    context,
                    attribution.to_string(),
                ));
            }
        }

        // Extract XMR addresses
        for capture in XMR_ADDRESS.find_iter(text) {
            let addr = capture.as_str().to_string();
            if self.seen_addresses.insert(addr.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                addresses.push(CryptoAddress::new(
                    addr,
                    file_path.to_string(),
                    context,
                    attribution.to_string(),
                ));
            }
        }

        // Extract TRX addresses
        for capture in TRX_ADDRESS.find_iter(text) {
            let addr = capture.as_str().to_string();
            if self.seen_addresses.insert(addr.clone()) {
                let context = self.extract_context_safe(text, capture.start(), capture.end());
                addresses.push(CryptoAddress::new(
                    addr,
                    file_path.to_string(),
                    context,
                    attribution.to_string(),
                ));
            }
        }

        addresses
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

    pub fn reset(&mut self) {
        self.seen_addresses.clear();
    }
}

impl Default for CryptoExtractor {
    fn default() -> Self {
        Self::new()
    }
}

fn is_valid_eth_address(addr: &str) -> bool {
    if !addr.starts_with("0x") {
        return false;
    }

    if addr.len() != 42 {
        return false;
    }

    addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btc_extraction() {
        let mut extractor = CryptoExtractor::new();
        let text = "Send funds to 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa for payment.";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].address, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    }

    #[test]
    fn test_deduplication() {
        let mut extractor = CryptoExtractor::new();
        let text = "Address 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa appears twice: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 1);
    }

    #[test]
    fn test_eth_address() {
        let mut extractor = CryptoExtractor::new();
        let text = "ETH: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 1);
        assert!(addresses[0].address.starts_with("0x"));
    }

    #[test]
    fn test_emoji_in_context() {
        let mut extractor = CryptoExtractor::new();
        let text = "🍎 Apple Pay to 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa for testing 🚀";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 1);
        // Should not panic on emoji boundaries
        assert!(!addresses[0].context.is_empty());
    }

    #[test]
    fn test_unicode_context() {
        let mut extractor = CryptoExtractor::new();
        let text = "支付地址 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa 日本語 🎌";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 1);
        assert!(!addresses[0].context.is_empty());
    }

    #[test]
    fn test_multiple_types() {
        let mut extractor = CryptoExtractor::new();
        let text = "BTC: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa ETH: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb";
        let addresses = extractor.extract_from_text(text, "test.md", "test");

        assert_eq!(addresses.len(), 2);
    }
}
