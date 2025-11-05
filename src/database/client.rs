// file: src/database/client.rs
// description: clickHouse client wrapper with connection management
// reference: https://docs.rs/clickhouse

use crate::config::DatabaseConfig;
use crate::error::{PipelineError, Result};
use clickhouse::Client;
use tracing::{debug, info};

#[derive(Clone)]
pub struct ClickHouseClient {
    client: Client,
    config: DatabaseConfig,
}

impl ClickHouseClient {
    pub fn new(config: DatabaseConfig) -> Result<Self> {
        info!(
            "Connecting to ClickHouse at {}:{}",
            config.host, config.port
        );

        let url = format!("http://{}:{}", config.host, config.port);

        let mut client = Client::default().with_url(&url);

        if let Some(ref username) = config.username {
            client = client.with_user(username);
        }

        if let Some(ref password) = config.password {
            client = client.with_password(password);
        }

        client = client.with_database(&config.database);

        Ok(Self { client, config })
    }

    pub fn get_client(&self) -> &Client {
        &self.client
    }

    pub async fn ping(&self) -> Result<bool> {
        debug!("Pinging ClickHouse server");

        let result = self.client.query("SELECT 1").fetch_one::<u8>().await;

        match result {
            Ok(_) => {
                info!("ClickHouse connection successful");
                Ok(true)
            }
            Err(e) => Err(PipelineError::Database(e)),
        }
    }

    pub async fn database_exists(&self) -> Result<bool> {
        let count: u64 = self
            .client
            .query("SELECT count() FROM system.databases WHERE name = ?")
            .bind(&self.config.database)
            .fetch_one()
            .await
            .map_err(PipelineError::Database)?;

        Ok(count > 0)
    }

    pub async fn table_exists(&self, table_name: &str) -> Result<bool> {
        let count: u64 = self
            .client
            .query("SELECT count() FROM system.tables WHERE database = ? AND name = ?")
            .bind(&self.config.database)
            .bind(table_name)
            .fetch_one()
            .await
            .map_err(PipelineError::Database)?;

        Ok(count > 0)
    }

    pub async fn get_document_hashes(&self) -> Result<Vec<String>> {
        if !self.table_exists("documents").await? {
            return Ok(Vec::new());
        }

        let query = "SELECT content_hash FROM documents";

        let hashes: Vec<String> = self
            .client
            .query(query)
            .fetch_all()
            .await
            .map_err(PipelineError::Database)?;

        Ok(hashes)
    }

    pub async fn get_document_count(&self) -> Result<u64> {
        if !self.table_exists("documents").await? {
            return Ok(0);
        }

        let count: u64 = self
            .client
            .query("SELECT count() FROM documents")
            .fetch_one()
            .await
            .map_err(PipelineError::Database)?;

        Ok(count)
    }

    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 8123,
            database: "test_db".to_string(),
            username: None,
            password: None,
            batch_size: 1000,
        };

        let client = ClickHouseClient::new(config);
        assert!(client.is_ok());
    }
}
