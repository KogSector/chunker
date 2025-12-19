//! Recursive text chunker with hierarchical splitting.

use anyhow::Result;

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, SourceItem};

/// Recursive chunker that splits text hierarchically.
///
/// This chunker tries multiple split strategies in order of preference:
/// 1. Double newlines (paragraphs)
/// 2. Single newlines
/// 3. Sentence endings (. ! ?)
/// 4. Commas
/// 5. Spaces (words)
/// 6. Characters (last resort)
///
/// For each level, it only proceeds to more granular splitting if
/// the current chunks are still too large.
pub struct RecursiveChunker {
    /// Separators in order of preference (most to least preferred)
    separators: Vec<&'static str>,
}

impl RecursiveChunker {
    /// Create a new recursive chunker with default separators.
    pub fn new() -> Self {
        Self {
            separators: vec![
                "\n\n",  // Paragraphs
                "\n",    // Lines
                ". ",    // Sentences
                "! ",    // Exclamations
                "? ",    // Questions
                "; ",    // Semicolons
                ", ",    // Commas
                " ",     // Words
            ],
        }
    }

    /// Create a recursive chunker for markdown content.
    pub fn for_markdown() -> Self {
        Self {
            separators: vec![
                "\n\n\n",  // Section breaks
                "\n\n",    // Paragraphs
                "\n# ",    // Headers
                "\n## ",   // Subheaders
                "\n### ",  // Sub-subheaders
                "\n",      // Lines
                ". ",      // Sentences
                " ",       // Words
            ],
        }
    }

    /// Create a recursive chunker with custom separators.
    pub fn with_separators(separators: Vec<&'static str>) -> Self {
        Self { separators }
    }

    /// Split text using the given separator.
    fn split_by_separator<'a>(&self, text: &'a str, separator: &str) -> Vec<&'a str> {
        if separator.is_empty() {
            // Character-level splitting
            text.chars().map(|c| {
                let start = text.char_indices()
                    .find(|(_, ch)| *ch == c)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let end = start + c.len_utf8();
                &text[start..end]
            }).collect()
        } else {
            text.split(separator).collect()
        }
    }

    /// Recursively chunk text using the separator hierarchy.
    fn recursive_chunk(
        &self,
        text: &str,
        chunk_size: usize,
        separator_index: usize,
    ) -> Vec<String> {
        if text.is_empty() {
            return vec![];
        }

        // If text fits in a single chunk, return it
        let token_count = count_tokens(text);
        if token_count <= chunk_size {
            return vec![text.to_string()];
        }

        // If we've exhausted all separators, split by characters
        if separator_index >= self.separators.len() {
            return self.split_by_chars(text, chunk_size);
        }

        let separator = self.separators[separator_index];
        let splits: Vec<&str> = self.split_by_separator(text, separator);

        // If we only got one split, try the next separator
        if splits.len() <= 1 {
            return self.recursive_chunk(text, chunk_size, separator_index + 1);
        }

        // Merge splits into chunks
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for (_i, split) in splits.iter().enumerate() {
            let test_chunk = if current_chunk.is_empty() {
                split.to_string()
            } else {
                format!("{}{}{}", current_chunk, separator, split)
            };

            let test_tokens = count_tokens(&test_chunk);

            if test_tokens <= chunk_size {
                current_chunk = test_chunk;
            } else {
                // Current chunk is full
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                }

                // Check if this split itself is too large
                let split_tokens = count_tokens(split);
                if split_tokens > chunk_size {
                    // Recursively split this piece with finer separators
                    let sub_chunks = self.recursive_chunk(split, chunk_size, separator_index + 1);
                    chunks.extend(sub_chunks);
                    current_chunk = String::new();
                } else {
                    current_chunk = split.to_string();
                }
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Split text by characters (last resort).
    fn split_by_chars(&self, text: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();

        for c in text.chars() {
            current.push(c);

            if count_tokens(&current) >= chunk_size {
                chunks.push(current);
                current = String::new();
            }
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}

impl Default for RecursiveChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for RecursiveChunker {
    fn name(&self) -> &'static str {
        "recursive"
    }

    fn description(&self) -> &'static str {
        "Hierarchically splits text using multiple separator levels"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Recursively split the content
        let text_chunks = self.recursive_chunk(content, config.chunk_size, 0);

        // Convert to Chunk objects
        let mut chunks = Vec::new();
        let mut current_index = 0;

        for (chunk_index, text) in text_chunks.iter().enumerate() {
            let token_count = count_tokens(text);
            let start_index = current_index;
            let end_index = start_index + text.len();

            chunks.push(Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                text.clone(),
                token_count,
                start_index,
                end_index,
                chunk_index,
            ));

            current_index = end_index;
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
    fn test_small_text() {
        let chunker = RecursiveChunker::new();
        let item = create_test_item("Hello, world!");
        let config = ChunkConfig::with_size(100);
        
        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello, world!");
    }

    #[test]
    fn test_paragraph_splitting() {
        let chunker = RecursiveChunker::new();
        let content = "This is paragraph one.\n\nThis is paragraph two.\n\nThis is paragraph three.";
        let item = create_test_item(content);
        let config = ChunkConfig::with_size(20);
        
        let chunks = chunker.chunk(&item, &config).unwrap();
        // Should produce at least one chunk
        assert!(!chunks.is_empty());
        // Total content should be preserved
        let total_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total_content.contains("paragraph one"));
        assert!(total_content.contains("paragraph two"));
    }

    #[test]
    fn test_sentence_splitting() {
        let chunker = RecursiveChunker::new();
        let content = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let item = create_test_item(content);
        let config = ChunkConfig::with_size(15);
        
        let chunks = chunker.chunk(&item, &config).unwrap();
        // Should produce at least one chunk
        assert!(!chunks.is_empty());
        // Content should be preserved
        let total_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total_content.contains("First"));
        assert!(total_content.contains("Fourth"));
    }
}
