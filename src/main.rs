// file: src/main.rs
// description: CLI application entry point with command handling
// reference: Application bootstrap and orchestration

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lazarus_ingest::{
    BatchInserter, ClickHouseClient, Config, CryptoExtractor, FileClassifier, FileScanner,
    IncidentExtractor, IocExtractor, MarkdownNormalizer, MarkdownParser, RepositorySync,
    SchemaManager, Validator,
};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "lazarus_ingest")]
#[command(author = "cipher")]
#[command(version = "0.1.0")]
#[command(about = "Ingestion pipeline for Lazarus/BlueNoroff threat research", long_about = None)]
struct Cli {
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "config/default.toml"
    )]
    config: PathBuf,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    setup_logging(cli.verbose);

    info!("Lazarus BlueNoroff Research Ingestion Pipeline");
    info!("Loading configuration from: {}", cli.config.display());

    let config = if cli.config.exists() {
        Config::from_file(cli.config.to_str().unwrap()).context("Failed to load configuration")?
    } else {
        warn!("Config file not found, using default configuration");
        Config::default_config()
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
    }

    Ok(())
}

fn setup_logging(verbosity: u8) {
    let level = match verbosity {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
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

    let client = ClickHouseClient::new(config.database.clone())
        .context("Failed to create ClickHouse client")?;

    if !client.ping().await? {
        error!("Cannot connect to ClickHouse");
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

async fn process_files(
    client: &ClickHouseClient,
    config: &Config,
    files: Vec<lazarus_ingest::ScannedFile>,
) -> Result<usize> {
    let inserter = BatchInserter::new(client);
    let classifier = FileClassifier::new();
    let markdown_parser = MarkdownParser::new();
    let normalizer = MarkdownNormalizer::new();

    let mut total_processed = 0;

    for file in files {
        let file_start = Instant::now();

        match process_single_file(
            &inserter,
            &classifier,
            &markdown_parser,
            &normalizer,
            config,
            &file,
        )
        .await
        {
            Ok(_) => {
                total_processed += 1;
                let processing_time = file_start.elapsed().as_millis() as u32;
                info!("Processed: {} ({} ms)", file.relative_path, processing_time);

                let _ = inserter
                    .log_processing(
                        &file.path.display().to_string(),
                        "success",
                        "",
                        processing_time,
                    )
                    .await;
            }
            Err(e) => {
                let processing_time = file_start.elapsed().as_millis() as u32;
                error!("Failed to process {}: {}", file.relative_path, e);

                let _ = inserter
                    .log_processing(
                        &file.path.display().to_string(),
                        "failed",
                        &e.to_string(),
                        processing_time,
                    )
                    .await;
            }
        }
    }

    Ok(total_processed)
}

async fn process_single_file(
    inserter: &BatchInserter<'_>,
    classifier: &FileClassifier,
    markdown_parser: &MarkdownParser,
    normalizer: &MarkdownNormalizer,
    config: &Config,
    file: &lazarus_ingest::ScannedFile,
) -> Result<()> {
    Validator::validate_file_path(&file.path)?;

    let content = std::fs::read_to_string(&file.path).context("Failed to read file")?;

    Validator::validate_content_not_empty(&content)?;

    let normalized_content = if config.extraction.normalize_markdown {
        normalizer.normalize(&content)?
    } else {
        content.clone()
    };

    let parsed = markdown_parser.parse(&normalized_content)?;

    let attribution = classifier.extract_attribution(&file.path);

    let document = lazarus_ingest::Document::new(
        file.path.display().to_string(),
        file.relative_path.clone(),
        normalized_content.clone(),
        file.modified,
    );

    let mut incidents = Vec::new();
    if config.extraction.extract_incidents {
        let incident_extractor = IncidentExtractor::new();
        incidents =
            incident_extractor.extract_from_markdown(&content, &file.path.display().to_string());
    }

    let mut addresses = Vec::new();
    if config.extraction.extract_crypto_addresses {
        let mut crypto_extractor = CryptoExtractor::new();
        addresses = crypto_extractor.extract_from_text(
            &parsed.plain_text,
            &file.path.display().to_string(),
            &attribution,
        );
    }

    let mut iocs = Vec::new();
    if config.extraction.extract_iocs {
        let mut ioc_extractor = IocExtractor::new();
        iocs = ioc_extractor.extract_from_text(&parsed.plain_text);
    }

    let stats = inserter
        .insert_complete_batch(document, incidents, addresses, iocs)
        .await?;

    info!(
        "Inserted: {} doc, {} incidents, {} addresses, {} IOCs",
        stats.documents_inserted,
        stats.incidents_inserted,
        stats.addresses_inserted,
        stats.iocs_inserted
    );

    Ok(())
}

async fn cmd_verify(config: &Config, create_schema: bool) -> Result<()> {
    info!("Verifying database schema");

    let client = ClickHouseClient::new(config.database.clone())
        .context("Failed to create ClickHouse client")?;

    if !client.ping().await? {
        error!("Cannot connect to ClickHouse");
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

    let client = ClickHouseClient::new(config.database.clone())
        .context("Failed to create ClickHouse client")?;

    if !client.ping().await? {
        error!("Cannot connect to ClickHouse");
        return Err(anyhow::anyhow!("Database connection failed"));
    }

    let doc_count = client.get_document_count().await?;
    info!("Total documents: {}", doc_count);

    let tables = vec!["incidents", "crypto_addresses", "iocs", "processing_log"];

    for table in tables {
        if client.table_exists(table).await? {
            let query = format!("SELECT count() FROM {}", table);
            let count: u64 = client
                .get_client()
                .query(&query)
                .fetch_one()
                .await
                .unwrap_or(0);
            info!("Total {}: {}", table, count);
        }
    }

    Ok(())
}

async fn cmd_reset(config: &Config, confirm: bool) -> Result<()> {
    if !confirm {
        error!("This will delete all data. Use --confirm to proceed");
        return Ok(());
    }

    warn!("Resetting database - all data will be lost");

    let client = ClickHouseClient::new(config.database.clone())
        .context("Failed to create ClickHouse client")?;

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
