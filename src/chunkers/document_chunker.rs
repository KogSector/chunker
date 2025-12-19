//! Document chunker for markdown and wiki content.

use anyhow::Result;
use regex::Regex;

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Document chunker for markdown, wiki, and structured text content.
///
/// This chunker is aware of document structure like headings, code blocks,
/// and lists, ensuring chunks respect these boundaries.
pub struct DocumentChunker {
    /// Regex for matching markdown headings
    heading_regex: Regex,
    /// Regex for matching code blocks (reserved for future use)
    #[allow(dead_code)]
    code_block_regex: Regex,
}

impl DocumentChunker {
    /// Create a new document chunker.
    pub fn new() -> Self {
        Self {
            heading_regex: Regex::new(r"(?m)^(#{1,6})\s+(.+)$").unwrap(),
            code_block_regex: Regex::new(r"(?s)```[\w]*\n.*?```").unwrap(),
        }
    }

    /// Split document into sections based on headings.
    fn split_by_headings(&self, content: &str) -> Vec<Section> {
        let mut sections = Vec::new();
        let mut current_section = Section::new(None, 0, 0);
        let mut in_code_block = false;
        let mut line_start = 0;

        for line in content.lines() {
            let line_end = line_start + line.len() + 1; // +1 for newline

            // Track code blocks to not split inside them
            if line.starts_with("```") {
                in_code_block = !in_code_block;
            }

            // Check for heading (not in code block)
            if !in_code_block {
                if let Some(caps) = self.heading_regex.captures(line) {
                    // Save current section if it has content
                    if !current_section.content.trim().is_empty() {
                        sections.push(current_section);
                    }

                    let level = caps.get(1).map(|m| m.as_str().len()).unwrap_or(1);
                    let title = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                    current_section = Section::new(
                        Some(title.to_string()),
                        level,
                        line_start,
                    );
                    current_section.content.push_str(line);
                    current_section.content.push('\n');
                    line_start = line_end;
                    continue;
                }
            }

            current_section.content.push_str(line);
            current_section.content.push('\n');
            line_start = line_end;
        }

        // Don't forget the last section
        if !current_section.content.trim().is_empty() {
            sections.push(current_section);
        }

        sections
    }

    /// Split a section into smaller chunks if it exceeds the token limit.
    fn split_section(&self, section: &Section, chunk_size: usize) -> Vec<(String, Option<String>)> {
        let tokens = count_tokens(&section.content);

        if tokens <= chunk_size {
            return vec![(section.content.clone(), section.heading.clone())];
        }

        // Split by paragraphs first
        let paragraphs = self.split_by_paragraphs(&section.content);
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_tokens = 0;

        // If there's a heading, include it in the first chunk
        let header_prefix = section.heading.as_ref().map(|h| {
            let hashes = "#".repeat(section.level);
            format!("{} {}\n\n", hashes, h)
        });

        for para in paragraphs {
            let para_tokens = count_tokens(&para);

            // Check if paragraph itself is too large
            if para_tokens > chunk_size {
                // Flush current chunk
                if !current_chunk.is_empty() {
                    chunks.push((current_chunk, section.heading.clone()));
                    current_chunk = String::new();
                    current_tokens = 0;
                }

                // Split paragraph by sentences
                let sentences = self.split_by_sentences(&para);
                for sentence in sentences {
                    let sent_tokens = count_tokens(&sentence);

                    if current_tokens + sent_tokens > chunk_size && !current_chunk.is_empty() {
                        chunks.push((current_chunk, section.heading.clone()));
                        current_chunk = String::new();
                        current_tokens = 0;
                    }

                    current_chunk.push_str(&sentence);
                    current_chunk.push(' ');
                    current_tokens += sent_tokens;
                }
            } else if current_tokens + para_tokens > chunk_size {
                // Current chunk is full
                chunks.push((current_chunk, section.heading.clone()));
                current_chunk = para;
                current_tokens = para_tokens;
            } else {
                if !current_chunk.is_empty() {
                    current_chunk.push_str("\n\n");
                }
                current_chunk.push_str(&para);
                current_tokens += para_tokens;
            }
        }

        // Last chunk
        if !current_chunk.is_empty() {
            chunks.push((current_chunk, section.heading.clone()));
        }

        // Prepend header to first chunk if we split
        if let (Some(prefix), Some((first, _))) = (header_prefix, chunks.first_mut()) {
            *first = format!("{}{}", prefix, first);
        }

        chunks
    }

    /// Split content by paragraph boundaries (double newlines).
    fn split_by_paragraphs(&self, content: &str) -> Vec<String> {
        content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Split content by sentence boundaries.
    fn split_by_sentences(&self, content: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for c in content.chars() {
            current.push(c);

            if c == '.' || c == '!' || c == '?' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current = String::new();
            }
        }

        if !current.trim().is_empty() {
            sentences.push(current.trim().to_string());
        }

        sentences
    }
}

/// A section of a document defined by a heading.
struct Section {
    heading: Option<String>,
    level: usize,
    #[allow(dead_code)]
    start_byte: usize,
    content: String,
}

impl Section {
    fn new(heading: Option<String>, level: usize, start_byte: usize) -> Self {
        Self {
            heading,
            level,
            start_byte,
            content: String::new(),
        }
    }
}

impl Default for DocumentChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for DocumentChunker {
    fn name(&self) -> &'static str {
        "document"
    }

    fn description(&self) -> &'static str {
        "Heading-aware document chunker for markdown and wiki content"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Split into sections by headings
        let sections = self.split_by_headings(content);

        // Split each section into chunks
        let mut chunks = Vec::new();
        let mut chunk_index = 0;
        let mut current_byte = 0;

        for section in sections {
            let section_chunks = self.split_section(&section, config.chunk_size);

            for (chunk_text, heading) in section_chunks {
                let token_count = count_tokens(&chunk_text);
                let start_index = current_byte;
                let end_index = start_index + chunk_text.len();

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    chunk_text,
                    token_count,
                    start_index,
                    end_index,
                    chunk_index,
                );

                // Add document metadata
                chunk.metadata = ChunkMetadata::for_document(
                    heading.as_deref(),
                    item.extract_path(),
                );

                chunks.push(chunk);
                chunk_index += 1;
                current_byte = end_index;
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

    fn create_doc_item(content: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::Document,
            content_type: "text/markdown".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_heading_splitting() {
        let chunker = DocumentChunker::new();
        let content = r#"
# Introduction

This is the introduction paragraph.

## Getting Started

This is the getting started section.

## Installation

This is the installation section.
"#;
        let item = create_doc_item(content);
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_small_document() {
        let chunker = DocumentChunker::new();
        let content = "Just a simple paragraph.";
        let item = create_doc_item(content);
        let config = ChunkConfig::with_size(100);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
    }
}
