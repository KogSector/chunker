//! Context enrichment module for embedding-optimized chunks.
//!
//! This module provides:
//! - Context prefix generation for code chunks
//! - Scope and dependency processing
//! - Rich metadata for improved embedding quality

pub mod context_builder;
pub mod dependency_parser;

pub use context_builder::{ChunkContext, ContextBuilder, EnrichedChunk};
pub use dependency_parser::{Dependency, DependencyParser, DependencyType};
