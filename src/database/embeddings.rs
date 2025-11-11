// file: src/database/embeddings.rs
// description: Groq API integration for text embeddings using GPT-OSS-120B
// reference: https://console.groq.com/docs/embeddings

use crate::error::{PipelineError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Serialize)]
struct GroqEmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct GroqEmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

pub struct GroqEmbeddingClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GroqEmbeddingClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Groq API endpoint
        let url = "https://api.groq.com/openai/v1/embeddings";

        let request = GroqEmbeddingRequest {
            input: vec![text.to_string()],
            model: self.model.clone(),
        };

        debug!(
            "Requesting embedding from Groq API for {} chars",
            text.len()
        );

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                PipelineError::Database(format!("Failed to send Groq API request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PipelineError::Database(format!(
                "Groq API request failed with status {}: {}",
                status, error_text
            )));
        }

        let embedding_response: GroqEmbeddingResponse = response.json().await.map_err(|e| {
            PipelineError::Database(format!("Failed to parse Groq API response: {}", e))
        })?;

        if let Some(embedding_data) = embedding_response.data.into_iter().next() {
            debug!(
                "Received embedding of dimension {}",
                embedding_data.embedding.len()
            );
            Ok(embedding_data.embedding)
        } else {
            Err(PipelineError::Database(
                "No embedding data returned from Groq API".to_string(),
            ))
        }
    }

    /// Generate a fallback embedding when API is unavailable
    pub fn generate_fallback_embedding(text: &str, dim: usize) -> Vec<f32> {
        warn!("Using fallback embedding generation");
        // Simple deterministic embedding based on text hash
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
        let embedding = GroqEmbeddingClient::generate_fallback_embedding("test text", 384);
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&x| x >= 0.0 && x <= 1.0));
    }

    #[test]
    fn test_fallback_embedding_deterministic() {
        let emb1 = GroqEmbeddingClient::generate_fallback_embedding("same text", 128);
        let emb2 = GroqEmbeddingClient::generate_fallback_embedding("same text", 128);
        assert_eq!(emb1, emb2);
    }
}
