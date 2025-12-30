//! Output module for sending chunks to downstream services.

mod embedding_client;
mod relation_graph_client;

pub use embedding_client::EmbeddingClient;
pub use relation_graph_client::{RelationGraphClient, IngestChunksResponse};
