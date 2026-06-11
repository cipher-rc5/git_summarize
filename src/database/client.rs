// file: src/database/client.rs
// description: LanceDB client wrapper with connection management
// reference: https://docs.rs/lancedb

use crate::config::DatabaseConfig;
use crate::error::{PipelineError, Result};
use crate::models::{
    SearchResult, SearchResultFileMetadata, SearchResultPaths, SearchResultScoring,
};
use arrow_array::{Float32Array, StringArray, UInt64Array};
use futures::StreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table, connect};
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct LanceDbClient {
    connection: Connection,
    config: DatabaseConfig,
}

/// Escape single quotes for safe interpolation into a SQL string literal.
fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
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

    /// Delete all documents belonging to a specific repository
    pub async fn delete_by_repository(&self, repository_url: &str) -> Result<u64> {
        if !self.table_exists(&self.config.table_name).await? {
            info!("Table does not exist, nothing to delete");
            return Ok(0);
        }

        let table = self.get_table(&self.config.table_name).await?;

        let predicate = format!("repository_url = '{}'", escape_sql_literal(repository_url));

        info!("Deleting documents with predicate: {}", predicate);

        table.delete(&predicate).await.map_err(|e| {
            PipelineError::Database(format!(
                "Failed to delete documents for repository {}: {}",
                repository_url, e
            ))
        })?;

        info!(
            "Successfully deleted documents for repository: {}",
            repository_url
        );
        Ok(0) // LanceDB doesn't return deletion count in this API
    }

    /// Delete all chunks for a single file within a repository. Used to clear
    /// stale chunks before re-inserting a reprocessed file.
    pub async fn delete_by_file(&self, repository_url: &str, relative_path: &str) -> Result<()> {
        if !self.table_exists(&self.config.table_name).await? {
            return Ok(());
        }

        let table = self.get_table(&self.config.table_name).await?;
        let predicate = format!(
            "repository_url = '{}' AND relative_path = '{}'",
            escape_sql_literal(repository_url),
            escape_sql_literal(relative_path)
        );

        table.delete(&predicate).await.map_err(|e| {
            PipelineError::Database(format!(
                "Failed to delete chunks for {}: {}",
                relative_path, e
            ))
        })?;
        Ok(())
    }

    /// Search for documents by vector similarity
    ///
    /// # Arguments
    /// * `query_embedding` - The query vector to search for
    /// * `limit` - Maximum number of results to return (default: 10)
    /// * `repository_filter` - Optional repository URL to filter results
    ///
    /// # Returns
    /// Vector of SearchResult ordered by similarity (highest first)
    pub async fn vector_search(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
        repository_filter: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        if !self.table_exists(&self.config.table_name).await? {
            warn!("Table does not exist, returning empty results");
            return Ok(Vec::new());
        }

        let table = self.get_table(&self.config.table_name).await?;

        info!("Performing vector search with limit {}", limit);

        // Create the search query
        let mut query = table
            .vector_search(query_embedding)
            .map_err(|e| PipelineError::Database(format!("Failed to create vector search: {}", e)))?
            .limit(limit);

        // Add repository filter if provided
        if let Some(repo_url) = repository_filter {
            let filter = format!("repository_url = '{}'", escape_sql_literal(repo_url));
            query = query.only_if(&filter);
            debug!("Applied filter: {}", filter);
        }

        // Execute the search
        let mut results_stream = query
            .execute()
            .await
            .map_err(|e| PipelineError::Database(format!("Vector search failed: {}", e)))?;

        // Convert Arrow RecordBatch results to SearchResult objects
        let mut search_results = Vec::new();

        while let Some(batch_result) = results_stream.next().await {
            let batch = batch_result.map_err(|e| {
                PipelineError::Database(format!("Failed to read result batch: {}", e))
            })?;

            let num_rows = batch.num_rows();

            let ids = batch
                .column_by_name("id")
                .ok_or_else(|| PipelineError::Database("Missing 'id' column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| PipelineError::Database("Invalid 'id' column type".to_string()))?;

            let file_paths = batch
                .column_by_name("file_path")
                .ok_or_else(|| PipelineError::Database("Missing 'file_path' column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'file_path' column type".to_string())
                })?;

            let relative_paths = batch
                .column_by_name("relative_path")
                .ok_or_else(|| {
                    PipelineError::Database("Missing 'relative_path' column".to_string())
                })?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'relative_path' column type".to_string())
                })?;

            let contents = batch
                .column_by_name("content")
                .ok_or_else(|| PipelineError::Database("Missing 'content' column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'content' column type".to_string())
                })?;

            let heading_paths = batch
                .column_by_name("heading_path")
                .ok_or_else(|| {
                    PipelineError::Database("Missing 'heading_path' column".to_string())
                })?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'heading_path' column type".to_string())
                })?;

            let repository_urls = batch
                .column_by_name("repository_url")
                .ok_or_else(|| {
                    PipelineError::Database("Missing 'repository_url' column".to_string())
                })?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'repository_url' column type".to_string())
                })?;

            let file_sizes = batch
                .column_by_name("file_size")
                .ok_or_else(|| PipelineError::Database("Missing 'file_size' column".to_string()))?
                .as_any()
                .downcast_ref::<UInt64Array>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'file_size' column type".to_string())
                })?;

            let last_modifieds = batch
                .column_by_name("last_modified")
                .ok_or_else(|| {
                    PipelineError::Database("Missing 'last_modified' column".to_string())
                })?
                .as_any()
                .downcast_ref::<UInt64Array>()
                .ok_or_else(|| {
                    PipelineError::Database("Invalid 'last_modified' column type".to_string())
                })?;

            // LanceDB returns distance score in a special column
            let distances = batch
                .column_by_name("_distance")
                .and_then(|col| col.as_any().downcast_ref::<Float32Array>());

            // Convert rows to SearchResult
            for i in 0..num_rows {
                let id = ids.value(i).to_string();
                let file_path = file_paths.value(i).to_string();
                let relative_path = relative_paths.value(i).to_string();
                let heading_path = heading_paths.value(i).to_string();
                let content = contents.value(i).to_string();
                let repository_url = repository_urls.value(i).to_string();
                let file_size = file_sizes.value(i);
                let last_modified = last_modifieds.value(i);

                // Get distance and convert to similarity score
                let (score, distance) = if let Some(dist_array) = distances {
                    let dist = dist_array.value(i);
                    // Convert distance to similarity (lower distance = higher similarity)
                    // Common approach: score = 1 / (1 + distance)
                    let similarity = 1.0 / (1.0 + dist);
                    (similarity, Some(dist))
                } else {
                    // If no distance column, use default
                    (1.0, None)
                };

                search_results.push(SearchResult::new(
                    id,
                    SearchResultPaths {
                        file_path,
                        relative_path,
                        heading_path,
                    },
                    content,
                    repository_url,
                    SearchResultScoring { score, distance },
                    SearchResultFileMetadata {
                        file_size,
                        last_modified,
                    },
                ));
            }
        }

        info!("Vector search returned {} results", search_results.len());
        Ok(search_results)
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
        };

        assert_eq!(config.uri, "memory://test");
        assert_eq!(config.table_name, "test_table");
    }
}
