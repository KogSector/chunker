//! Job processor for async chunk processing.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::store::JobStore;
use crate::output::EmbeddingClient;
use crate::router::ChunkingRouter;
use crate::types::{Chunk, SourceItem, StartChunkJobRequest};

/// Processor that handles chunking jobs asynchronously.
pub struct JobProcessor {
    router: Arc<ChunkingRouter>,
    embedding_client: Option<Arc<EmbeddingClient>>,
}

impl JobProcessor {
    /// Create a new job processor.
    pub fn new(router: Arc<ChunkingRouter>, embedding_client: Option<Arc<EmbeddingClient>>) -> Self {
        Self {
            router,
            embedding_client,
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

        // Send chunks to embedding service if configured
        if let Some(ref client) = self.embedding_client {
            if let Err(e) = client.send_chunks(&all_chunks).await {
                error!(job_id = %job_id, error = %e, "Failed to send chunks to embedding service");
            }
        }

        // Mark job as completed
        {
            let mut store = job_store.write().await;
            store.complete_job(job_id);
        }
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
