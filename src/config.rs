// file: src/config.rs
// description: Application configuration management with TOML support
// reference: https://docs.rs/config

use crate::error::{PipelineError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub repository: RepositoryConfig,
    pub database: DatabaseConfig,
    pub pipeline: PipelineConfig,
    pub extraction: ExtractionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepositoryConfig {
    pub source_url: String,
    pub local_path: PathBuf,
    pub branch: String,
    pub sync_on_start: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub batch_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PipelineConfig {
    pub parallel_workers: usize,
    pub skip_patterns: Vec<String>,
    pub force_reprocess: bool,
    pub max_file_size_mb: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractionConfig {
    pub extract_crypto_addresses: bool,
    pub extract_incidents: bool,
    pub extract_iocs: bool,
    pub normalize_markdown: bool,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("LAZARUS"))
            .build()
            .map_err(|e| PipelineError::Config(e.to_string()))?;

        let config: Config = settings
            .try_deserialize()
            .map_err(|e| PipelineError::Config(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    pub fn default_config() -> Self {
        Self {
            repository: RepositoryConfig {
                source_url: "https://github.com/tayvano/lazarus-bluenoroff-research".to_string(),
                local_path: PathBuf::from("./data_repo"),
                branch: "main".to_string(),
                sync_on_start: true,
            },
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 8123,
                database: "lazarus_research".to_string(),
                username: None,
                password: None,
                batch_size: 1000,
            },
            pipeline: PipelineConfig {
                parallel_workers: 4,
                skip_patterns: vec![
                    "*.zip".to_string(),
                    "*.pdf".to_string(),
                    ".git/*".to_string(),
                ],
                force_reprocess: false,
                max_file_size_mb: 10,
            },
            extraction: ExtractionConfig {
                extract_crypto_addresses: true,
                extract_incidents: true,
                extract_iocs: true,
                normalize_markdown: true,
            },
        }
    }

    fn validate(&self) -> Result<()> {
        if self.pipeline.parallel_workers == 0 {
            return Err(PipelineError::Config(
                "parallel_workers must be greater than 0".to_string(),
            ));
        }

        if self.database.batch_size == 0 {
            return Err(PipelineError::Config(
                "batch_size must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}
