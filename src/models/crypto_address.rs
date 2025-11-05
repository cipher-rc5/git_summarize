// file: src/models/crypto_address.rs
// description: cryptocurrency address model with chain detection
// reference: blockchain address formats

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainType {
    BTC = 1,
    ETH = 2,
    XMR = 3,
    TRX = 4,
    OTHER = 5,
}

impl ChainType {
    pub fn from_address(address: &str) -> Self {
        if address.starts_with("0x") && address.len() == 42 {
            ChainType::ETH
        } else if address.starts_with("bc1") || address.starts_with('1') || address.starts_with('3')
        {
            ChainType::BTC
        } else if address.starts_with('T') && address.len() == 34 {
            ChainType::TRX
        } else if address.starts_with('4') && (address.len() == 95 || address.len() == 106) {
            ChainType::XMR
        } else {
            ChainType::OTHER
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ChainType::BTC => "BTC",
            ChainType::ETH => "ETH",
            ChainType::XMR => "XMR",
            ChainType::TRX => "TRX",
            ChainType::OTHER => "OTHER",
        }
    }
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct CryptoAddress {
    pub address: String,
    pub chain: String,
    pub document_id: String,
    pub file_path: String,
    pub context: String,
    pub attribution: String,
    pub parsed_at: u64,
}

impl CryptoAddress {
    pub fn new(address: String, file_path: String, context: String, attribution: String) -> Self {
        let chain_type = ChainType::from_address(&address);
        let parsed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            address,
            chain: chain_type.as_str().to_string(),
            document_id: String::new(),
            file_path,
            context,
            attribution,
            parsed_at,
        }
    }

    pub fn with_document_id(mut self, document_id: String) -> Self {
        self.document_id = document_id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_detection_eth() {
        let chain = ChainType::from_address("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        assert_eq!(chain, ChainType::ETH);
    }

    #[test]
    fn test_chain_detection_btc() {
        let chain = ChainType::from_address("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
        assert_eq!(chain, ChainType::BTC);
    }

    #[test]
    fn test_crypto_address_creation() {
        let addr = CryptoAddress::new(
            "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            "/path/to/file.md".to_string(),
            "Found in section about hack".to_string(),
            "lazarus_group".to_string(),
        );

        assert_eq!(addr.chain, "ETH");
        assert!(!addr.address.is_empty());
    }
}
