// file: src/cli.rs
// description: commandline application entry point with command handling
// reference: application bootstrap and orchestration

use crate::mcp::GitSummarizeMcp;
use crate::{
    AnswerGenerator, BatchInserter, ChunkOptions, Config, EmbeddingClient, FileScanner,
    JsonExporter, LanceDbClient, MarkdownNormalizer, MarkdownParser, RepositorySync, SchemaManager,
    Validator,
};
use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use futures::stream::{self, StreamExt};
use rmcp::service::ServiceExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{stdin, stdout};
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "git_summarize")]
#[command(author = "cipher")]
#[command(version = "0.1.0")]
#[command(about = "RAG pipeline for GitHub repositories using LanceDB", long_about = None)]
struct Cli {
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "config/default.toml"
    )]
    config: PathBuf,

    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    color: bool,

    #[arg(short, long, action = ArgAction::SetTrue)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Sync {
        #[arg(long)]
        force: bool,
    },

    Ingest {
        #[arg(long)]
        force: bool,

        #[arg(long)]
        skip_sync: bool,

        #[arg(long, value_name = "NUM")]
        limit: Option<usize>,
    },

    Verify {
        #[arg(long)]
        create_schema: bool,
    },

    Stats,

    Reset {
        #[arg(long)]
        confirm: bool,
    },

    Export {
        #[arg(short, long, default_value = "./exports")]
        output: PathBuf,

        #[arg(short, long)]
        pretty: bool,

        #[arg(long)]
        document_hash: Option<String>,

        #[arg(long)]
        query: Option<String>,
    },

    /// Start MCP (Model Context Protocol) server for agentic tool integration
    Mcp {
        #[arg(long, default_value = "stdio")]
        transport: String,
    },

    /// Search for documents by semantic similarity
    Search {
        /// Search query text
        query: String,

        #[arg(short, long, default_value_t = 5)]
        limit: usize,

        #[arg(short, long)]
        repository: Option<String>,
    },

    /// Ask a natural-language question; retrieves context and synthesizes a cited answer
    Ask {
        /// The question to answer
        question: String,

        /// Number of chunks to retrieve as context
        #[arg(short, long, default_value_t = 8)]
        limit: usize,

        #[arg(short, long)]
        repository: Option<String>,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    crate::utils::logging::init_logger(cli.color, cli.verbose);

    info!("Git Summarize RAG Pipeline");
    info!("Loading configuration from: {}", cli.config.display());

    let config = if cli.config.exists() {
        Config::load(Some(cli.config.as_path())).context("Failed to load configuration")?
    } else {
        warn!(
            "Config file {} not found, using default configuration",
            cli.config.display()
        );
        Config::load(None).unwrap_or_else(|e| {
            warn!("Falling back to built-in defaults: {}", e);
            Config::default_config()
        })
    };

    match cli.command {
        Commands::Sync { force } => {
            cmd_sync(&config, force).await?;
        }
        Commands::Ingest {
            force,
            skip_sync,
            limit,
        } => {
            cmd_ingest(&config, force, skip_sync, limit).await?;
        }
        Commands::Verify { create_schema } => {
            cmd_verify(&config, create_schema).await?;
        }
        Commands::Stats => {
            cmd_stats(&config).await?;
        }
        Commands::Reset { confirm } => {
            cmd_reset(&config, confirm).await?;
        }
        Commands::Export {
            output,
            pretty,
            document_hash,
            query,
        } => {
            cmd_export(&config, output, pretty, document_hash, query).await?;
        }
        Commands::Mcp { transport } => {
            cmd_mcp(&config, &transport).await?;
        }
        Commands::Search {
            query,
            limit,
            repository,
        } => {
            cmd_search(&config, &query, limit, repository.as_deref()).await?;
        }
        Commands::Ask {
            question,
            limit,
            repository,
        } => {
            cmd_ask(&config, &question, limit, repository.as_deref()).await?;
        }
    }

    Ok(())
}

async fn cmd_sync(config: &Config, force: bool) -> Result<()> {
    info!("Synchronizing repository");

    let sync = RepositorySync::new(config.repository.clone());

    if force || config.repository.sync_on_start {
        sync.sync().context("Repository sync failed")?;
        let commit = sync.get_current_commit()?;
        info!("Current commit: {}", commit);
    } else {
        info!("Sync skipped (use --force to sync anyway)");
    }

    Ok(())
}

async fn cmd_ingest(
    config: &Config,
    force: bool,
    skip_sync: bool,
    limit: Option<usize>,
) -> Result<()> {
    info!("Starting ingestion pipeline");
    let start_time = Instant::now();

    if !skip_sync && config.repository.sync_on_start {
        info!("Syncing repository first");
        cmd_sync(config, false).await?;
    }

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let schema_manager = SchemaManager::new(&client);
    if !schema_manager.verify_schema().await? {
        warn!("Database schema incomplete, initializing");
        schema_manager
            .initialize()
            .await
            .context("Failed to initialize schema")?;
    }

    let scanner = FileScanner::new(config.pipeline.clone());
    let files = scanner
        .scan_directory(&config.repository.local_path)
        .context("Failed to scan directory")?;

    info!("Found {} files to process", files.len());

    let files_to_process = if let Some(limit) = limit {
        files.into_iter().take(limit).collect()
    } else {
        files
    };

    let mut config_modified = config.clone();
    config_modified.pipeline.force_reprocess = force;

    let processed = process_files(&client, &config_modified, files_to_process).await?;

    let elapsed = start_time.elapsed();
    info!("Ingestion complete in {:.2}s", elapsed.as_secs_f64());
    info!("Processed {} files", processed);

    Ok(())
}

