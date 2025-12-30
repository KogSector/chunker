//! Job processor for async chunk processing.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::store::JobStore;
use crate::output::{EmbeddingClient, RelationGraphClient};
use crate::router::ChunkingRouter;
use crate::types::{Chunk, SourceItem, StartChunkJobRequest};

/// Processor that handles chunking jobs asynchronously.
pub struct JobProcessor {
    router: Arc<ChunkingRouter>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    relation_graph_client: Option<Arc<RelationGraphClient>>,
}

impl JobProcessor {
    /// Create a new job processor.
    pub fn new(
        router: Arc<ChunkingRouter>,
        embedding_client: Option<Arc<EmbeddingClient>>,
        relation_graph_client: Option<Arc<RelationGraphClient>>,
    ) -> Self {
        Self {
            router,
            embedding_client,
            relation_graph_client,
        }
    }

    /// Process a chunking job.
    pub async fn process_job(
        &self,
        job_id: Uuid,
        request: StartChunkJobRequest,
        job_store: Arc<RwLock<JobStore>>,
    ) {
        info!(job_id = %job_id, items = request.items.len(), "Starting job processing");

        // Mark job as started
        {
            let mut store = job_store.write().await;
            store.start_job(job_id);
        }

        let mut total_chunks = 0;
        let mut processed = 0;
        let mut all_chunks = Vec::new();

        for item in &request.items {
            match self.process_item(item) {
                Ok(chunks) => {
                    total_chunks += chunks.len();
                    all_chunks.extend(chunks);
                }
                Err(e) => {
                    warn!(
                        job_id = %job_id,
                        item_id = %item.id,
                        error = %e,
                        "Failed to process item, continuing with others"
                    );
                }
            }

            processed += 1;

            // Update progress
            {
                let mut store = job_store.write().await;
                store.update_job_progress(job_id, processed, total_chunks);
            }
        }

        info!(
            job_id = %job_id,
            total_items = processed,
            total_chunks = total_chunks,
            "Job processing complete"
        );

        // Send chunks to downstream services in PARALLEL
        self.send_chunks_to_downstream_services(job_id, &all_chunks).await;

        // Mark job as completed
        {
            let mut store = job_store.write().await;
            store.complete_job(job_id);
        }
    }

    /// Send chunks to both embedding and relation-graph services in parallel.
    async fn send_chunks_to_downstream_services(&self, job_id: Uuid, chunks: &[Chunk]) {
        if chunks.is_empty() {
            return;
        }

        // Clone Arcs for async move
        let embedding_client = self.embedding_client.clone();
        let relation_graph_client = self.relation_graph_client.clone();
        
        // Create owned copies of chunks for each async task
        let chunks_for_embedding = chunks.to_vec();
        let chunks_for_graph = chunks.to_vec();

        // Send to both services in parallel using tokio::join!
        let (embedding_result, graph_result) = tokio::join!(
            async {
                if let Some(client) = embedding_client {
                    match client.send_chunks(&chunks_for_embedding).await {
                        Ok(count) => {
                            info!(
                                job_id = %job_id,
                                embedded_count = count,
                                "Successfully sent chunks to embedding service"
                            );
                            Ok(count)
                        }
                        Err(e) => {
                            error!(
                                job_id = %job_id,
                                error = %e,
                                "Failed to send chunks to embedding service"
                            );
                            Err(e)
                        }
                    }
                } else {
                    Ok(0)
                }
            },
            async {
                if let Some(client) = relation_graph_client {
                    if client.is_enabled() {
                        match client.send_chunks(&chunks_for_graph).await {
                            Ok(response) => {
                                info!(
                                    job_id = %job_id,
                                    chunks_processed = response.chunks_processed,
                                    entities_created = response.entities_created,
                                    relationships_created = response.relationships_created,
                                    "Successfully sent chunks to relation-graph service"
                                );
                                Ok(response)
                            }
                            Err(e) => {
                                error!(
                                    job_id = %job_id,
                                    error = %e,
                                    "Failed to send chunks to relation-graph service"
                                );
                                Err(e)
                            }
                        }
                    } else {
                        Ok(crate::output::IngestChunksResponse {
                            chunks_processed: 0,
                            entities_created: 0,
                            relationships_created: 0,
                            errors: vec![],
                        })
                    }
                } else {
                    Ok(crate::output::IngestChunksResponse {
                        chunks_processed: 0,
                        entities_created: 0,
                        relationships_created: 0,
                        errors: vec![],
                    })
                }
            }
        );

        // Log summary
        let embedded = embedding_result.unwrap_or(0);
        let graph_processed = graph_result.map(|r| r.chunks_processed).unwrap_or(0);
        
        info!(
            job_id = %job_id,
            chunks_total = chunks.len(),
            chunks_embedded = embedded,
            chunks_graphed = graph_processed,
            "Completed sending chunks to downstream services"
        );
    }

    /// Process a single source item.
    fn process_item(&self, item: &SourceItem) -> anyhow::Result<Vec<Chunk>> {
        let chunker = self.router.get_chunker(item);
        let config = self.router.get_config(item);

        info!(
            item_id = %item.id,
            chunker = chunker.name(),
            content_len = item.content.len(),
            "Processing item"
        );

        chunker.chunk(item, &config)
    }

    /// Process a single item synchronously (for testing/simple use).
    pub fn process_item_sync(&self, item: &SourceItem) -> anyhow::Result<Vec<Chunk>> {
        self.process_item(item)
    }
}
