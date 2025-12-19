//! HTTP client for sending chunks to the embedding service.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::types::Chunk;

/// Client for sending chunks to the embedding service.
pub struct EmbeddingClient {
    client: Client,
    base_url: String,
    batch_size: usize,
}

/// Request payload for embedding chunks.
#[derive(Debug, Serialize)]
struct EmbedChunksRequest {
    chunks: Vec<ChunkForEmbedding>,
}

/// Chunk data sent to embedding service.
#[derive(Debug, Serialize)]
struct ChunkForEmbedding {
    id: String,
    source_item_id: String,
    source_id: String,
    content: String,
    metadata: serde_json::Value,
}

/// Response from embedding service.
#[derive(Debug, Deserialize)]
struct EmbedChunksResponse {
    embedded_count: usize,
    #[serde(default)]
    errors: Vec<String>,
}

impl EmbeddingClient {
    /// Create a new embedding client.
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: base_url.to_string(),
            batch_size: 50,
        }
    }

    /// Set the batch size for sending chunks.
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Send chunks to the embedding service.
    pub async fn send_chunks(&self, chunks: &[Chunk]) -> Result<usize> {
        if chunks.is_empty() {
            return Ok(0);
        }

        info!(chunk_count = chunks.len(), "Sending chunks to embedding service");

        let mut total_embedded = 0;

        // Send in batches
        for batch in chunks.chunks(self.batch_size) {
            match self.send_batch(batch).await {
                Ok(count) => {
                    total_embedded += count;
                    debug!(batch_size = batch.len(), embedded = count, "Batch sent successfully");
                }
                Err(e) => {
                    error!(error = %e, "Failed to send batch to embedding service");
                    // Continue with other batches
                }
            }
        }

        info!(total_embedded, "Finished sending chunks to embedding service");
        Ok(total_embedded)
    }

    /// Send a single batch of chunks.
    async fn send_batch(&self, chunks: &[Chunk]) -> Result<usize> {
        let request = EmbedChunksRequest {
            chunks: chunks
                .iter()
                .map(|c| ChunkForEmbedding {
                    id: c.id.to_string(),
                    source_item_id: c.source_item_id.to_string(),
                    source_id: c.source_id.to_string(),
                    content: c.content.clone(),
                    metadata: serde_json::to_value(&c.metadata).unwrap_or_default(),
                })
                .collect(),
        };

        let url = format!("{}/embed/chunks", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let result: EmbedChunksResponse = response.json().await?;
            if !result.errors.is_empty() {
                for error in &result.errors {
                    error!(error, "Embedding service reported error");
                }
            }
            Ok(result.embedded_count)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Embedding service returned {}: {}",
                status,
                text
            ))
        }
    }

    /// Check if the embedding service is healthy.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    #[test]
    fn test_client_creation() {
        let client = EmbeddingClient::new("http://localhost:3018");
        assert_eq!(client.batch_size, 50);
    }

    #[test]
    fn test_batch_size_config() {
        let client = EmbeddingClient::new("http://localhost:3018").with_batch_size(100);
        assert_eq!(client.batch_size, 100);
    }
}