async fn cmd_export(
    config: &Config,
    output: PathBuf,
    pretty: bool,
    document_hash: Option<String>,
    query: Option<String>,
) -> Result<()> {
    info!("Initializing JSON export");

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let exporter = JsonExporter::new(output)?;

    if let Some(hash) = document_hash {
        exporter.export_single(&client, &hash, pretty).await?;
    } else if let Some(custom_query) = query {
        let count = exporter
            .export_filtered(&client, &custom_query, pretty)
            .await?;
        info!("Exported {} documents with custom query", count);
    } else {
        let manifest = exporter.export_all(&client, pretty).await?;
        info!("Export complete: {} files generated", manifest.files.len());
    }

    Ok(())
}

async fn process_files(
    client: &LanceDbClient,
    config: &Config,
    files: Vec<crate::ScannedFile>,
) -> Result<usize> {
    let client = Arc::new(client.clone());
    let markdown_parser = Arc::new(MarkdownParser::new());
    let normalizer = Arc::new(MarkdownNormalizer::new());
    let embedding = Arc::new(EmbeddingClient::new(config.embedding.clone()));
    let config = Arc::new(config.clone());

    let parallel_workers = config.pipeline.parallel_workers.max(1);

    let results = stream::iter(files.into_iter().map(|file| {
        let client = Arc::clone(&client);
        let markdown_parser = Arc::clone(&markdown_parser);
        let normalizer = Arc::clone(&normalizer);
        let embedding = Arc::clone(&embedding);
        let config = Arc::clone(&config);

        async move {
            let file_start = Instant::now();
            let inserter = BatchInserter::new(client.as_ref(), embedding).with_options(
                ChunkOptions::default(),
                config.embedding.allow_fallback,
            );

            let result = process_single_file(
                &inserter,
                markdown_parser.as_ref(),
                normalizer.as_ref(),
                config.as_ref(),
                &file,
            )
            .await;

            let processing_time = file_start.elapsed().as_millis() as u32;
            let status = if result.is_ok() { "success" } else { "failed" };
            let error_message = match &result {
                Ok(_) => String::new(),
                Err(err) => err.to_string(),
            };

            if let Err(log_err) = inserter
                .log_processing(
                    &file.path.display().to_string(),
                    status,
                    &error_message,
                    processing_time,
                )
                .await
            {
                error!(
                    "Failed to log processing result for {}: {}",
                    file.relative_path, log_err
                );
            }

            (file, result, processing_time)
        }
    }))
    .buffer_unordered(parallel_workers)
    .collect::<Vec<_>>()
    .await;

    let mut total_processed = 0;

    for (file, result, processing_time) in results {
        match result {
            Ok(_) => {
                total_processed += 1;
                info!("Processed: {} ({} ms)", file.relative_path, processing_time);
            }
            Err(e) => {
                error!("Failed to process {}: {}", file.relative_path, e);
            }
        }
    }

    Ok(total_processed)
}

async fn process_single_file(
    inserter: &BatchInserter<'_>,
    markdown_parser: &MarkdownParser,
    normalizer: &MarkdownNormalizer,
    config: &Config,
    file: &crate::ScannedFile,
) -> Result<()> {
    Validator::validate_file_path(&file.path)?;

    let content = std::fs::read_to_string(&file.path).context("Failed to read file")?;

    Validator::validate_content_not_empty(&content)?;

    let normalized_content = if config.extraction.normalize_markdown {
        normalizer.normalize(&content)?
    } else {
        content
    };

    // Validate markdown structure
    markdown_parser.parse(&normalized_content)?;

    let chunks_inserted = inserter
        .insert_file(
            &file.path.display().to_string(),
            &file.relative_path,
            &normalized_content,
            file.modified,
            &config.repository.source_url,
            config.extraction.normalize_markdown,
        )
        .await?;

    info!(
        "Inserted {} chunk(s) for {}",
        chunks_inserted, file.relative_path
    );

    Ok(())
}

async fn cmd_verify(config: &Config, create_schema: bool) -> Result<()> {
    info!("Verifying database schema");

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    info!("Database connection successful");

    let schema_manager = SchemaManager::new(&client);

    if schema_manager.verify_schema().await? {
        info!("Schema verification passed - all tables exist");
    } else {
        warn!("Schema verification failed - some tables are missing");

        if create_schema {
            info!("Creating schema");
            schema_manager
                .initialize()
                .await
                .context("Failed to create schema")?;
            info!("Schema created successfully");
        } else {
            info!("Use --create-schema to create missing tables");
        }
    }

    Ok(())
}

