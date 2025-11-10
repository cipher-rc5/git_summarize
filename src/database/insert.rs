// file: src/database/insert.rs
// description: LanceDB batch insertion operations with vector embeddings
// reference: https://docs.rs/lancedb

use crate::database::client::LanceDbClient;
use crate::database::embeddings::GroqEmbeddingClient;
use crate::database::schema::SchemaManager;
use crate::error::{PipelineError, Result};
use crate::models::Document;
use arrow_array::{
    BooleanArray, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
    UInt64Array,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct BatchInserter<'a> {
    client: &'a LanceDbClient,
    embedding_client: Option<Arc<GroqEmbeddingClient>>,
}

#[derive(Debug, Clone, Default)]
pub struct InsertStats {
    pub documents_inserted: usize,
    pub errors: usize,
}

impl<'a> BatchInserter<'a> {
    pub fn new(client: &'a LanceDbClient) -> Self {
        // Try to create Groq client from config if API key is present
        let embedding_client = client
            .groq_api_key()
            .map(|key| {
                Arc::new(GroqEmbeddingClient::new(
                    key.clone(),
                    client.groq_model().to_string(),
                ))
            });

        if embedding_client.is_some() {
            info!("BatchInserter initialized with Groq API embeddings");
        } else {
            warn!("BatchInserter initialized without API key - using fallback embeddings");
        }

        Self {
            client,
            embedding_client,
        }
    }

    /// Insert a single document with its embedding into LanceDB
    pub async fn insert_document(&self, document: &Document) -> Result<String> {
        // Fixed embedding dimension (can be made configurable later)
        const EMBEDDING_DIM: usize = 768;
        let schema = SchemaManager::get_documents_schema(EMBEDDING_DIM);

        // Generate embedding using Groq API or fallback
        let embedding = self.generate_embedding(&document.content, EMBEDDING_DIM).await?;

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

    /// Generate embedding using Groq API or fallback to deterministic embeddings
    async fn generate_embedding(&self, text: &str, dim: usize) -> Result<Vec<f32>> {
        // Try to use Groq API if available
        if let Some(ref client) = self.embedding_client {
            match client.generate_embedding(text).await {
                Ok(embedding) => {
                    // Verify embedding dimension matches expected
                    if embedding.len() != dim {
                        warn!(
                            "Groq API returned embedding with dimension {}, expected {}. Using fallback.",
                            embedding.len(),
                            dim
                        );
                        Ok(GroqEmbeddingClient::generate_fallback_embedding(text, dim))
                    } else {
                        debug!("Generated Groq API embedding for {} chars", text.len());
                        Ok(embedding)
                    }
                }
                Err(e) => {
                    warn!("Groq API embedding failed: {}. Using fallback.", e);
                    Ok(GroqEmbeddingClient::generate_fallback_embedding(text, dim))
                }
            }
        } else {
            // No API key configured, use fallback
            debug!("Using fallback embedding (no API key configured)");
            Ok(GroqEmbeddingClient::generate_fallback_embedding(text, dim))
        }
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

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_stats_default() {
        let stats = InsertStats::default();
        assert_eq!(stats.documents_inserted, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_fallback_embedding_generation() {
        let embedding = GroqEmbeddingClient::generate_fallback_embedding("test content", 384);
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&x| x >= 0.0 && x <= 1.0));
    }
}
