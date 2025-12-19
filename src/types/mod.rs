//! Core types for the chunking service.

mod chunk;
mod config;
mod source;

pub use chunk::{Chunk, ChunkMetadata};
pub use config::{ChunkConfig, ChunkingConfig, ChunkingPolicy, ChunkingProfile};
pub use source::{
    ChunkJobStatus, ChunkJobStatusResponse, SourceItem, SourceKind,
    StartChunkJobRequest, StartChunkJobResponse,
};
