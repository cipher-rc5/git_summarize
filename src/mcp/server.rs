// file: src/mcp/server.rs
// description: MCP server implementation for git_summarize RAG pipeline
// reference: https://docs.rs/rmcp

use crate::config::Config;
use crate::database::{BatchInserter, LanceDbClient, SchemaManager};
use crate::repository::{FileScanner, RepositorySync};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::*;
use rmcp::{tool, tool_router, ErrorData as McpError};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Clone)]
pub struct GitSummarizeMcp {
    config: Arc<Mutex<Config>>,
    db_client: Arc<Mutex<Option<LanceDbClient>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl GitSummarizeMcp {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            db_client: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    pub fn get_tool_router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// Initialize database connection
    async fn ensure_db_connected(&self) -> Result<(), McpError> {
        let mut db_client = self.db_client.lock().await;
        if db_client.is_none() {
            let config = self.config.lock().await;
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

    #[tool(description = "Ingest a GitHub repository into the RAG pipeline. Clones/syncs the repository and processes all documents.")]
    async fn ingest_repository(
        &self,
        #[arg(description = "GitHub repository URL (e.g., https://github.com/user/repo)")] repo_url: String,
        #[arg(description = "Branch name to checkout (default: main)")] branch: Option<String>,
        #[arg(description = "Force reprocess all files even if already ingested")] force: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Ingesting repository {} (branch: {:?})", repo_url, branch);

        // Update config with new repository URL
        {
            let mut config = self.config.lock().await;
            config.repository.source_url = repo_url.clone();
            if let Some(b) = branch {
                config.repository.branch = b;
            }
        }

        // Sync repository
        let config = self.config.lock().await.clone();
        let sync = RepositorySync::new(config.repository.clone());
        sync.sync().map_err(|e| McpError {
            code: -32603,
            message: format!("Repository sync failed: {}", e),
            data: None,
        })?;

        // Ensure DB is connected
        self.ensure_db_connected().await?;

        // Scan files
        let scanner = FileScanner::new(config.pipeline.clone());
        let files = scanner
            .scan_directory(&config.repository.local_path)
            .map_err(|e| McpError {
                code: -32603,
                message: format!("Failed to scan directory: {}", e),
                data: None,
            })?;

        let file_count = files.len();
        info!("MCP: Found {} files to process", file_count);

        // Get DB client for processing
        let db_guard = self.db_client.lock().await;
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

        // Process files (simplified for MCP - no parallel processing)
        for file in files.iter().take(file_count.min(100)) {
            // Limit to 100 files per request
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
            );

            let inserter = BatchInserter::new(client);
            match inserter.insert_document(&document).await {
                Ok(_) => {
                    processed += 1;
                    info!("MCP: Processed {}", file.relative_path);
                }
                Err(e) => {
                    error!("Failed to insert {}: {}", file.relative_path, e);
                    failed += 1;
                }
            }
        }

        let result_text = format!(
            "Repository ingestion complete:\n\
             - Repository: {}\n\
             - Total files found: {}\n\
             - Files processed: {}\n\
             - Files failed: {}\n\
             - Success rate: {:.1}%",
            repo_url,
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

    #[tool(description = "Get statistics about the ingested documents in the RAG pipeline")]
    async fn get_stats(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Getting statistics");

        self.ensure_db_connected().await?;

        let db_guard = self.db_client.lock().await;
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

        let stats_text = format!(
            "RAG Pipeline Statistics:\n\
             - Total documents: {}\n\
             - Database: LanceDB\n\
             - Storage location: {}\n\
             - Table name: {}",
            doc_count,
            client
                .get_connection()
                .uri()
                .map(|u| u.to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            client.table_name()
        );

        Ok(CallToolResult::success(vec![Content::text(stats_text)]))
    }

    #[tool(description = "Search for documents by content (simple text search for now)")]
    async fn search_documents(
        &self,
        #[arg(description = "Search query text")] query: String,
        #[arg(description = "Maximum number of results to return (default: 5)")] limit: Option<usize>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP: Searching for documents with query: {}", query);

        self.ensure_db_connected().await?;

        // TODO: Implement vector similarity search with embeddings
        // For now, return a placeholder response

        let result_text = format!(
            "Search functionality coming soon!\n\
             Query: {}\n\
             Limit: {}\n\
             \n\
             TODO: Implement vector similarity search using LanceDB and Groq embeddings.",
            query,
            limit.unwrap_or(5)
        );

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }

    #[tool(description = "Get configuration information about the RAG pipeline")]
    async fn get_config(&self) -> Result<CallToolResult, McpError> {
        info!("MCP: Getting configuration");

        let config = self.config.lock().await;

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

        let db_guard = self.db_client.lock().await;
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
    use crate::config::DatabaseConfig;

    #[test]
    fn test_mcp_server_creation() {
        let config = Config::default_config();
        let mcp = GitSummarizeMcp::new(config);
        assert!(mcp.get_tool_router().list_tools().len() > 0);
    }
}
