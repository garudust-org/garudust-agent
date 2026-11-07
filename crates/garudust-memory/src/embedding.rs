use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::vector::normalize_l2;

/// Common interface for text embedding models.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding vectors for a list of input texts.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// The name or identifier of the embedding model.
    fn model_name(&self) -> &str;

    /// Expected embedding dimension, if known.
    fn dimension(&self) -> Option<usize> {
        None
    }
}

/// A deterministic mock embedding provider for tests and offline usage.
/// Computes pseudo-embeddings using character-based hashing.
pub struct MockEmbeddingProvider {
    dimension: usize,
    model: String,
}

impl MockEmbeddingProvider {
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            model: "mock-embedding-v1".to_string(),
        }
    }
}

impl Default for MockEmbeddingProvider {
    fn default() -> Self {
        Self::new(8)
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let mut vec = vec![0.0_f32; self.dimension];
            for (i, b) in text.as_bytes().iter().enumerate() {
                let idx = (i + usize::from(*b)) % self.dimension;
                vec[idx] += f32::from(*b) / 255.0;
            }
            normalize_l2(&mut vec);
            results.push(vec);
        }
        Ok(results)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimension(&self) -> Option<usize> {
        Some(self.dimension)
    }
}

/// HTTP-based embedding provider compatible with OpenAI, Ollama, vLLM, and LocalAI.
pub struct HttpEmbeddingProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    dimension: Option<usize>,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    input: &'a [String],
    model: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingResponseItem {
    embedding: Vec<f32>,
    index: Option<usize>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingResponseItem>,
}

impl HttpEmbeddingProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
    ) -> Self {
        let mut base = base_url.into();
        if base.ends_with('/') {
            base.pop();
        }
        Self {
            client: reqwest::Client::new(),
            base_url: base,
            api_key,
            model: model.into(),
            dimension: None,
        }
    }

    #[must_use]
    pub fn with_dimension(mut self, dim: usize) -> Self {
        self.dimension = Some(dim);
        self
    }
}

#[async_trait]
impl EmbeddingProvider for HttpEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = if self.base_url.ends_with("/embeddings") {
            self.base_url.clone()
        } else {
            format!("{}/embeddings", self.base_url)
        };

        let mut req = self.client.post(&url);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let body = EmbeddingRequest {
            input: texts,
            model: &self.model,
        };

        let res = req
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to send embedding request to {url}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            bail!("embedding API returned HTTP {status}: {err_text}");
        }

        let response: EmbeddingResponse = res
            .json()
            .await
            .with_context(|| "failed to parse embedding API response JSON")?;

        let mut items = response.data;
        if items.len() != texts.len() {
            bail!(
                "embedding API returned {} vectors for {} inputs",
                items.len(),
                texts.len()
            );
        }

        // Sort items by index if provided by the API
        items.sort_by_key(|item| item.index.unwrap_or(0));

        let mut vectors: Vec<Vec<f32>> = items.into_iter().map(|item| item.embedding).collect();
        for vec in &mut vectors {
            normalize_l2(vec);
        }

        Ok(vectors)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimension(&self) -> Option<usize> {
        self.dimension
    }
}

pub type DynEmbeddingProvider = Arc<dyn EmbeddingProvider>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedding_provider() {
        let provider = MockEmbeddingProvider::new(16);
        let texts = vec![
            "Hello world".to_string(),
            "Rust memory and vectors".to_string(),
        ];
        let embeddings = provider.embed(&texts).await.expect("mock embed failed");

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 16);
        assert_eq!(embeddings[1].len(), 16);

        // Verify vectors are normalized
        let norm0: f32 = embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm0 - 1.0).abs() < 1e-4);
    }
}
