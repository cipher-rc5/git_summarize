// file: src/database/schema.rs
// description: database schema management and migrations
// reference: clickhouse ddl operations

use crate::database::client::ClickHouseClient;
use crate::error::Result;
use tracing::{info, warn};

pub struct SchemaManager<'a> {
    client: &'a ClickHouseClient,
}

impl<'a> SchemaManager<'a> {
    pub fn new(client: &'a ClickHouseClient) -> Self {
        Self { client }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing database schema");

        self.create_documents_table().await?;
        self.create_incidents_table().await?;
        self.create_crypto_addresses_table().await?;
        self.create_iocs_table().await?;
        self.create_processing_log_table().await?;

        info!("Database schema initialized successfully");
        Ok(())
    }

    pub async fn verify_schema(&self) -> Result<bool> {
        let tables = vec![
            "documents",
            "incidents",
            "crypto_addresses",
            "iocs",
            "processing_log",
        ];

        for table in tables {
            if !self.client.table_exists(table).await? {
                warn!("Table '{}' does not exist", table);
                return Ok(false);
            }
        }

        info!("All required tables exist");
        Ok(true)
    }

    async fn create_documents_table(&self) -> Result<()> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS documents (
                id UUID DEFAULT generateUUIDv4(),
                file_path String,
                relative_path String,
                content String,
                content_hash String,
                file_size UInt64,
                last_modified UInt64,
                parsed_at UInt64,
                normalized Bool DEFAULT false,
                INDEX path_idx file_path TYPE bloom_filter GRANULARITY 1,
                INDEX hash_idx content_hash TYPE bloom_filter GRANULARITY 1
            ) ENGINE = MergeTree
            ORDER BY (parsed_at, id)
            PARTITION BY toYYYYMM(toDateTime(parsed_at))
            SETTINGS index_granularity = 8192
        "#;

        self.client.get_client().query(query).execute().await?;

        info!("Documents table created/verified");
        Ok(())
    }

    async fn create_incidents_table(&self) -> Result<()> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS incidents (
                id UUID DEFAULT generateUUIDv4(),
                document_id String,
                title String,
                date Int64,
                date_precision String,
                victim String,
                attack_vector String,
                amount_usd Nullable(Float64),
                description String,
                source_file String,
                extracted_at UInt64,
                INDEX title_idx title TYPE tokenbf_v1(32768, 3, 0) GRANULARITY 1,
                INDEX victim_idx victim TYPE tokenbf_v1(32768, 3, 0) GRANULARITY 1
            ) ENGINE = MergeTree
            ORDER BY (date, id)
            SETTINGS index_granularity = 8192
        "#;

        self.client.get_client().query(query).execute().await?;

        info!("Incidents table created/verified");
        Ok(())
    }

    async fn create_crypto_addresses_table(&self) -> Result<()> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS crypto_addresses (
                id UUID DEFAULT generateUUIDv4(),
                address String,
                chain String,
                document_id String,
                file_path String,
                context String,
                attribution String,
                parsed_at UInt64,
                INDEX addr_idx address TYPE bloom_filter GRANULARITY 1
            ) ENGINE = MergeTree
            ORDER BY (chain, address)
            SETTINGS index_granularity = 8192
        "#;

        self.client.get_client().query(query).execute().await?;

        info!("Crypto addresses table created/verified");
        Ok(())
    }

    async fn create_iocs_table(&self) -> Result<()> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS iocs (
                id UUID DEFAULT generateUUIDv4(),
                ioc_type String,
                value String,
                document_id String,
                context String,
                extracted_at UInt64,
                INDEX ioc_idx value TYPE bloom_filter GRANULARITY 1
            ) ENGINE = MergeTree
            ORDER BY (ioc_type, value)
            SETTINGS index_granularity = 8192
        "#;

        self.client.get_client().query(query).execute().await?;

        info!("IOCs table created/verified");
        Ok(())
    }

    async fn create_processing_log_table(&self) -> Result<()> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS processing_log (
                id UUID DEFAULT generateUUIDv4(),
                file_path String,
                status String,
                error_message String DEFAULT '',
                processed_at UInt64,
                processing_time_ms UInt32
            ) ENGINE = MergeTree
            ORDER BY (processed_at, id)
            PARTITION BY toYYYYMM(toDateTime(processed_at))
            SETTINGS index_granularity = 8192
        "#;

        self.client.get_client().query(query).execute().await?;

        info!("Processing log table created/verified");
        Ok(())
    }

    pub async fn drop_all_tables(&self) -> Result<()> {
        warn!("Dropping all tables");

        let tables = vec![
            "documents",
            "incidents",
            "crypto_addresses",
            "iocs",
            "processing_log",
        ];

        for table in tables {
            let query = format!("DROP TABLE IF EXISTS {}", table);
            self.client.get_client().query(&query).execute().await?;
            info!("Dropped table: {}", table);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running ClickHouse instance
    async fn test_schema_creation() {
        use crate::config::DatabaseConfig;

        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 8123,
            database: "test_db".to_string(),
            username: None,
            password: None,
            batch_size: 1000,
        };

        let client = ClickHouseClient::new(config).unwrap();
        let schema_manager = SchemaManager::new(&client);

        let result = schema_manager.initialize().await;
        assert!(result.is_ok());
    }
}
