//! Chunker Service Library
//!
//! A high-performance, production-ready chunking service for RAG pipelines.
//! Supports multiple content types including code, documents, chat, and tickets.

pub mod api;
pub mod batch;
pub mod chunkers;
pub mod jobs;
pub mod output;
pub mod router;
pub mod types;

pub use types::{Chunk, ChunkMetadata, SourceItem, SourceKind};
pub use chunkers::{Chunker, AgenticChunker};
pub use chunkers::repo_chunker::{RepositoryContext, Symbol, SymbolType, extract_symbols};
pub use router::ChunkingRouter;
pub use batch::{BatchProcessor, BatchConfig, BatchResult};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::types::*;
    pub use crate::chunkers::{Chunker, AgenticChunker};
    pub use crate::chunkers::repo_chunker::*;
    pub use crate::router::ChunkingRouter;
    pub use crate::batch::*;
}

/// Default chunk size in tokens
pub const DEFAULT_CHUNK_SIZE: usize = 512;

/// Default chunk overlap in tokens
pub const DEFAULT_CHUNK_OVERLAP: usize = 50;

/// Default minimum characters per sentence
pub const DEFAULT_MIN_CHARS_PER_SENTENCE: usize = 12;

/// Maximum content size for single-pass processing (10MB)
pub const DEFAULT_MAX_CONTENT_SIZE: usize = 10 * 1024 * 1024;