async fn cmd_stats(config: &Config) -> Result<()> {
    info!("Gathering statistics");

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let doc_count = client.get_document_count().await?;
    info!("Total documents: {}", doc_count);

    Ok(())
}

async fn cmd_reset(config: &Config, confirm: bool) -> Result<()> {
    if !confirm {
        error!("This will delete all data. Use --confirm to proceed");
        return Ok(());
    }

    warn!("Resetting database - all data will be lost");

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    let schema_manager = SchemaManager::new(&client);
    schema_manager
        .drop_all_tables()
        .await
        .context("Failed to drop tables")?;

    info!("All tables dropped");

    schema_manager
        .initialize()
        .await
        .context("Failed to recreate schema")?;

    info!("Schema recreated - database reset complete");

    Ok(())
}

async fn cmd_mcp(config: &Config, transport: &str) -> Result<()> {
    info!("Starting MCP server (transport: {})", transport);

    if transport != "stdio" {
        error!("Only stdio transport is currently supported");
        return Err(anyhow::anyhow!("Unsupported transport: {}", transport));
    }

    let mcp_server = GitSummarizeMcp::new(config.clone());

    info!("MCP server ready. Available tools:");
    for tool in mcp_server.get_tool_router().list_all() {
        let description = tool.description.as_deref().unwrap_or("No description");
        info!("  - {}: {}", tool.name, description);
    }

    info!("Starting stdio transport...");
    let running = mcp_server
        .clone()
        .serve((stdin(), stdout()))
        .await
        .context("Failed to start MCP server over stdio")?;
    let quit_reason = running.waiting().await.context("MCP server task failed")?;
    info!("MCP server stopped: {:?}", quit_reason);

    Ok(())
}

async fn cmd_search(
    config: &Config,
    query: &str,
    limit: usize,
    repository_filter: Option<&str>,
) -> Result<()> {
    info!("Searching for: {}", query);

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let query_embedding = embed_query(config, query).await?;

    // Perform search
    let results = client
        .vector_search(query_embedding, limit, repository_filter)
        .await
        .context("Vector search failed")?;

    // Display results
    if results.is_empty() {
        println!("\nNo results found for query: \"{}\"\n", query);
        println!("Try:");
        println!("  - Using different search terms");
        println!("  - Removing repository filter");
        println!("  - Checking that documents have been ingested");
        return Ok(());
    }

    println!("\nSearch Results for: \"{}\"\n", query);
    println!("Found {} result(s)\n", results.len());
    println!("{}", "=".repeat(80));

    for (idx, result) in results.iter().enumerate() {
        println!("\n{}. {} (Score: {:.4})", idx + 1, result.location(), result.score);
        println!("   Repository: {}", result.repository_url);

        if let Some(distance) = result.distance {
            println!("   Distance: {:.4}", distance);
        }

        let preview: String = result.content.chars().take(300).collect();
        println!("   Preview:");
        for line in preview.lines().take(5) {
            println!("     {}", line);
        }
    }

    println!("\n{}", "=".repeat(80));
    info!("Search complete");

    Ok(())
}

/// Embed a query string using the configured embedding provider. Honors the
/// `allow_fallback` flag; otherwise a failed call surfaces as an error.
async fn embed_query(config: &Config, query: &str) -> Result<Vec<f32>> {
    let embedding = EmbeddingClient::new(config.embedding.clone());
    match embedding.generate_embedding(query).await {
        Ok(v) => Ok(v),
        Err(e) if config.embedding.allow_fallback => {
            warn!("Query embedding failed ({e}); using non-semantic fallback (degraded)");
            Ok(EmbeddingClient::generate_fallback_embedding(
                query,
                config.embedding.dimension,
            ))
        }
        Err(e) => Err(e).context("Failed to embed query"),
    }
}

async fn cmd_ask(
    config: &Config,
    question: &str,
    limit: usize,
    repository_filter: Option<&str>,
) -> Result<()> {
    info!("Answering: {}", question);

    let client = LanceDbClient::new(config.database.clone())
        .await
        .context("Failed to create LanceDB client")?;

    if !client.ping().await? {
        error!("Cannot connect to LanceDB");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let query_embedding = embed_query(config, question).await?;

    let results = client
        .vector_search(query_embedding, limit, repository_filter)
        .await
        .context("Vector search failed")?;

    if results.is_empty() {
        println!("\nNo relevant context found. Ingest a repository first, or broaden the question.\n");
        return Ok(());
    }

    let generator = AnswerGenerator::new(config.generation.clone());
    let answer = generator
        .answer(question, results)
        .await
        .context("Answer generation failed")?;

    println!("\n{}\n", answer.text);
    println!("{}", "-".repeat(80));
    println!("Sources:");
    for (idx, source) in answer.sources.iter().enumerate() {
        println!(
            "  [{}] {} ({}) — score {:.4}",
            idx + 1,
            source.location(),
            source.repository_url,
            source.score
        );
    }

    Ok(())
}
