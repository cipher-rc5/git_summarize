// file: src/database/schema.rs
// description: LanceDB schema management for vector storage
// reference: https://docs.rs/lancedb

use crate::database::client::LanceDbClient;
use crate::error::Result;
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;
use tracing::{info, warn};

pub struct SchemaManager<'a> {
    client: &'a LanceDbClient,
}

impl<'a> SchemaManager<'a> {
    pub fn new(client: &'a LanceDbClient) -> Self {
        Self { client }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing LanceDB schema");

        // Check if the main documents table exists
        if !self.client.table_exists(self.client.table_name()).await? {
            info!("Creating documents table with vector embeddings");
            // Table will be created on first insert with proper schema
        } else {
            info!("Documents table already exists");
        }

        info!("LanceDB schema initialized successfully");
        Ok(())
    }

    pub async fn verify_schema(&self) -> Result<bool> {
        let table_name = self.client.table_name();

        if !self.client.table_exists(table_name).await? {
            warn!("Table '{}' does not exist", table_name);
            return Ok(false);
        }

        info!("Table '{}' exists", table_name);
        Ok(true)
    }

    /// Returns the Arrow schema for the documents table with vector embeddings
    pub fn get_documents_schema(embedding_dim: usize) -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("file_path", DataType::Utf8, false),
            Field::new("relative_path", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("content_hash", DataType::Utf8, false),
            Field::new("file_size", DataType::UInt64, false),
            Field::new("last_modified", DataType::UInt64, false),
            Field::new("parsed_at", DataType::UInt64, false),
            Field::new("normalized", DataType::Boolean, false),
            // Vector embedding field for RAG
            Field::new(
                "embedding",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    embedding_dim as i32,
                ),
                false,
            ),
            // Optional metadata fields
            Field::new("title", DataType::Utf8, true),
            Field::new("description", DataType::Utf8, true),
            Field::new("language", DataType::Utf8, true),
            // Required for repository tracking and deletion
            Field::new("repository_url", DataType::Utf8, false),
        ]))
    }

    pub async fn drop_all_tables(&self) -> Result<()> {
        warn!("Dropping all tables in LanceDB");

        let table_name = self.client.table_name();

        // Drop the main table
        if self.client.table_exists(table_name).await? {
            self.client
                .get_connection()
                .drop_table(table_name)
                .await
                .map_err(|e| {
                    crate::error::PipelineError::Database(format!(
                        "Failed to drop table {}: {}",
                        table_name, e
                    ))
                })?;
            info!("Dropped table: {}", table_name);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_generation() {
        let schema = SchemaManager::get_documents_schema(384);
        assert_eq!(schema.fields().len(), 14);

        let embedding_field = schema.field_with_name("embedding").unwrap();
        assert!(matches!(embedding_field.data_type(), DataType::FixedSizeList(_, 384)));
    }
}
