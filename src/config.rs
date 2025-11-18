// file: src/config.rs
// description: application configuration management with toml support
// reference: https://docs.rs/config

use crate::error::{PipelineError, Result};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    pub uri: String,
    pub table_name: String,
    pub batch_size: usize,
    pub groq_api_key: Option<String>,
    pub groq_model: String,
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
    pub normalize_markdown: bool,
    #[serde(default)]
    pub categories: Vec<CategoryRule>,
    #[serde(default)]
    pub topics: Vec<TopicRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CategoryRule {
    pub keywords: Vec<String>,
    pub category: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicRule {
    pub keyword: String,
    pub topic: String,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        dotenv().ok();

        let mut builder = config::Config::builder();

        if let Some(path) = path {
            builder = builder.add_source(config::File::from(path));
        } else {
            builder = builder.add_source(config::File::from(Path::new("config/default.toml")));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("GIT_SUMMARIZE")
                .separator("__")
                .try_parsing(true),
        );

        let settings = builder
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
                source_url: "https://github.com/user/example-repo".to_string(),
                local_path: PathBuf::from("./data_repo"),
                branch: "main".to_string(),
                sync_on_start: true,
            },
            database: DatabaseConfig {
                uri: "data/lancedb".to_string(),
                table_name: "documents".to_string(),
                batch_size: 100,
                groq_api_key: None,
                groq_model: "openai/gpt-oss-120b".to_string(),
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
                normalize_markdown: true,
                categories: vec![],
                topics: vec![],
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
