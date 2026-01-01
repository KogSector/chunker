//! Semantic code chunker for normalized code input.
//!
//! This chunker receives pre-parsed/normalized code from code-normalize-fetch
//! and creates intelligent chunks based on the provided entity boundaries.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Entity boundary provided by code-normalize-fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityBoundary {
    /// Entity name
    pub name: String,
    /// Entity type (function, class, method, etc.)
    pub entity_type: String,
    /// Start line (1-indexed)
    pub start_line: usize,
    /// End line (1-indexed)
    pub end_line: usize,
    /// Optional signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Code chunker that uses pre-parsed entity boundaries.
///
/// This chunker receives normalized input from code-normalize-fetch
/// and creates semantic chunks based on entity boundaries.
pub struct CodeChunker {
    /// Languages supported for semantic chunking
    supported_languages: Vec<String>,
}

impl CodeChunker {
    /// Create a new code chunker.
    pub fn new() -> Self {
        Self {
            supported_languages: vec![
                "python", "javascript", "typescript", "rust", "go",
                "java", "c", "cpp", "ruby", "tsx", "jsx",
            ].into_iter().map(String::from).collect(),
        }
    }

    /// Chunk code with entity boundaries from code-normalize-fetch.
    pub fn chunk_with_entities(
        &self,
        item: &SourceItem,
        config: &ChunkConfig,
        entities: &[EntityBoundary],
    ) -> Result<Vec<Chunk>> {
        let content = &item.content;
        let lines: Vec<&str> = content.lines().collect();
        let chunk_size = config.chunk_size;
        let overlap = config.chunk_overlap;
        let language = item.extract_language().unwrap_or("unknown");

        if entities.is_empty() {
            // No entities provided, fall back to line-based chunking
            return self.fallback_chunk(item, config, language);
        }

        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        for entity in entities {
            let start_idx = entity.start_line.saturating_sub(1);
            let end_idx = entity.end_line.min(lines.len());

            if start_idx >= lines.len() || start_idx >= end_idx {
                continue;
            }

            let entity_text: String = lines[start_idx..end_idx].join("\n");
            let token_count = count_tokens(&entity_text);

            if token_count <= chunk_size {
                // Entity fits in one chunk
                let chunk = self.create_chunk(
                    &entity_text,
                    entity.start_line,
                    entity.end_line,
                    item,
                    chunk_index,
                    language,
                    Some(&entity.name),
                    Some(&entity.entity_type),
                );
                chunks.push(chunk);
                chunk_index += 1;
            } else {
                // Entity too large, split it
                let sub_chunks = self.split_large_entity(
                    &entity_text,
                    entity.start_line,
                    chunk_size,
                    overlap,
                    item,
                    &mut chunk_index,
                    language,
                    &entity.name,
                    &entity.entity_type,
                );
                chunks.extend(sub_chunks);
            }
        }

        // Handle any gaps between entities
        let covered_lines = self.get_covered_lines(entities, lines.len());
        let gap_chunks = self.chunk_gaps(&lines, &covered_lines, item, &mut chunk_index, config, language);
        chunks.extend(gap_chunks);

        // Sort chunks by start line
        chunks.sort_by_key(|c| c.metadata.line_range.map(|(s, _)| s).unwrap_or(0));

        Ok(chunks)
    }

    /// Create a chunk from text.
    fn create_chunk(
        &self,
        text: &str,
        start_line: usize,
        end_line: usize,
        item: &SourceItem,
        chunk_index: usize,
        language: &str,
        entity_name: Option<&str>,
        entity_type: Option<&str>,
    ) -> Chunk {
        let path = item.extract_path().unwrap_or("unknown");
        let token_count = count_tokens(text);
        
        let metadata = ChunkMetadata {
            content_type: entity_type.map(String::from),
            language: Some(language.to_string()),
            path: Some(path.to_string()),
            section: None,
            symbol_name: entity_name.map(String::from),
            parent_symbol: None,
            line_range: Some((start_line, end_line)),
            author: None,
            thread_id: None,
            timestamp: None,
            extra: None,
        };

        Chunk::new(
            item.id,
            item.source_id,
            item.source_kind,
            text.to_string(),
            token_count,
            0, // start_index not tracked at line level
            text.len(),
            chunk_index,
        ).with_metadata(metadata)
    }

    /// Split a large entity into multiple chunks.
    fn split_large_entity(
        &self,
        text: &str,
        base_start_line: usize,
        chunk_size: usize,
        overlap: usize,
        item: &SourceItem,
        chunk_index: &mut usize,
        language: &str,
        entity_name: &str,
        entity_type: &str,
    ) -> Vec<Chunk> {
        let lines: Vec<&str> = text.lines().collect();
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < lines.len() {
            // Find end point based on token count
            let mut end = start;
            let mut accumulated = String::new();

            while end < lines.len() && count_tokens(&accumulated) < chunk_size {
                accumulated.push_str(lines[end]);
                accumulated.push('\n');
                end += 1;
            }

            // Ensure we make progress
            if end == start {
                end = start + 1;
            }

            let chunk_text = lines[start..end].join("\n");
            let chunk_start_line = base_start_line + start;
            let chunk_end_line = base_start_line + end - 1;

            let chunk = self.create_chunk(
                &chunk_text,
                chunk_start_line,
                chunk_end_line,
                item,
                *chunk_index,
                language,
                Some(entity_name),
                Some(entity_type),
            );
            chunks.push(chunk);
            *chunk_index += 1;

            // Move start with overlap
            let overlap_lines = (overlap as f32 / 10.0).ceil() as usize;
            let next_start = end.saturating_sub(overlap_lines.min(end - start));
            start = if next_start <= start { end } else { next_start };
        }

        chunks
    }

