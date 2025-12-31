//! AST Engine module for code parsing and entity extraction.
//!
//! This module provides:
//! - Tree-sitter based AST parsing for multiple languages
//! - Entity extraction (functions, classes, imports, etc.)
//! - Scope tree construction for context enrichment
//! - Semantic boundary detection for intelligent chunking

pub mod entity_extractor;
pub mod languages;
pub mod parser;
pub mod scope_tree;

pub use entity_extractor::{CodeEntity, EntityExtractor, EntityType, Import};
pub use parser::{AstBoundary, AstParser, NodeKind, ParsedFile};
pub use scope_tree::{ScopeNode, ScopeTree};
