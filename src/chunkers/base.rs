//! Base trait for all chunkers.

use anyhow::Result;

use crate::types::{Chunk, ChunkConfig, SourceItem};

/// The core trait that all chunkers must implement.
///
/// A chunker takes a source item and splits it into semantically meaningful
/// chunks that are suitable for embedding and retrieval.
pub trait Chunker: Send + Sync {
    /// Get the name of this chunker.
    fn name(&self) -> &'static str;

    /// Chunk the given content with the provided configuration.
    ///
    /// # Arguments
    /// * `item` - The source item to chunk
    /// * `config` - Configuration for chunking
    ///
    /// # Returns
    /// A vector of chunks extracted from the source item.
    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>>;

    /// Check if this chunker supports the given language.
    ///
    /// For code chunkers, this indicates language support.
    /// For text chunkers, this might indicate locale support.
    fn supports_language(&self, language: Option<&str>) -> bool {
        // By default, chunkers support all languages
        let _ = language;
        true
    }

    /// Get the description of this chunker.
    fn description(&self) -> &'static str {
        "A text chunker"
    }
}

/// Token counter trait for counting tokens in text.
pub trait TokenCounter: Send + Sync {
    /// Count the number of tokens in the given text.
    fn count_tokens(&self, text: &str) -> usize;

    /// Encode text into token IDs.
    fn encode(&self, text: &str) -> Vec<usize>;

    /// Decode token IDs back to text.
    fn decode(&self, tokens: &[usize]) -> String;
}

/// Default token counter using tiktoken (cl100k_base encoding).
pub struct TiktokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

impl TiktokenCounter {
    /// Create a new token counter with the cl100k_base encoding (GPT-4/ChatGPT).
    pub fn new() -> Self {
        // cl100k_base is used by GPT-4, ChatGPT, and text-embedding-ada-002
        let bpe = tiktoken_rs::cl100k_base().expect("Failed to load cl100k_base encoding");
        Self { bpe }
    }

    /// Create a token counter with a specific encoding.
    #[allow(dead_code)]
    pub fn with_encoding(encoding_name: &str) -> Result<Self> {
        let bpe = match encoding_name {
            "cl100k_base" => tiktoken_rs::cl100k_base()?,
            "p50k_base" => tiktoken_rs::p50k_base()?,
            "p50k_edit" => tiktoken_rs::p50k_edit()?,
            "r50k_base" => tiktoken_rs::r50k_base()?,
            _ => tiktoken_rs::cl100k_base()?,
        };
        Ok(Self { bpe })
    }
}

impl Default for TiktokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter for TiktokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        self.bpe.encode_ordinary(text).len()
    }

    fn encode(&self, text: &str) -> Vec<usize> {
        self.bpe.encode_ordinary(text)
    }

    fn decode(&self, tokens: &[usize]) -> String {
        self.bpe.decode(tokens.to_vec()).unwrap_or_default()
    }
}

/// Helper function to count tokens using the default counter.
pub fn count_tokens(text: &str) -> usize {
    lazy_static::lazy_static! {
        static ref COUNTER: TiktokenCounter = TiktokenCounter::new();
    }
    COUNTER.count_tokens(text)
}

/// Split text at sentence boundaries.
#[allow(dead_code)]
pub fn split_sentences(text: &str, delimiters: &[char]) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        current.push(c);

        if delimiters.contains(&c) {
            // Look ahead for space or end of string
            if chars.peek().map_or(true, |next| next.is_whitespace()) {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current = String::new();
                // Skip the whitespace
                if chars.peek().map_or(false, |next| next.is_whitespace()) {
                    chars.next();
                }
            }
        }
    }

    // Add remaining content
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

/// Merge short segments to meet minimum length requirements.
#[allow(dead_code)]
pub fn merge_short_segments(segments: Vec<String>, min_chars: usize) -> Vec<String> {
    if segments.is_empty() {
        return segments;
    }

    let mut result = Vec::new();
    let mut current = String::new();

    for segment in segments {
        if current.is_empty() {
            current = segment;
        } else {
            current.push(' ');
            current.push_str(&segment);
        }

        if current.len() >= min_chars {
            result.push(current);
            current = String::new();
        }
    }

    // Add any remaining content
    if !current.is_empty() {
        if let Some(last) = result.last_mut() {
            last.push(' ');
            last.push_str(&current);
        } else {
            result.push(current);
        }
    }

    result
}