    /// Get set of covered line indices.
    fn get_covered_lines(&self, entities: &[EntityBoundary], total_lines: usize) -> Vec<bool> {
        let mut covered = vec![false; total_lines];
        for entity in entities {
            let start = entity.start_line.saturating_sub(1);
            let end = entity.end_line.min(total_lines);
            for i in start..end {
                covered[i] = true;
            }
        }
        covered
    }

    /// Chunk gaps between entities.
    fn chunk_gaps(
        &self,
        lines: &[&str],
        covered: &[bool],
        item: &SourceItem,
        chunk_index: &mut usize,
        _config: &ChunkConfig,
        language: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut gap_start: Option<usize> = None;

        for (i, &is_covered) in covered.iter().enumerate() {
            if !is_covered {
                if gap_start.is_none() {
                    gap_start = Some(i);
                }
            } else if let Some(start) = gap_start {
                // End of gap
                let gap_text: String = lines[start..i].join("\n");
                if count_tokens(&gap_text) > 10 { // Only chunk meaningful gaps
                    let chunk = self.create_chunk(
                        &gap_text,
                        start + 1,
                        i,
                        item,
                        *chunk_index,
                        language,
                        None,
                        None,
                    );
                    chunks.push(chunk);
                    *chunk_index += 1;
                }
                gap_start = None;
            }
        }

        // Handle trailing gap
        if let Some(start) = gap_start {
            let gap_text: String = lines[start..].join("\n");
            if count_tokens(&gap_text) > 10 {
                let chunk = self.create_chunk(
                    &gap_text,
                    start + 1,
                    lines.len(),
                    item,
                    *chunk_index,
                    language,
                    None,
                    None,
                );
                chunks.push(chunk);
                *chunk_index += 1;
            }
        }

        chunks
    }

    /// Fallback: simple line-based chunking when no entities provided.
    fn fallback_chunk(&self, item: &SourceItem, config: &ChunkConfig, language: &str) -> Result<Vec<Chunk>> {
        let content = &item.content;
        let lines: Vec<&str> = content.lines().collect();
        let chunk_size = config.chunk_size;
        let overlap = config.chunk_overlap;

        let mut chunks = Vec::new();
        let mut chunk_index = 0;
        let mut start = 0;

        while start < lines.len() {
            let mut end = start;
            let mut accumulated = String::new();

            while end < lines.len() && count_tokens(&accumulated) < chunk_size {
                accumulated.push_str(lines[end]);
                accumulated.push('\n');
                end += 1;
            }

            if end == start {
                end = start + 1;
            }

            let chunk_text = lines[start..end].join("\n");
            let chunk = self.create_chunk(
                &chunk_text,
                start + 1,
                end,
                item,
                chunk_index,
                language,
                None,
                None,
            );
            chunks.push(chunk);
            chunk_index += 1;

            let overlap_lines = (overlap as f32 / 10.0).ceil() as usize;
            let next_start = end.saturating_sub(overlap_lines.min(end - start));
            start = if next_start <= start { end } else { next_start };
        }

        Ok(chunks)
    }
}

impl Default for CodeChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for CodeChunker {
    fn name(&self) -> &'static str {
        "code"
    }

    fn description(&self) -> &'static str {
        "Semantic code chunker that uses entity boundaries from code-normalize-fetch"
    }

    fn supports_language(&self, language: Option<&str>) -> bool {
        match language {
            Some(lang) => self.supported_languages.iter().any(|l| l == lang),
            None => true,
        }
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        // When called without entities, use fallback
        let language = item.extract_language().unwrap_or("unknown");
        self.fallback_chunk(item, config, language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    fn create_code_item(content: &str, language: &str) -> SourceItem {
        let metadata = serde_json::json!({
            "language": language,
            "path": "test.py"
        });
        
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::CodeRepo,
            content_type: format!("text/code:{}", language),
            content: content.to_string(),
            metadata,
            created_at: None,
        }
    }

    #[test]
    fn test_chunk_with_entities() {
        let chunker = CodeChunker::new();
        let config = ChunkConfig::default();
        
        let code = r#"import os

def hello():
    print("Hello")

def world():
    print("World")
"#;
        let item = create_code_item(code, "python");
        
        let entities = vec![
            EntityBoundary {
                name: "hello".to_string(),
                entity_type: "function".to_string(),
                start_line: 3,
                end_line: 4,
                signature: Some("def hello()".to_string()),
            },
            EntityBoundary {
                name: "world".to_string(),
                entity_type: "function".to_string(),
                start_line: 6,
                end_line: 7,
                signature: Some("def world()".to_string()),
            },
        ];

        let chunks = chunker.chunk_with_entities(&item, &config, &entities).unwrap();
        
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.content.contains("hello")));
        assert!(chunks.iter().any(|c| c.content.contains("world")));
    }

    #[test]
    fn test_fallback_chunking() {
        let chunker = CodeChunker::new();
        let config = ChunkConfig::default();
        
        let code = "line1\nline2\nline3\nline4\nline5";
        let item = create_code_item(code, "unknown");

        let chunks = chunker.chunk(&item, &config).unwrap();
        
        assert!(!chunks.is_empty());
    }
}
