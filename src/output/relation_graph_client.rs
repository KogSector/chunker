//! HTTP client for sending chunks to the relation-graph service.
//!
//! This client enables the chunker to send processed chunks to the relation-graph
//! service in parallel with the embedding service. The relation-graph service
//! uses these chunks to:
//! - Extract entities (functions, classes, modules, concepts)
//! - Build the knowledge graph in Neo4j
//! - Create cross-source links between code and documentation

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::types::Chunk;

/// Client for sending chunks to the relation-graph service.
pub struct RelationGraphClient {
    client: Client,
    base_url: String,
    batch_size: usize,
    enabled: bool,
}

/// Request payload for ingesting chunks into the relation-graph.
#[derive(Debug, Serialize)]
struct IngestChunksRequest {
    chunks: Vec<ChunkForGraph>,
    /// Whether to extract entities from chunks
    extract_entities: bool,
    /// Whether to create cross-source links between code and docs
    create_cross_links: bool,
}

/// Chunk data sent to relation-graph service.
#[derive(Debug, Serialize)]
struct ChunkForGraph {
    id: String,
    content: String,
    source_kind: String,
    source_type: String,
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heading_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_id: Option<String>,
    metadata: serde_json::Value,
}

/// Response from relation-graph service.
#[derive(Debug, Deserialize)]
pub struct IngestChunksResponse {
    pub chunks_processed: usize,
    pub entities_created: usize,
    pub relationships_created: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl RelationGraphClient {
    /// Create a new relation-graph client.
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: base_url.to_string(),
            batch_size: 50,
            enabled: true,
        }
    }

    /// Create a disabled client (for when relation-graph service is not configured).
    pub fn disabled() -> Self {
        Self {
            client: Client::new(),
            base_url: String::new(),
            batch_size: 50,
            enabled: false,
        }
    }

    /// Set the batch size for sending chunks.
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Check if the client is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Send chunks to the relation-graph service for knowledge graph construction.
    pub async fn send_chunks(&self, chunks: &[Chunk]) -> Result<IngestChunksResponse> {
        if !self.enabled {
            debug!("Relation-graph client is disabled, skipping");
            return Ok(IngestChunksResponse {
                chunks_processed: 0,
                entities_created: 0,
                relationships_created: 0,
                errors: vec![],
            });
        }

        if chunks.is_empty() {
            return Ok(IngestChunksResponse {
                chunks_processed: 0,
                entities_created: 0,
                relationships_created: 0,
                errors: vec![],
            });
        }

        info!(chunk_count = chunks.len(), "Sending chunks to relation-graph service");

        let mut total_response = IngestChunksResponse {
            chunks_processed: 0,
            entities_created: 0,
            relationships_created: 0,
            errors: vec![],
        };

        // Send in batches
        for batch in chunks.chunks(self.batch_size) {
            match self.send_batch(batch).await {
                Ok(response) => {
                    total_response.chunks_processed += response.chunks_processed;
                    total_response.entities_created += response.entities_created;
                    total_response.relationships_created += response.relationships_created;
                    total_response.errors.extend(response.errors);
                    debug!(
                        batch_size = batch.len(),
                        entities = response.entities_created,
                        "Batch sent successfully to relation-graph"
                    );
                }
                Err(e) => {
                    error!(error = %e, "Failed to send batch to relation-graph service");
                    total_response.errors.push(e.to_string());
                    // Continue with other batches
                }
            }
        }

        info!(
            chunks_processed = total_response.chunks_processed,
            entities_created = total_response.entities_created,
            relationships_created = total_response.relationships_created,
            "Finished sending chunks to relation-graph service"
        );

        Ok(total_response)
    }

    /// Send a single batch of chunks.
    async fn send_batch(&self, chunks: &[Chunk]) -> Result<IngestChunksResponse> {
        let request = IngestChunksRequest {
            chunks: chunks.iter().map(|c| self.chunk_to_graph_format(c)).collect(),
            extract_entities: true,
            create_cross_links: true,
        };

        let url = format!("{}/api/graph/chunks", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let result: IngestChunksResponse = response.json().await?;
            if !result.errors.is_empty() {
                for error in &result.errors {
                    warn!(error, "Relation-graph service reported error");
                }
            }
            Ok(result)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Relation-graph service returned {}: {}",
                status,
                text
            ))
        }
    }

    /// Convert a Chunk to the format expected by relation-graph service.
    fn chunk_to_graph_format(&self, chunk: &Chunk) -> ChunkForGraph {
        let metadata = serde_json::to_value(&chunk.metadata).unwrap_or_default();
        
        // Extract fields from metadata if available
        let file_path = metadata.get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        let language = metadata.get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        let repo_name = metadata.get("repo")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let heading_path = metadata.get("heading_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let owner_id = metadata.get("owner_id")
            .or_else(|| metadata.get("tenant_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        ChunkForGraph {
            id: chunk.id.to_string(),
            content: chunk.content.clone(),
            source_kind: chunk.source_kind.to_string(),
            source_type: metadata.get("source_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            source_id: chunk.source_id.to_string(),
            file_path,
            repo_name,
            language,
            heading_path,
            owner_id,
            metadata,
        }
    }

    /// Check if the relation-graph service is healthy.
    pub async fn health_check(&self) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }

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

    #[test]
    fn test_client_creation() {
        let client = RelationGraphClient::new("http://localhost:3018");
        assert_eq!(client.batch_size, 50);
        assert!(client.is_enabled());
    }

    #[test]
    fn test_disabled_client() {
        let client = RelationGraphClient::disabled();
        assert!(!client.is_enabled());
    }

    #[test]
    fn test_batch_size_config() {
        let client = RelationGraphClient::new("http://localhost:3018").with_batch_size(100);
        assert_eq!(client.batch_size, 100);
    }
}
