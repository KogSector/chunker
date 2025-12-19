//! Sentence-based chunker that respects sentence boundaries.

use anyhow::Result;

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, SourceItem};

/// Sentence-based chunker that splits text at sentence boundaries.
///
/// This chunker identifies sentence endings and groups sentences into
/// chunks that respect the token limit while maintaining readability.
pub struct SentenceChunker {
    /// Sentence-ending delimiters
    delimiters: Vec<char>,
}

impl SentenceChunker {
    /// Create a new sentence chunker with default delimiters.
    pub fn new() -> Self {
        Self {
            delimiters: vec!['.', '!', '?'],
        }
    }

    /// Create a sentence chunker with custom delimiters.
    pub fn with_delimiters(delimiters: Vec<char>) -> Self {
        Self { delimiters }
    }

    /// Split text into sentences.
    fn split_sentences(&self, text: &str) -> Vec<Sentence> {
        let mut sentences = Vec::new();
        let mut current_start = 0;
        let mut current_text = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            current_text.push(c);

            if self.delimiters.contains(&c) {
                // Check if followed by whitespace or end of string
                let is_sentence_end = i + 1 >= chars.len() || chars[i + 1].is_whitespace();

                if is_sentence_end {
                    // Include trailing whitespace in the sentence
                    let mut j = i + 1;
                    while j < chars.len() && chars[j].is_whitespace() && chars[j] != '\n' {
                        current_text.push(chars[j]);
                        j += 1;
                    }

                    let trimmed = current_text.trim();
                    if !trimmed.is_empty() {
                        sentences.push(Sentence {
                            text: current_text.clone(),
                            start_index: current_start,
                            end_index: current_start + current_text.len(),
                            token_count: count_tokens(&current_text),
                        });
                    }

                    current_start += current_text.len();
                    current_text = String::new();
                    i = j;
                    continue;
                }
            }

            i += 1;
        }

        // Add remaining text as final sentence
        if !current_text.trim().is_empty() {
            sentences.push(Sentence {
                text: current_text.clone(),
                start_index: current_start,
                end_index: current_start + current_text.len(),
                token_count: count_tokens(&current_text),
            });
        }

        sentences
    }

    /// Merge short sentences to meet minimum character requirement.
    fn merge_short_sentences(&self, sentences: Vec<Sentence>, min_chars: usize) -> Vec<Sentence> {
        if sentences.is_empty() {
            return sentences;
        }

        let mut result = Vec::new();
        let mut current: Option<Sentence> = None;

        for sentence in sentences {
            current = match current {
                None => Some(sentence),
                Some(mut curr) => {
                    if curr.text.len() < min_chars {
                        // Merge with current
                        curr.text.push_str(&sentence.text);
                        curr.end_index = sentence.end_index;
                        curr.token_count = count_tokens(&curr.text);
                        Some(curr)
                    } else {
                        result.push(curr);
                        Some(sentence)
                    }
                }
            };
        }

        if let Some(curr) = current {
            result.push(curr);
        }

        result
    }
}

impl Default for SentenceChunker {
    fn default() -> Self {
        Self::new()
    }
}

/// Intermediate sentence representation.
struct Sentence {
    text: String,
    start_index: usize,
    end_index: usize,
    token_count: usize,
}

impl Chunker for SentenceChunker {
    fn name(&self) -> &'static str {
        "sentence"
    }

    fn description(&self) -> &'static str {
        "Splits text at sentence boundaries while respecting token limits"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Split into sentences
        let sentences = self.split_sentences(content);

        // Merge short sentences
        let sentences = self.merge_short_sentences(sentences, config.min_chars_per_sentence);

        if sentences.is_empty() {
            return Ok(vec![]);
        }

        // Group sentences into chunks
        let mut chunks = Vec::new();
        let mut current_sentences: Vec<&Sentence> = Vec::new();
        let mut current_tokens = 0;
        let mut chunk_start = 0;
        let mut chunk_index = 0;

        for sentence in &sentences {
            // Check if adding this sentence exceeds the limit
            if current_tokens + sentence.token_count > config.chunk_size && !current_sentences.is_empty() {
                // Create chunk from current sentences
                let chunk_text: String = current_sentences.iter().map(|s| s.text.as_str()).collect();
                let chunk_end = current_sentences.last().map(|s| s.end_index).unwrap_or(chunk_start);

                chunks.push(Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    chunk_text,
                    current_tokens,
                    chunk_start,
                    chunk_end,
                    chunk_index,
                ));

                chunk_index += 1;
                chunk_start = sentence.start_index;
                current_sentences = vec![sentence];
                current_tokens = sentence.token_count;
            } else {
                current_sentences.push(sentence);
                current_tokens += sentence.token_count;
            }
        }

        // Add final chunk
        if !current_sentences.is_empty() {
            let chunk_text: String = current_sentences.iter().map(|s| s.text.as_str()).collect();
            let chunk_end = current_sentences.last().map(|s| s.end_index).unwrap_or(chunk_start);

            chunks.push(Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                chunk_text,
                current_tokens,
                chunk_start,
                chunk_end,
                chunk_index,
            ));
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
    fn test_sentence_splitting() {
        let chunker = SentenceChunker::new();
        let content = "This is the first sentence. This is the second sentence! Is this the third?";
        let item = create_test_item(content);
        let config = ChunkConfig::with_size(1000);
        
        let chunks = chunker.chunk(&item, &config).unwrap();
        // With large chunk size, all sentences should be in one chunk
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("first sentence"));
        assert!(chunks[0].content.contains("second sentence"));
    }

    #[test]
    fn test_multiple_chunks() {
        let chunker = SentenceChunker::new();
        let content = "Sentence one. ".repeat(20) + &"Sentence two. ".repeat(20);
        let item = create_test_item(&content);
        let config = ChunkConfig::with_size(50);
        
        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(chunks.len() > 1);
    }
}
