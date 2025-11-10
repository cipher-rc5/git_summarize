// file: src/database/insert.rs
// description: LanceDB batch insertion operations with vector embeddings
// reference: https://docs.rs/lancedb

use crate::database::client::LanceDbClient;
use crate::database::schema::SchemaManager;
use crate::error::{PipelineError, Result};
use crate::models::Document;
use arrow_array::{
    BooleanArray, FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, UInt64Array,
};
use arrow_array::types::Float32Type;
use arrow_array::Float32Array;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct BatchInserter<'a> {
    client: &'a LanceDbClient,
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
    pub fn new(client: &'a LanceDbClient) -> Self {
        Self { client }
    }

    /// Insert a single document with its embedding into LanceDB
    pub async fn insert_document(&self, document: &Document) -> Result<String> {
        let embedding_dim = self.client.embedding_dim();
        let schema = SchemaManager::get_documents_schema(embedding_dim);

        // Generate a dummy embedding for now (in production, use a real embedding model)
        let embedding = Self::generate_embedding(&document.content, embedding_dim);

        let record_batch = Self::create_record_batch(
            schema.clone(),
            vec![document.clone()],
            vec![embedding],
        )?;

        let table_name = self.client.table_name();

        // Check if table exists
        if !self.client.table_exists(table_name).await? {
            // Create table with first batch
            self.client
                .get_connection()
                .create_table(
                    table_name,
                    RecordBatchIterator::new(vec![Ok(record_batch)], schema.clone()),
                )
                .execute()
                .await
                .map_err(|e| {
                    PipelineError::Database(format!("Failed to create table: {}", e))
                })?;
            info!("Created new table: {}", table_name);
        } else {
            // Append to existing table
            let table = self.client.get_table(table_name).await?;
            table
                .add(RecordBatchIterator::new(vec![Ok(record_batch)], schema))
                .execute()
                .await
                .map_err(|e| {
                    PipelineError::Database(format!("Failed to insert document: {}", e))
                })?;
        }

        debug!("Inserted document: {}", document.file_path);
        Ok(document.content_hash.clone())
    }

    /// Create an Arrow RecordBatch from documents and embeddings
    fn create_record_batch(
        schema: Arc<arrow_schema::Schema>,
        documents: Vec<Document>,
        embeddings: Vec<Vec<f32>>,
    ) -> Result<RecordBatch> {
        let len = documents.len();

        // Build arrays for each field
        let ids: StringArray = documents
            .iter()
            .map(|doc| Some(doc.content_hash.clone()))
            .collect();

        let file_paths: StringArray = documents
            .iter()
            .map(|doc| Some(doc.file_path.clone()))
            .collect();

        let relative_paths: StringArray = documents
            .iter()
            .map(|doc| Some(doc.relative_path.clone()))
            .collect();

        let contents: StringArray = documents
            .iter()
            .map(|doc| Some(doc.content.clone()))
            .collect();

        let content_hashes: StringArray = documents
            .iter()
            .map(|doc| Some(doc.content_hash.clone()))
            .collect();

        let file_sizes: UInt64Array = documents.iter().map(|doc| Some(doc.file_size)).collect();

        let last_modifieds: UInt64Array = documents
            .iter()
            .map(|doc| Some(doc.last_modified))
            .collect();

        let parsed_ats: UInt64Array = documents.iter().map(|doc| Some(doc.parsed_at)).collect();

        let normalized: BooleanArray = documents.iter().map(|doc| Some(doc.normalized)).collect();

        // Build embedding array (FixedSizeList of Float32)
        let embedding_values: Float32Array = embeddings
            .iter()
            .flat_map(|emb| emb.iter().copied())
            .collect();

        let embedding_list = FixedSizeListArray::try_new_from_values(
            embedding_values,
            embeddings[0].len() as i32,
        )
        .map_err(|e| PipelineError::Database(format!("Failed to create embedding array: {}", e)))?;

        // Optional metadata fields (null for now)
        let titles: StringArray = (0..len).map(|_| None::<String>).collect();
        let descriptions: StringArray = (0..len).map(|_| None::<String>).collect();
        let languages: StringArray = (0..len).map(|_| None::<String>).collect();
        let repository_urls: StringArray = (0..len).map(|_| None::<String>).collect();

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(ids),
                Arc::new(file_paths),
                Arc::new(relative_paths),
                Arc::new(contents),
                Arc::new(content_hashes),
                Arc::new(file_sizes),
                Arc::new(last_modifieds),
                Arc::new(parsed_ats),
                Arc::new(normalized),
                Arc::new(embedding_list),
                Arc::new(titles),
                Arc::new(descriptions),
                Arc::new(languages),
                Arc::new(repository_urls),
            ],
        )
        .map_err(|e| PipelineError::Database(format!("Failed to create record batch: {}", e)))
    }

    /// Generate a simple embedding (placeholder - in production use a real embedding model)
    fn generate_embedding(text: &str, dim: usize) -> Vec<f32> {
        // This is a placeholder. In production, you would use:
        // - sentence-transformers
        // - OpenAI embeddings API
        // - Local models like all-MiniLM-L6-v2
        // For now, generate a simple deterministic embedding based on text hash
        let hash = text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        (0..dim)
            .map(|i| ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0))
            .collect()
    }

    pub async fn log_processing(
        &self,
        file_path: &str,
        status: &str,
        error_message: &str,
        processing_time_ms: u32,
    ) -> Result<()> {
        // For LanceDB, we could log to a separate table or just use tracing
        if status == "failed" {
            warn!(
                "Processing failed for {}: {} (took {}ms)",
                file_path, error_message, processing_time_ms
            );
        } else {
            debug!(
                "Processing succeeded for {} (took {}ms)",
                file_path, processing_time_ms
            );
        }
        Ok(())
    }

    pub async fn insert_complete_batch(
        &self,
        document: Document,
        _incidents: Vec<crate::models::Incident>,
        _addresses: Vec<crate::models::CryptoAddress>,
        _iocs: Vec<crate::models::Ioc>,
    ) -> Result<InsertStats> {
        let mut stats = InsertStats::default();

        // For LanceDB RAG pipeline, we focus on documents with embeddings
        // Additional entities can be stored as metadata or in separate tables
        self.insert_document(&document).await?;
        stats.documents_inserted = 1;

        // Note: incidents, addresses, and IOCs are ignored in this simplified version
        // They could be stored as JSON metadata or in separate tables if needed

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

    #[test]
    fn test_embedding_generation() {
        let embedding = BatchInserter::generate_embedding("test content", 384);
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&x| x >= 0.0 && x <= 1.0));
    }
}
