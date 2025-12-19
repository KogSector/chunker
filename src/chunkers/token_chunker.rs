//! Token-based chunker for fixed-size token chunking.

use anyhow::Result;

use super::base::{Chunker, TiktokenCounter, TokenCounter};
use crate::types::{Chunk, ChunkConfig, SourceItem};

/// Simple token-based chunker that splits text into fixed-size token chunks.
///
/// This is the most basic chunker that doesn't consider semantic boundaries.
/// It's fast and predictable, useful as a fallback or for unstructured content.
pub struct TokenChunker {
    counter: TiktokenCounter,
}

impl TokenChunker {
    /// Create a new token chunker.
    pub fn new() -> Self {
        Self {
            counter: TiktokenCounter::new(),
        }
    }
}

impl Default for TokenChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for TokenChunker {
    fn name(&self) -> &'static str {
        "token"
    }

    fn description(&self) -> &'static str {
        "Splits text into fixed-size token chunks with optional overlap"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        let tokens = self.counter.encode(content);
        if tokens.is_empty() {
            return Ok(vec![]);
        }

        let mut chunks = Vec::new();
        let mut start_token = 0;
        let mut chunk_index = 0;

        let step = if config.chunk_overlap >= config.chunk_size {
            config.chunk_size
        } else {
            config.chunk_size - config.chunk_overlap
        };

        while start_token < tokens.len() {
            let end_token = (start_token + config.chunk_size).min(tokens.len());
            let chunk_tokens: Vec<usize> = tokens[start_token..end_token].to_vec();
            let chunk_text = self.counter.decode(&chunk_tokens);

            // Calculate character positions
            // This is approximate since token boundaries don't align perfectly with chars
            let start_char = if chunk_index == 0 {
                0
            } else {
                // Find the approximate start by decoding tokens before this chunk
                self.counter.decode(&tokens[..start_token].to_vec()).len()
            };
            let end_char = start_char + chunk_text.len();

            let chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                chunk_text,
                chunk_tokens.len(),
                start_char,
                end_char,
                chunk_index,
            );

            chunks.push(chunk);
            chunk_index += 1;

            start_token += step;

            // Stop if we've reached the end
            if end_token >= tokens.len() {
                break;
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    fn create_test_item(content: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::Document,
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_empty_content() {
        let chunker = TokenChunker::new();
        let item = create_test_item("");
        let config = ChunkConfig::default();
        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_small_content() {
        let chunker = TokenChunker::new();
        let item = create_test_item("Hello, world!");
        let config = ChunkConfig::with_size(100);
        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello, world!");
    }

    #[test]
    fn test_chunk_overlap() {
        let chunker = TokenChunker::new();
        // Create content that will span multiple chunks
        let content = "This is a test sentence. ".repeat(50);
        let item = create_test_item(&content);
        let config = ChunkConfig::with_size(50).with_overlap(10);
        let chunks = chunker.chunk(&item, &config).unwrap();
        
        assert!(chunks.len() > 1);
        // Each chunk except the last should have approximately chunk_size tokens
        for chunk in &chunks[..chunks.len() - 1] {
            assert!(chunk.token_count <= 50);
        }
    }
}
