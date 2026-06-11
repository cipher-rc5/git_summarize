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
    pub embedding: EmbeddingConfig,
    pub generation: GenerationConfig,
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
}

/// Embedding provider configuration.
///
/// Defaults target an OpenAI-compatible embeddings endpoint, but `base_url` and
/// `model` make it provider-agnostic (OpenAI, Voyage, Jina, a local TEI server, etc.).
/// `dimension` must match what the model returns; for OpenAI `text-embedding-3-*`
/// models it is sent as the `dimensions` request parameter (Matryoshka truncation).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    pub base_url: String,
    pub model: String,
    pub dimension: usize,
    #[serde(default)]
    pub api_key: Option<String>,
    /// When true, fall back to a deterministic non-semantic embedding if the API
    /// call fails. Off by default: a failed call should error rather than silently
    /// poison the index with vectors that share no space with real embeddings.
    #[serde(default)]
    pub allow_fallback: bool,
}

/// Answer-generation (LLM) configuration for the RAG question-answering step.
/// Defaults to Groq's OpenAI-compatible chat completions endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenerationConfig {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_max_context_chunks")]
    pub max_context_chunks: usize,
}

fn default_max_context_chunks() -> usize {
    8
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

        let mut config: Config = settings
            .try_deserialize()
            .map_err(|e| PipelineError::Config(e.to_string()))?;

        // Resolve API keys from the environment when not set in config, using the
        // conventional provider variable names.
        if config.embedding.api_key.is_none() {
            config.embedding.api_key = std::env::var("EMBEDDING_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .ok();
        }
        if config.generation.api_key.is_none() {
            config.generation.api_key = std::env::var("GENERATION_API_KEY")
                .or_else(|_| std::env::var("GROQ_API_KEY"))
                .ok();
        }

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
            },
            embedding: EmbeddingConfig {
                base_url: "https://api.openai.com/v1".to_string(),
                model: "text-embedding-3-small".to_string(),
                dimension: 768,
                api_key: None,
                allow_fallback: false,
            },
            generation: GenerationConfig {
                base_url: "https://api.groq.com/openai/v1".to_string(),
                model: "openai/gpt-oss-120b".to_string(),
                api_key: None,
                max_context_chunks: default_max_context_chunks(),
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

        if self.embedding.dimension == 0 {
            return Err(PipelineError::Config(
                "embedding.dimension must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}
