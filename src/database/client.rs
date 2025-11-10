// file: src/database/client.rs
// description: LanceDB client wrapper with connection management
// reference: https://docs.rs/lancedb

use crate::config::DatabaseConfig;
use crate::error::{PipelineError, Result};
use lancedb::{connect, Connection, Table};
use tracing::{debug, info};

#[derive(Clone)]
pub struct LanceDbClient {
    connection: Connection,
    config: DatabaseConfig,
}

impl LanceDbClient {
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        info!("Connecting to LanceDB at {}", config.uri);

        let connection = connect(&config.uri)
            .execute()
            .await
            .map_err(|e| PipelineError::Database(format!("Failed to connect to LanceDB: {}", e)))?;

        Ok(Self { connection, config })
    }

    pub fn get_connection(&self) -> &Connection {
        &self.connection
    }

    pub async fn ping(&self) -> Result<bool> {
        debug!("Checking LanceDB connection");

        // Try to list tables as a ping equivalent
        match self.connection.table_names().execute().await {
            Ok(_) => {
                info!("LanceDB connection successful");
                Ok(true)
            }
            Err(e) => Err(PipelineError::Database(format!(
                "LanceDB connection failed: {}",
                e
            ))),
        }
    }

    pub async fn table_exists(&self, table_name: &str) -> Result<bool> {
        let table_names = self
            .connection
            .table_names()
            .execute()
            .await
            .map_err(|e| PipelineError::Database(format!("Failed to list tables: {}", e)))?;

        Ok(table_names.iter().any(|name| name == table_name))
    }

    pub async fn get_table(&self, table_name: &str) -> Result<Table> {
        self.connection
            .open_table(table_name)
            .execute()
            .await
            .map_err(|e| {
                PipelineError::Database(format!("Failed to open table {}: {}", table_name, e))
            })
    }

    pub async fn get_document_count(&self) -> Result<u64> {
        if !self.table_exists(&self.config.table_name).await? {
            return Ok(0);
        }

        let table = self.get_table(&self.config.table_name).await?;
        let count = table
            .count_rows(None)
            .await
            .map_err(|e| PipelineError::Database(format!("Failed to count rows: {}", e)))?;

        Ok(count as u64)
    }

    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    pub fn table_name(&self) -> &str {
        &self.config.table_name
    }

    pub fn groq_api_key(&self) -> Option<&String> {
        self.config.groq_api_key.as_ref()
    }

    pub fn groq_model(&self) -> &str {
        &self.config.groq_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let config = DatabaseConfig {
            uri: "memory://test".to_string(),
            table_name: "test_table".to_string(),
            batch_size: 100,
            groq_api_key: None,
            groq_model: "openai/gpt-oss-120b".to_string(),
        };

        assert_eq!(config.uri, "memory://test");
        assert_eq!(config.table_name, "test_table");
    }
}
