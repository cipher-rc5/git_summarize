// file: src/database/insert.rs
// description: LanceDB batch insertion operations with vector embeddings
// reference: https://docs.rs/lancedb

use crate::database::client::LanceDbClient;
use crate::database::embeddings::EmbeddingClient;
use crate::database::schema::SchemaManager;
use crate::error::{PipelineError, Result};
use crate::models::Document;
use crate::parser::{ChunkOptions, chunk_markdown};
use arrow_array::{
    ArrayRef, BooleanArray, FixedSizeListArray, Float32Array, RecordBatch, StringArray,
    UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct BatchInserter<'a> {
    client: &'a LanceDbClient,
    embedding: Arc<EmbeddingClient>,
    chunk_opts: ChunkOptions,
    allow_fallback: bool,
}

#[derive(Debug, Clone, Default)]
pub struct InsertStats {
    pub documents_inserted: usize,
    pub errors: usize,
}

impl<'a> BatchInserter<'a> {
    pub fn new(client: &'a LanceDbClient, embedding: Arc<EmbeddingClient>) -> Self {
        Self {
            client,
            embedding,
            chunk_opts: ChunkOptions::default(),
            allow_fallback: false,
        }
    }

    pub fn with_options(mut self, chunk_opts: ChunkOptions, allow_fallback: bool) -> Self {
        self.chunk_opts = chunk_opts;
        self.allow_fallback = allow_fallback;
        self
    }

    /// Chunk a file's (normalized) content, embed every chunk, and insert all
    /// chunks as rows. Existing rows for the same file are removed first so
    /// reprocessing does not leave stale chunks. Returns the number of chunks
    /// inserted.
    pub async fn insert_file(
        &self,
        file_path: &str,
        relative_path: &str,
        content: &str,
        last_modified: u64,
        repository_url: &str,
        normalized: bool,
    ) -> Result<usize> {
        let chunks = chunk_markdown(content, &self.chunk_opts);
        if chunks.is_empty() {
            debug!("No chunks produced for {}", relative_path);
            return Ok(0);
        }

        let embedding_texts: Vec<String> = chunks.iter().map(|c| c.embedding_text()).collect();
        let embeddings = self.embed(&embedding_texts).await?;

        let documents: Vec<Document> = chunks
            .iter()
            .map(|chunk| {
                Document::from_chunk(
                    file_path,
                    relative_path,
                    chunk,
                    last_modified,
                    repository_url,
                    normalized,
                )
            })
            .collect();

        let dim = self.embedding.dimension();
        let schema = SchemaManager::get_documents_schema(dim);
        let record_batch = Self::create_record_batch(schema, &documents, &embeddings)?;

        let table_name = self.client.table_name();

        if !self.client.table_exists(table_name).await? {
            self.client
                .get_connection()
                .create_table(table_name, vec![record_batch])
                .execute()
                .await
                .map_err(|e| PipelineError::Database(format!("Failed to create table: {}", e)))?;
            info!("Created new table: {}", table_name);
        } else {
            // Remove any prior chunks for this file before re-inserting.
            self.client
                .delete_by_file(repository_url, relative_path)
                .await?;
            let table = self.client.get_table(table_name).await?;
            table
                .add(vec![record_batch])
                .execute()
                .await
                .map_err(|e| PipelineError::Database(format!("Failed to insert chunks: {}", e)))?;
        }

        debug!("Inserted {} chunk(s) for {}", documents.len(), relative_path);
        Ok(documents.len())
    }

    /// Embed `texts`, applying the configured fallback only when explicitly
    /// enabled. By default a failed call propagates as an error rather than
    /// silently filling the index with non-semantic vectors.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self.embedding.embed_batch(texts).await {
            Ok(vectors) => Ok(vectors),
            Err(e) if self.allow_fallback => {
                warn!("Embedding API failed ({e}); using non-semantic fallback (degraded)");
                let dim = self.embedding.dimension();
                Ok(texts
                    .iter()
                    .map(|t| EmbeddingClient::generate_fallback_embedding(t, dim))
                    .collect())
            }
            Err(e) => Err(e),
        }
    }

    /// Create an Arrow RecordBatch from documents and embeddings
    fn create_record_batch(
        schema: Arc<arrow_schema::Schema>,
        documents: &[Document],
        embeddings: &[Vec<f32>],
    ) -> Result<RecordBatch> {
        let len = documents.len();

        if embeddings.is_empty() || embeddings.len() != len {
            return Err(PipelineError::Database(format!(
                "embedding count {} does not match document count {}",
                embeddings.len(),
                len
            )));
        }

        // Build arrays for each field
        let ids: StringArray = documents.iter().map(|doc| Some(doc.id.clone())).collect();

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

        let chunk_indices: UInt32Array =
            documents.iter().map(|doc| Some(doc.chunk_index)).collect();

        let heading_paths: StringArray = documents
            .iter()
            .map(|doc| Some(doc.heading_path.clone()))
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

        let value_field = Arc::new(Field::new("item", DataType::Float32, true));
        let embedding_list = FixedSizeListArray::try_new(
            value_field,
            embeddings[0].len() as i32,
            Arc::new(embedding_values) as ArrayRef,
            None,
        )
        .map_err(|e| PipelineError::Database(format!("Failed to create embedding array: {}", e)))?;

        // Optional metadata fields
        let titles: StringArray = (0..len).map(|_| None::<String>).collect();
        let descriptions: StringArray = (0..len).map(|_| None::<String>).collect();
        let languages: StringArray = (0..len).map(|_| None::<String>).collect();

        // Repository URL is required for deletion tracking
        let repository_urls: StringArray = documents
            .iter()
            .map(|doc| Some(doc.repository_url.clone()))
            .collect();

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(ids),
                Arc::new(file_paths),
                Arc::new(relative_paths),
                Arc::new(contents),
                Arc::new(content_hashes),
                Arc::new(chunk_indices),
                Arc::new(heading_paths),
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
        let embedding = EmbeddingClient::generate_fallback_embedding("test content", 384);
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&x| (0.0..=1.0).contains(&x)));
    }
}
