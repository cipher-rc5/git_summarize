// file: src/mcp/server.rs
// description: Enhanced MCP server with repository management capabilities
// reference: https://docs.rs/rmcp

use crate::config::Config;
use crate::database::{BatchInserter, LanceDbClient, SchemaManager};
use crate::repository::{FileScanner, RepositorySync};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::*;
use rmcp::{tool, tool_router, ErrorData as McpError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Metadata about an ingested repository
#[derive(Debug, Clone)]
struct RepositoryMetadata {
    url: String,
    branch: String,
    commit_hash: String,
    local_path: PathBuf,
    subdirectories: Option<Vec<String>>,
    file_count: usize,
    ingested_at: u64,
}

/// GitSummarizeMcp server with concurrent access controls
///
/// Lock Ordering (to prevent deadlocks, always acquire in this order):
/// 1. config (RwLock) - read-heavy, rarely modified
/// 2. repositories (RwLock) - read-heavy during list/get operations
/// 3. db_client (Mutex) - moderate read/write for database operations
///
/// All locks have 30-second timeouts to prevent indefinite hangs.
#[derive(Clone)]
pub struct GitSummarizeMcp {
    config: Arc<RwLock<Config>>,
    db_client: Arc<Mutex<Option<LanceDbClient>>>,
    repositories: Arc<RwLock<HashMap<String, RepositoryMetadata>>>,
    tool_router: ToolRouter<Self>,
}

/// Lock acquisition timeout (30 seconds)
const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

#[tool_router]
impl GitSummarizeMcp {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            db_client: Arc::new(Mutex::new(None)),
            repositories: Arc::new(RwLock::new(HashMap::new())),
            tool_router: Self::tool_router(),
        }
    }

    /// Acquire config read lock with timeout
    async fn read_config(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Config>, McpError> {
        timeout(LOCK_TIMEOUT, self.config.read())
            .await
            .map_err(|_| McpError {
                code: -32603,
                message: "Timeout acquiring config read lock".to_string(),
                data: None,
            })
    }

    /// Acquire config write lock with timeout
    async fn write_config(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, Config>, McpError> {
        timeout(LOCK_TIMEOUT, self.config.write())
            .await
            .map_err(|_| McpError {
                code: -32603,
                message: "Timeout acquiring config write lock".to_string(),
                data: None,
            })
    }

    /// Acquire repositories read lock with timeout
    async fn read_repositories(&self) -> Result<tokio::sync::RwLockReadGuard<'_, HashMap<String, RepositoryMetadata>>, McpError> {
        timeout(LOCK_TIMEOUT, self.repositories.read())
            .await
            .map_err(|_| McpError {
                code: -32603,
                message: "Timeout acquiring repositories read lock".to_string(),
                data: None,
            })
    }

    /// Acquire repositories write lock with timeout
    async fn write_repositories(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, HashMap<String, RepositoryMetadata>>, McpError> {
        timeout(LOCK_TIMEOUT, self.repositories.write())
            .await
            .map_err(|_| McpError {
                code: -32603,
                message: "Timeout acquiring repositories write lock".to_string(),
                data: None,
            })
    }

    /// Acquire db_client lock with timeout
    async fn lock_db_client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<LanceDbClient>>, McpError> {
        timeout(LOCK_TIMEOUT, self.db_client.lock())
            .await
            .map_err(|_| McpError {
                code: -32603,
                message: "Timeout acquiring database client lock".to_string(),
                data: None,
            })
    }

    pub fn get_tool_router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// Initialize database connection
    async fn ensure_db_connected(&self) -> Result<(), McpError> {
        let mut db_client = self.lock_db_client().await?;
        if db_client.is_none() {
            let config = self.read_config().await?;
            let client = LanceDbClient::new(config.database.clone())
                .await
                .map_err(|e| McpError {
                    code: -32603,
                    message: format!("Failed to connect to LanceDB: {}", e),
                    data: None,
                })?;
            *db_client = Some(client);
        }
        Ok(())
    }

    /// Get repository key for tracking
    fn get_repo_key(url: &str) -> String {
        // Extract repo name from URL
        url.trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or(url)
            .trim_end_matches(".git")
            .to_string()
    }

    #[tool(description = "Ingest a GitHub repository into the RAG pipeline. Supports branch selection and subdirectory filtering.")]
    async fn ingest_repository(
        &self,
        #[arg(description = "GitHub repository URL (e.g., https://github.com/user/repo)")] repo_url: String,
        #[arg(description = "Branch, tag, or commit to checkout (default: main)")] reference: Option<String>,
        #[arg(description = "Specific subdirectories to ingest (comma-separated, e.g., 'src,docs')")] subdirs: Option<String>,
        #[arg(description = "Force reprocess all files even if already ingested")] force: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Ingesting repository {} (ref: {:?}, subdirs: {:?})",
              repo_url, reference, subdirs);

        // Parse subdirectories
        let subdirectories: Option<Vec<String>> = subdirs.map(|s| {
            s.split(',')
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect()
        });

        // Update config with new repository URL
        let local_path = {
            let mut config = self.write_config().await?;
            config.repository.source_url = repo_url.clone();
            if let Some(ref_name) = reference.clone() {
                config.repository.branch = ref_name;
            }
            config.repository.local_path.clone()
        };

        // Sync repository
        let config = self.read_config().await?.clone();
        let sync = RepositorySync::new(config.repository.clone());
        sync.sync().map_err(|e| McpError {
            code: -32603,
            message: format!("Repository sync failed: {}", e),
            data: None,
        })?;

        // Get current commit hash
        let commit_hash = sync.get_current_commit().unwrap_or_else(|_| "unknown".to_string());

        // Ensure DB is connected
        self.ensure_db_connected().await?;

        // Scan files
        let scanner = FileScanner::new(config.pipeline.clone());
        let mut files = scanner
            .scan_directory(&config.repository.local_path)
            .map_err(|e| McpError {
                code: -32603,
                message: format!("Failed to scan directory: {}", e),
                data: None,
            })?;

        // Filter by subdirectories if specified
        if let Some(ref subdirs) = subdirectories {
            files.retain(|file| {
                subdirs.iter().any(|subdir| {
                    file.relative_path.starts_with(subdir) ||
                    file.relative_path.starts_with(&format!("{}/", subdir))
                })
            });
            info!("MCP: Filtered to {} files in subdirectories: {:?}", files.len(), subdirs);
        }

        let file_count = files.len();
        info!("MCP: Found {} files to process", file_count);

        // Get DB client for processing
        let db_guard = self.lock_db_client().await?;
        let client = db_guard.as_ref().ok_or(McpError {
            code: -32603,
            message: "Database not connected".to_string(),
            data: None,
        })?;

        // Initialize schema
        let schema_manager = SchemaManager::new(client);
        schema_manager.initialize().await.map_err(|e| McpError {
            code: -32603,
            message: format!("Schema initialization failed: {}", e),
            data: None,
        })?;

        let mut processed = 0;
        let mut failed = 0;

        // Process files (limit to 100 per request for responsiveness)
        let limit = file_count.min(100);

        // Get max file size from config
        let config_guard = self.read_config().await?;
        let max_file_size_bytes = config_guard.pipeline.max_file_size_mb * 1024 * 1024;
        drop(config_guard);

        for file in files.iter().take(limit) {
            // Enforce file size limit
            if file.size > max_file_size_bytes as u64 {
                warn!(
                    "Skipping {}: file size {} MB exceeds limit of {} MB",
                    file.relative_path,
                    file.size / (1024 * 1024),
                    max_file_size_bytes / (1024 * 1024)
                );
                failed += 1;
                continue;
            }

            let content = match std::fs::read_to_string(&file.path) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to read {}: {}", file.relative_path, e);
                    failed += 1;
                    continue;
                }
            };

            let document = crate::models::Document::new(
                file.path.display().to_string(),
                file.relative_path.clone(),
                content,
                file.modified,
                repo_url.clone(),
            );

            let inserter = BatchInserter::new(client);
            match inserter.insert_document(&document).await {
                Ok(_) => {
                    processed += 1;
                    if processed % 10 == 0 {
                        info!("MCP: Processed {}/{}", processed, limit);
                    }
                }
                Err(e) => {
                    error!("Failed to insert {}: {}", file.relative_path, e);
                    failed += 1;
                }
            }
        }

        // Store repository metadata
        let repo_key = Self::get_repo_key(&repo_url);
        let metadata = RepositoryMetadata {
            url: repo_url.clone(),
            branch: reference.clone().unwrap_or_else(|| "main".to_string()),
            commit_hash: commit_hash.clone(),
            local_path,
            subdirectories: subdirectories.clone(),
            file_count: processed,
            ingested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0))
                .as_secs(),
        };

        self.write_repositories().await?.insert(repo_key, metadata);

        let result_text = format!(
            "Repository ingestion complete:\n\
             \n\
             Repository: {}\n\
             Reference: {}\n\
             Commit: {}\n\
             Subdirectories: {}\n\
             Total files found: {}\n\
             Files processed: {}\n\
             Files failed: {}\n\
             Success rate: {:.1}%\n\
             \n\
             Note: Limited to first 100 files per request.",
            repo_url,
            reference.unwrap_or_else(|| "main".to_string()),
            &commit_hash[..8.min(commit_hash.len())],
            subdirectories.map(|s| s.join(", ")).unwrap_or_else(|| "all".to_string()),
            file_count,
            processed,
            failed,
            if processed + failed > 0 {
                (processed as f64 / (processed + failed) as f64) * 100.0
            } else {
                0.0
            }
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "List all ingested repositories with their metadata")]
    async fn list_repositories(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Listing repositories");

        let repositories = self.read_repositories().await?;

        if repositories.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No repositories have been ingested yet.\n\
                 Use ingest_repository to add a repository."
            )]));
        }

        let mut result = String::from("Ingested Repositories:\n\n");

        for (key, meta) in repositories.iter() {
            let subdirs = meta.subdirectories.as_ref()
                .map(|s| s.join(", "))
                .unwrap_or_else(|| "all".to_string());

            result.push_str(&format!(
                "• {} ({})\n\
                   URL: {}\n\
                   Branch: {}\n\
                   Commit: {}\n\
                   Subdirs: {}\n\
                   Files: {}\n\
                   Ingested: {}\n\n",
                key,
                if meta.url.contains(&key) { "active" } else { "cached" },
                meta.url,
                meta.branch,
                &meta.commit_hash[..8.min(meta.commit_hash.len())],
                subdirs,
                meta.file_count,
                chrono::DateTime::from_timestamp(meta.ingested_at as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Remove a repository and its documents from the database")]
    async fn remove_repository(
        &self,
        #[arg(description = "Repository URL or name to remove")] repo_identifier: String,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Removing repository: {}", repo_identifier);

        // Get repository key
        let repo_key = if repo_identifier.contains("://") {
            Self::get_repo_key(&repo_identifier)
        } else {
            repo_identifier.clone()
        };

        // Check if repository exists
        let mut repositories = self.write_repositories().await?;
        let metadata = repositories.remove(&repo_key).ok_or_else(|| McpError {
            code: -32602,
            message: format!("Repository '{}' not found. Use list_repositories to see available repositories.", repo_key),
            data: None,
        })?;
        drop(repositories); // Release lock early

        // Delete documents from LanceDB
        info!("MCP: Deleting documents for repository: {}", metadata.url);

        // Ensure DB is connected
        self.ensure_db_connected().await?;

        let db_guard = self.lock_db_client().await?;
        let client = db_guard.as_ref().ok_or(McpError {
            code: -32603,
            message: "Database not connected".to_string(),
            data: None,
        })?;

        // Delete all documents belonging to this repository
        match client.delete_by_repository(&metadata.url).await {
            Ok(_) => {
                info!("MCP: Successfully deleted documents for repository: {}", metadata.url);
            }
            Err(e) => {
                warn!("MCP: Failed to delete documents: {}. Metadata removed but documents may remain.", e);
            }
        }
        drop(db_guard);

        let result_text = format!(
            "Repository removed successfully:\n\
             \n\
             Name: {}\n\
             URL: {}\n\
             Files tracked: {}\n\
             \n\
             All documents and metadata have been removed from the database.",
            repo_key,
            metadata.url,
            metadata.file_count
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Update an existing repository to the latest version")]
    async fn update_repository(
        &self,
        #[arg(description = "Repository URL or name to update")] repo_identifier: String,
        #[arg(description = "New branch/tag/commit to checkout (optional)")] new_reference: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Updating repository: {}", repo_identifier);

        // Get repository key
        let repo_key = if repo_identifier.contains("://") {
            Self::get_repo_key(&repo_identifier)
        } else {
            repo_identifier.clone()
        };

        // Get existing metadata
        let repositories = self.read_repositories().await?;
        let old_metadata = repositories.get(&repo_key).ok_or_else(|| McpError {
            code: -32602,
            message: format!("Repository '{}' not found. Use list_repositories to see available repositories.", repo_key),
            data: None,
        })?;

        let url = old_metadata.url.clone();
        let subdirs = old_metadata.subdirectories.clone()
            .map(|s| s.join(","));

        drop(repositories);

        // Re-ingest with force flag
        self.ingest_repository(
            url,
            new_reference.or_else(|| Some(old_metadata.branch.clone())),
            subdirs,
            Some(true), // Force reprocess
        ).await
    }

    #[tool(description = "Get statistics about the ingested documents in the RAG pipeline")]
    async fn get_stats(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Getting statistics");

        self.ensure_db_connected().await?;

        let db_guard = self.lock_db_client().await?;
        let client = db_guard.as_ref().ok_or(McpError {
            code: -32603,
            message: "Database not connected".to_string(),
            data: None,
        })?;

        let doc_count = client.get_document_count().await.map_err(|e| McpError {
            code: -32603,
            message: format!("Failed to get document count: {}", e),
            data: None,
        })?;

        let repos = self.read_repositories().await?;
        let repo_count = repos.len();

        let stats_text = format!(
            "RAG Pipeline Statistics:\n\
             \n\
             Documents:\n\
             - Total documents: {}\n\
             \n\
             Repositories:\n\
             - Tracked repositories: {}\n\
             \n\
             Database:\n\
             - Backend: LanceDB\n\
             - Storage: {}\n\
             - Table: {}",
            doc_count,
            repo_count,
            client
                .get_connection()
                .uri()
                .map(|u| u.to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            client.table_name()
        );

        Ok(CallToolResult::success(vec![Content::text(stats_text)]))
    }

    #[tool(description = "Search for documents by semantic similarity using vector embeddings")]
    async fn search_documents(
        &self,
        #[arg(description = "Search query text")] query: String,
        #[arg(description = "Maximum number of results to return (default: 5)")] limit: Option<usize>,
        #[arg(description = "Filter by repository URL (optional)")] repository_filter: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Searching for documents with query: {}", query);

        self.ensure_db_connected().await?;

        let search_limit = limit.unwrap_or(5);

        // Get database client
        let db_guard = self.lock_db_client().await?;
        let client = db_guard.as_ref().ok_or(McpError {
            code: -32603,
            message: "Database not connected".to_string(),
            data: None,
        })?;

        // Generate embedding for the query
        const EMBEDDING_DIM: usize = 768;
        let query_embedding = if let Some(api_key) = client.groq_api_key() {
            // Use Groq API for embedding
            let groq_client = crate::database::GroqEmbeddingClient::new(
                api_key.clone(),
                client.groq_model().to_string(),
            );

            match groq_client.generate_embedding(&query).await {
                Ok(embedding) => {
                    if embedding.len() != EMBEDDING_DIM {
                        warn!("Groq API returned embedding with dimension {}, expected {}. Using fallback.",
                              embedding.len(), EMBEDDING_DIM);
                        crate::database::GroqEmbeddingClient::generate_fallback_embedding(&query, EMBEDDING_DIM)
                    } else {
                        info!("Using Groq API embedding for search query");
                        embedding
                    }
                }
                Err(e) => {
                    warn!("Groq API embedding failed: {}. Using fallback.", e);
                    crate::database::GroqEmbeddingClient::generate_fallback_embedding(&query, EMBEDDING_DIM)
                }
            }
        } else {
            info!("No API key configured, using fallback embedding for search");
            crate::database::GroqEmbeddingClient::generate_fallback_embedding(&query, EMBEDDING_DIM)
        };

        // Perform vector search
        let results = client
            .vector_search(
                query_embedding,
                search_limit,
                repository_filter.as_deref(),
            )
            .await
            .map_err(|e| McpError {
                code: -32603,
                message: format!("Vector search failed: {}", e),
                data: None,
            })?;

        drop(db_guard);

        // Format results
        if results.is_empty() {
            let result_text = format!(
                "No results found for query: {}\n\
                 \n\
                 Try:\n\
                 - Using different search terms\n\
                 - Removing repository filter\n\
                 - Checking that documents have been ingested",
                query
            );
            return Ok(CallToolResult::success(vec![Content::text(result_text)]));
        }

        let mut result_text = format!(
            "Search Results for: \"{}\"\n\
             Found {} result(s)\n\
             \n",
            query,
            results.len()
        );

        for (idx, result) in results.iter().enumerate() {
            result_text.push_str(&format!(
                "{}. {} (Score: {:.4})\n\
                 Repository: {}\n\
                 Preview: {}\n\
                 \n",
                idx + 1,
                result.relative_path,
                result.score,
                result.repository_url,
                result.format_summary(200).trim()
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Get configuration information about the RAG pipeline")]
    async fn get_config(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Getting configuration");

        let config = self.read_config().await?;

        let config_text = format!(
            "Git Summarize Configuration:\n\
             \n\
             Repository:\n\
             - URL: {}\n\
             - Local path: {}\n\
             - Branch: {}\n\
             - Sync on start: {}\n\
             \n\
             Database:\n\
             - URI: {}\n\
             - Table: {}\n\
             - Batch size: {}\n\
             - Groq model: {}\n\
             \n\
             Pipeline:\n\
             - Parallel workers: {}\n\
             - Max file size: {} MB\n\
             - Force reprocess: {}",
            config.repository.source_url,
            config.repository.local_path.display(),
            config.repository.branch,
            config.repository.sync_on_start,
            config.database.uri,
            config.database.table_name,
            config.database.batch_size,
            config.database.groq_model,
            config.pipeline.parallel_workers,
            config.pipeline.max_file_size_mb,
            config.pipeline.force_reprocess
        );

        Ok(CallToolResult::success(vec![Content::text(config_text)]))
    }

    #[tool(description = "Verify database connection and schema")]
    async fn verify_database(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Verifying database");

        self.ensure_db_connected().await?;

        let db_guard = self.lock_db_client().await?;
        let client = db_guard.as_ref().ok_or(McpError {
            code: -32603,
            message: "Database not connected".to_string(),
            data: None,
        })?;

        let ping_result = client.ping().await.map_err(|e| McpError {
            code: -32603,
            message: format!("Database ping failed: {}", e),
            data: None,
        })?;

        let schema_manager = SchemaManager::new(client);
        let schema_valid = schema_manager.verify_schema().await.map_err(|e| McpError {
            code: -32603,
            message: format!("Schema verification failed: {}", e),
            data: None,
        })?;

        let result_text = format!(
            "Database Verification:\n\
             - Connection: {}\n\
             - Schema: {}\n\
             - Status: Ready for operations",
            if ping_result { "✓ Success" } else { "✗ Failed" },
            if schema_valid { "✓ Valid" } else { "✗ Invalid" }
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let config = Config::default_config();
        let mcp = GitSummarizeMcp::new(config);
        assert!(mcp.get_tool_router().list_tools().len() > 0);
    }

    #[test]
    fn test_repo_key_extraction() {
        assert_eq!(GitSummarizeMcp::get_repo_key("https://github.com/user/repo"), "repo");
        assert_eq!(GitSummarizeMcp::get_repo_key("https://github.com/user/repo.git"), "repo");
        assert_eq!(GitSummarizeMcp::get_repo_key("https://github.com/org/my-project/"), "my-project");
    }
}
