// file: src/models/ioc.rs
// description: indicators of compromise model for threat intelligence
// reference: stix ioc standards

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IocType {
    Ip = 1,
    Domain = 2,
    Hash = 3,
    Email = 4,
    Url = 5,
}

impl IocType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IocType::Ip => "ip",
            IocType::Domain => "domain",
            IocType::Hash => "hash",
            IocType::Email => "email",
            IocType::Url => "url",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ioc {
    pub ioc_type: String,
    pub value: String,
    pub document_id: String,
    pub context: String,
    pub extracted_at: u64,
}

impl Ioc {
    pub fn new(ioc_type: IocType, value: String, context: String) -> Self {
        let extracted_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            ioc_type: ioc_type.as_str().to_string(),
            value,
            document_id: String::new(),
            context,
            extracted_at,
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
    fn test_ioc_creation() {
        let ioc = Ioc::new(
            IocType::Ip,
            "192.168.1.1".to_string(),
            "C2 server IP".to_string(),
        );

        assert_eq!(ioc.ioc_type, "ip");
        assert_eq!(ioc.value, "192.168.1.1");
    }
}
