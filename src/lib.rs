//! Chunker Service Library
//!
//! A high-performance, production-ready chunking service for RAG pipelines.
//! Supports multiple content types including code, documents, chat, and tickets.
//!
//! Includes:
//! - File processing with language detection
//! - AST parsing and entity extraction
//! - Context enrichment for embeddings

pub mod api;
pub mod ast_engine;
pub mod batch;
pub mod chunkers;
pub mod enrichment;
pub mod jobs;
pub mod output;
pub mod processing;
pub mod router;
pub mod types;

pub use types::{Chunk, ChunkMetadata, SourceItem, SourceKind};
pub use chunkers::{Chunker, AgenticChunker};
pub use chunkers::repo_chunker::{RepositoryContext, Symbol, SymbolType, extract_symbols};
pub use router::ChunkingRouter;
pub use batch::{BatchProcessor, BatchConfig, BatchResult};
pub use processing::{FileProcessor, Language, LanguageInfo, ProcessableFile};
pub use ast_engine::{AstParser, ParsedFile, CodeEntity, EntityType, ScopeTree};
pub use enrichment::{ContextBuilder, ChunkContext, EnrichedChunk};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::types::*;
    pub use crate::chunkers::{Chunker, AgenticChunker};
    pub use crate::chunkers::repo_chunker::*;
    pub use crate::router::ChunkingRouter;
    pub use crate::batch::*;
    pub use crate::processing::*;
    pub use crate::ast_engine::*;
    pub use crate::enrichment::*;
}

/// Default chunk size in tokens
pub const DEFAULT_CHUNK_SIZE: usize = 512;

/// Default chunk overlap in tokens
pub const DEFAULT_CHUNK_OVERLAP: usize = 50;

/// Default minimum characters per sentence
pub const DEFAULT_MIN_CHARS_PER_SENTENCE: usize = 12;

/// Maximum content size for single-pass processing (10MB)
pub const DEFAULT_MAX_CONTENT_SIZE: usize = 10 * 1024 * 1024;

