// file: src/generation/mod.rs
// description: RAG answer synthesis over retrieved chunks via an OpenAI-compatible chat API
// reference: https://console.groq.com/docs/api-reference#chat-create

use crate::config::GenerationConfig;
use crate::error::{PipelineError, Result};
use crate::models::SearchResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

const SYSTEM_PROMPT: &str = "You are a precise documentation assistant for software repositories. \
Answer the user's question using ONLY the provided context excerpts. \
Cite the sources you use inline with bracketed numbers like [1], [2] that refer to the numbered \
context excerpts. If the context does not contain the answer, say so plainly instead of guessing. \
Be concise and technical.";

/// A synthesized answer plus the sources that were supplied as context.
#[derive(Debug, Clone)]
pub struct Answer {
    pub text: String,
    pub sources: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

pub struct AnswerGenerator {
    client: Client,
    config: GenerationConfig,
}

impl AnswerGenerator {
    pub fn new(config: GenerationConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn max_context_chunks(&self) -> usize {
        self.config.max_context_chunks
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    /// Synthesize an answer to `query` grounded in `results`. The top
    /// `max_context_chunks` results are passed as numbered context.
    pub async fn answer(&self, query: &str, results: Vec<SearchResult>) -> Result<Answer> {
        let api_key = self.config.api_key.as_ref().ok_or_else(|| {
            PipelineError::Config(
                "No generation API key configured (set generation.api_key, GENERATION_API_KEY, or GROQ_API_KEY)".to_string(),
            )
        })?;

        if results.is_empty() {
            return Ok(Answer {
                text: "No relevant context was found in the index for this question.".to_string(),
                sources: results,
            });
        }

        let sources: Vec<SearchResult> = results
            .into_iter()
            .take(self.config.max_context_chunks)
            .collect();

        let context = build_context(&sources);
        let user_content = format!(
            "Context excerpts:\n\n{context}\n\n---\n\nQuestion: {query}\n\n\
             Answer using only the excerpts above and cite them with [n]."
        );

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            temperature: 0.2,
        };

        debug!(
            "Requesting answer from {} (model {}, {} sources)",
            self.endpoint(),
            self.config.model,
            sources.len()
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
                PipelineError::Database(format!("Failed to send generation request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PipelineError::Database(format!(
                "Generation request failed with status {}: {}",
                status, error_text
            )));
        }

        let parsed: ChatResponse = response.json().await.map_err(|e| {
            PipelineError::Database(format!("Failed to parse generation response: {}", e))
        })?;

        let text = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| {
                PipelineError::Database("Generation API returned no choices".to_string())
            })?;

        Ok(Answer { text, sources })
    }
}

/// Build the numbered context block fed to the model. Numbers here line up with
/// the `[n]` citations the model is asked to produce and the printed source list.
fn build_context(sources: &[SearchResult]) -> String {
    sources
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {} ({})\n{}", i + 1, r.location(), r.repository_url, r.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        SearchResultFileMetadata, SearchResultPaths, SearchResultScoring,
    };

    fn result(idx: usize, content: &str) -> SearchResult {
        SearchResult::new(
            format!("id{idx}"),
            SearchResultPaths {
                file_path: format!("/repo/file{idx}.md"),
                relative_path: format!("file{idx}.md"),
                heading_path: "Section".to_string(),
            },
            content.to_string(),
            "https://github.com/x/y".to_string(),
            SearchResultScoring {
                score: 0.9,
                distance: Some(0.1),
            },
            SearchResultFileMetadata {
                file_size: 10,
                last_modified: 0,
            },
        )
    }

    #[test]
    fn context_is_numbered_and_located() {
        let ctx = build_context(&[result(1, "alpha"), result(2, "beta")]);
        assert!(ctx.contains("[1] file1.md # Section"));
        assert!(ctx.contains("[2] file2.md # Section"));
        assert!(ctx.contains("alpha"));
        assert!(ctx.contains("beta"));
    }
}
