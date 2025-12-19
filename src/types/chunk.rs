//! Chunk type definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SourceKind;

/// A chunk of content extracted from a source item.
///
/// Chunks are the fundamental unit of content that gets embedded and indexed.
/// Each chunk maintains references back to its source for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Unique identifier for this chunk
    pub id: Uuid,
    
    /// ID of the source item this chunk was extracted from
    pub source_item_id: Uuid,
    
    /// ID of the source (connected account/integration)
    pub source_id: Uuid,
    
    /// Kind of source this chunk came from
    pub source_kind: SourceKind,
    
    /// The actual text content of the chunk
    pub content: String,
    
    /// Number of tokens in this chunk
    pub token_count: usize,
    
    /// Starting character index in the original source item content
    pub start_index: usize,
    
    /// Ending character index in the original source item content
    pub end_index: usize,
    
    /// Order of this chunk within its source item (0-indexed)
    pub chunk_index: usize,
    
    /// Additional metadata about this chunk
    pub metadata: ChunkMetadata,
    
    /// Embedding vector (populated by embedding service)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    
    /// When this chunk was created
    pub created_at: DateTime<Utc>,
}

impl Chunk {
    /// Create a new chunk with the given parameters.
    pub fn new(
        source_item_id: Uuid,
        source_id: Uuid,
        source_kind: SourceKind,
        content: String,
        token_count: usize,
        start_index: usize,
        end_index: usize,
        chunk_index: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_item_id,
            source_id,
            source_kind,
            content,
            token_count,
            start_index,
            end_index,
            chunk_index,
            metadata: ChunkMetadata::default(),
            embedding: None,
            created_at: Utc::now(),
        }
    }

    /// Create a chunk with metadata.
    pub fn with_metadata(mut self, metadata: ChunkMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Get the length of the chunk content in characters.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if the chunk is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Metadata associated with a chunk.
///
/// Contains contextual information that helps understand the chunk's
/// origin and structure within its source.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Type of content (e.g., "function", "class", "paragraph", "message")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    
    /// Language of the content (for code: "rust", "python"; for text: "en", "es")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    
    /// File path or document path (for code and documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    
    /// Section or heading this chunk belongs to (for documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    
    /// Function or class name (for code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    
    /// Parent symbol (e.g., class name for a method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_symbol: Option<String>,
    
    /// Line numbers in original file (start, end)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_range: Option<(usize, usize)>,
    
    /// Author or speaker (for chat/comments)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    
    /// Thread ID (for chat/comments)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    
    /// Timestamp (for chat messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    
    /// Additional arbitrary metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl ChunkMetadata {
    /// Create metadata for a code chunk.
    pub fn for_code(language: &str, path: Option<&str>) -> Self {
        Self {
            language: Some(language.to_string()),
            path: path.map(String::from),
            ..Default::default()
        }
    }

    /// Create metadata for a document chunk.
    pub fn for_document(section: Option<&str>, path: Option<&str>) -> Self {
        Self {
            section: section.map(String::from),
            path: path.map(String::from),
            ..Default::default()
        }
    }

    /// Create metadata for a chat message chunk.
    pub fn for_chat(author: Option<&str>, thread_id: Option<&str>, timestamp: Option<DateTime<Utc>>) -> Self {
        Self {
            author: author.map(String::from),
            thread_id: thread_id.map(String::from),
            timestamp,
            ..Default::default()
        }
    }

    /// Set the symbol name (for code).
    pub fn with_symbol(mut self, name: &str, parent: Option<&str>) -> Self {
        self.symbol_name = Some(name.to_string());
        self.parent_symbol = parent.map(String::from);
        self
    }

    /// Set line range (for code).
    pub fn with_lines(mut self, start: usize, end: usize) -> Self {
        self.line_range = Some((start, end));
        self
    }
}
