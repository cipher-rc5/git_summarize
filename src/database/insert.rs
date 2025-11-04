// file: src/database/insert.rs
// description: batch insertion operations with error handling
// reference: clickhouse insert patterns

use crate::database::client::ClickHouseClient;
use crate::error::Result;
use crate::models::{CryptoAddress, Document, Incident, Ioc};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct BatchInserter<'a> {
    client: &'a ClickHouseClient,
}

#[derive(Debug, Clone, Default)]
pub struct InsertStats {
    pub documents_inserted: usize,
    pub incidents_inserted: usize,
    pub addresses_inserted: usize,
    pub iocs_inserted: usize,
    pub errors: usize,
}

impl<'a> BatchInserter<'a> {
    pub fn new(client: &'a ClickHouseClient) -> Self {
        Self { client }
    }

    pub async fn insert_document(&self, document: &Document) -> Result<String> {
        let mut inserter = self
            .client
            .get_client()
            .insert::<Document>("documents")
            .await?;
        inserter.write(document).await?;
        inserter.end().await?;

        let document_id = Uuid::new_v4().to_string();
        debug!("Inserted document: {}", document.file_path);

        Ok(document_id)
    }

    pub async fn insert_documents_batch(&self, documents: &[Document]) -> Result<usize> {
        if documents.is_empty() {
            return Ok(0);
        }

        let mut inserter = self
            .client
            .get_client()
            .insert::<Document>("documents")
            .await?;

        for doc in documents {
            inserter.write(doc).await?;
        }

        inserter.end().await?;

        info!("Inserted {} documents", documents.len());
        Ok(documents.len())
    }

    pub async fn insert_incidents_batch(&self, incidents: &[Incident]) -> Result<usize> {
        if incidents.is_empty() {
            return Ok(0);
        }

        let mut inserter = self
            .client
            .get_client()
            .insert::<Incident>("incidents")
            .await?;

        for incident in incidents {
            inserter.write(incident).await?;
        }

        inserter.end().await?;

        info!("Inserted {} incidents", incidents.len());
        Ok(incidents.len())
    }

    pub async fn insert_crypto_addresses_batch(
        &self,
        addresses: &[CryptoAddress],
    ) -> Result<usize> {
        if addresses.is_empty() {
            return Ok(0);
        }

        let mut inserter = self
            .client
            .get_client()
            .insert::<CryptoAddress>("crypto_addresses")
            .await?;

        for address in addresses {
            inserter.write(address).await?;
        }

        inserter.end().await?;

        info!("Inserted {} crypto addresses", addresses.len());
        Ok(addresses.len())
    }

    pub async fn insert_iocs_batch(&self, iocs: &[Ioc]) -> Result<usize> {
        if iocs.is_empty() {
            return Ok(0);
        }

        let mut inserter = self.client.get_client().insert::<Ioc>("iocs").await?;

        for ioc in iocs {
            inserter.write(ioc).await?;
        }

        inserter.end().await?;

        info!("Inserted {} IOCs", iocs.len());
        Ok(iocs.len())
    }

    pub async fn log_processing(
        &self,
        file_path: &str,
        status: &str,
        error_message: &str,
        processing_time_ms: u32,
    ) -> Result<()> {
        let query = format!(
            "INSERT INTO processing_log (file_path, status, error_message, processed_at, processing_time_ms) VALUES ('{}', '{}', '{}', {}, {})",
            file_path.replace('\'', "''"),
            status,
            error_message.replace('\'', "''"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            processing_time_ms
        );

        self.client.get_client().query(&query).execute().await?;

        Ok(())
    }

    pub async fn insert_complete_batch(
        &self,
        document: Document,
        incidents: Vec<Incident>,
        addresses: Vec<CryptoAddress>,
        iocs: Vec<Ioc>,
    ) -> Result<InsertStats> {
        let mut stats = InsertStats::default();

        let document_id = self.insert_document(&document).await?;
        stats.documents_inserted = 1;

        let incidents_with_id: Vec<Incident> = incidents
            .into_iter()
            .map(|i| i.with_document_id(document_id.clone()))
            .collect();

        if !incidents_with_id.is_empty() {
            match self.insert_incidents_batch(&incidents_with_id).await {
                Ok(count) => stats.incidents_inserted = count,
                Err(e) => {
                    warn!("Failed to insert incidents: {}", e);
                    stats.errors += 1;
                }
            }
        }

        let addresses_with_id: Vec<CryptoAddress> = addresses
            .into_iter()
            .map(|a| a.with_document_id(document_id.clone()))
            .collect();

        if !addresses_with_id.is_empty() {
            match self.insert_crypto_addresses_batch(&addresses_with_id).await {
                Ok(count) => stats.addresses_inserted = count,
                Err(e) => {
                    warn!("Failed to insert crypto addresses: {}", e);
                    stats.errors += 1;
                }
            }
        }

        let iocs_with_id: Vec<Ioc> = iocs
            .into_iter()
            .map(|i| i.with_document_id(document_id.clone()))
            .collect();

        if !iocs_with_id.is_empty() {
            match self.insert_iocs_batch(&iocs_with_id).await {
                Ok(count) => stats.iocs_inserted = count,
                Err(e) => {
                    warn!("Failed to insert IOCs: {}", e);
                    stats.errors += 1;
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_stats_default() {
        let stats = InsertStats::default();
        assert_eq!(stats.documents_inserted, 0);
        assert_eq!(stats.incidents_inserted, 0);
        assert_eq!(stats.addresses_inserted, 0);
        assert_eq!(stats.iocs_inserted, 0);
        assert_eq!(stats.errors, 0);
    }
}
