// file: src/database/embeddings.rs
// description: text embedding client for an OpenAI-compatible embeddings endpoint
// reference: https://platform.openai.com/docs/api-reference/embeddings

use crate::config::EmbeddingConfig;
use crate::error::{PipelineError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
    // OpenAI text-embedding-3-* honor this for Matryoshka truncation. Providers
    // that ignore it must already return `dimension`-sized vectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

/// Client for an OpenAI-compatible `/embeddings` endpoint.
///
/// The provider is selected entirely via [`EmbeddingConfig`] (`base_url`, `model`,
/// `dimension`), so OpenAI / Voyage / Jina / a local TEI server all work without
/// code changes.
pub struct EmbeddingClient {
    client: Client,
    config: EmbeddingConfig,
}

impl EmbeddingClient {
    pub fn new(config: EmbeddingConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn dimension(&self) -> usize {
        self.config.dimension
    }

    fn endpoint(&self) -> String {
        format!("{}/embeddings", self.config.base_url.trim_end_matches('/'))
    }

    /// Embed a single string. Convenience wrapper over [`Self::embed_batch`].
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let mut out = self.embed_batch(&[text.to_string()]).await?;
        out.pop().ok_or_else(|| {
            PipelineError::Database("Embedding API returned no vectors".to_string())
        })
    }

    /// Embed many strings in a single request, returning vectors in input order.
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.config.api_key.as_ref().ok_or_else(|| {
            PipelineError::Config(
                "No embedding API key configured (set embedding.api_key, EMBEDDING_API_KEY, or OPENAI_API_KEY)".to_string(),
            )
        })?;

        let request = EmbeddingRequest {
            input: texts.to_vec(),
            model: self.config.model.clone(),
            dimensions: Some(self.config.dimension),
        };

        debug!(
            "Requesting {} embedding(s) from {} (model {})",
            texts.len(),
            self.endpoint(),
            self.config.model
        );

        let response = self
            .client
            .post(self.endpoint())
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                PipelineError::Database(format!("Failed to send embedding request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PipelineError::Database(format!(
                "Embedding request failed with status {}: {}",
                status, error_text
            )));
        }

        let mut parsed: EmbeddingResponse = response.json().await.map_err(|e| {
            PipelineError::Database(format!("Failed to parse embedding response: {}", e))
        })?;

        // Return in input order regardless of how the provider ordered `data`.
        parsed.data.sort_by_key(|d| d.index);

        if parsed.data.len() != texts.len() {
            return Err(PipelineError::Database(format!(
                "Embedding API returned {} vectors for {} inputs",
                parsed.data.len(),
                texts.len()
            )));
        }

        let vectors: Vec<Vec<f32>> = parsed.data.into_iter().map(|d| d.embedding).collect();

        for v in &vectors {
            if v.len() != self.config.dimension {
                return Err(PipelineError::Database(format!(
                    "Embedding API returned dimension {} but config expects {}. \
                     Set embedding.dimension to match the model.",
                    v.len(),
                    self.config.dimension
                )));
            }
        }

        Ok(vectors)
    }

    /// Deterministic, NON-SEMANTIC fallback. Only suitable for offline tests or
    /// when `embedding.allow_fallback` is explicitly enabled. It carries no
    /// meaning, so search results built on it are effectively random. Never mix
    /// fallback vectors with real ones in the same index.
    pub fn generate_fallback_embedding(text: &str, dim: usize) -> Vec<f32> {
        warn!("Using NON-SEMANTIC fallback embedding — search quality will be degraded");
        let hash = text.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        (0..dim)
            .map(|i| (hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_embedding() {
        let embedding = EmbeddingClient::generate_fallback_embedding("test text", 384);
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&x| (0.0..=1.0).contains(&x)));
    }

    #[test]
    fn test_fallback_embedding_deterministic() {
        let emb1 = EmbeddingClient::generate_fallback_embedding("same text", 128);
        let emb2 = EmbeddingClient::generate_fallback_embedding("same text", 128);
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_endpoint_trims_trailing_slash() {
        let client = EmbeddingClient::new(EmbeddingConfig {
            base_url: "https://api.openai.com/v1/".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimension: 768,
            api_key: Some("k".to_string()),
            allow_fallback: false,
        });
        assert_eq!(client.endpoint(), "https://api.openai.com/v1/embeddings");
    }
}
