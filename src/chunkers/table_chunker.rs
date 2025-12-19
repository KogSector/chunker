//! Table chunker for markdown and CSV tables.

use anyhow::Result;
use regex::Regex;

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Table chunker for markdown tables and CSV data.
///
/// This chunker understands table structure and preserves headers
/// when splitting large tables into smaller chunks.
pub struct TableChunker {
    /// Rows per chunk (when using row-based chunking)
    #[allow(dead_code)]
    rows_per_chunk: usize,
    /// Pattern for detecting table rows
    #[allow(dead_code)]
    row_pattern: Regex,
}

impl TableChunker {
    /// Create a new table chunker.
    pub fn new() -> Self {
        Self {
            rows_per_chunk: 10,
            row_pattern: Regex::new(r"^\|.*\|$").unwrap(),
        }
    }

    /// Create with custom rows per chunk.
    pub fn with_rows_per_chunk(rows_per_chunk: usize) -> Self {
        Self {
            rows_per_chunk,
            ..Self::new()
        }
    }

    /// Parse a markdown table into header and data rows.
    fn parse_markdown_table(&self, content: &str) -> Option<(String, String, Vec<String>)> {
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.len() < 3 {
            return None;
        }

        // Find header row (first row with |)
        let header_idx = lines.iter().position(|l| l.trim().starts_with('|'))?;
        let header = lines.get(header_idx)?.to_string();
        
        // Find separator row (row with |---|)
        let sep_idx = lines.iter().position(|l| {
            let trimmed = l.trim();
            trimmed.starts_with('|') && trimmed.contains('-')
        })?;
        let separator = lines.get(sep_idx)?.to_string();

        // Get data rows
        let data_rows: Vec<String> = lines
            .iter()
            .skip(sep_idx + 1)
            .filter(|l| l.trim().starts_with('|'))
            .map(|s| s.to_string())
            .collect();

        if data_rows.is_empty() {
            return None;
        }

        Some((header, separator, data_rows))
    }

    /// Parse CSV content.
    fn parse_csv(&self, content: &str) -> Option<(String, Vec<String>)> {
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.len() < 2 {
            return None;
        }

        let header = lines[0].to_string();
        let data_rows: Vec<String> = lines[1..].iter().map(|s| s.to_string()).collect();

        Some((header, data_rows))
    }

    /// Detect if content is markdown table or CSV.
    fn is_markdown_table(&self, content: &str) -> bool {
        let first_line = content.lines().next().unwrap_or("");
        first_line.trim().starts_with('|')
    }

    /// Chunk a markdown table.
    fn chunk_markdown_table(
        &self,
        header: &str,
        separator: &str,
        data_rows: Vec<String>,
        item: &SourceItem,
        config: &ChunkConfig,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;
        let mut current_index = 0;

        // Calculate header size
        let header_combined = format!("{}\n{}\n", header, separator);
        let header_tokens = count_tokens(&header_combined);

        // Determine rows per chunk based on token limit
        let max_tokens_for_rows = config.chunk_size.saturating_sub(header_tokens);
        
        let mut current_rows: Vec<&String> = Vec::new();
        let mut current_tokens = 0;

        for row in &data_rows {
            let row_tokens = count_tokens(row);

            // Check if adding this row exceeds limit
            if current_tokens + row_tokens > max_tokens_for_rows && !current_rows.is_empty() {
                // Create chunk
                let rows_text: String = current_rows.iter().map(|r| format!("{}\n", r)).collect();
                let chunk_content = format!("{}{}", header_combined, rows_text);
                let token_count = count_tokens(&chunk_content);

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    chunk_content.clone(),
                    token_count,
                    current_index,
                    current_index + chunk_content.len(),
                    chunk_index,
                );

                chunk.metadata = ChunkMetadata {
                    content_type: Some("table".to_string()),
                    ..Default::default()
                };

                chunks.push(chunk);
                chunk_index += 1;
                current_index += rows_text.len();

                current_rows = vec![row];
                current_tokens = row_tokens;
            } else {
                current_rows.push(row);
                current_tokens += row_tokens;
            }
        }

        // Last chunk
        if !current_rows.is_empty() {
            let rows_text: String = current_rows.iter().map(|r| format!("{}\n", r)).collect();
            let chunk_content = format!("{}{}", header_combined, rows_text);
            let token_count = count_tokens(&chunk_content);

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                chunk_content.clone(),
                token_count,
                current_index,
                current_index + chunk_content.len(),
                chunk_index,
            );

            chunk.metadata = ChunkMetadata {
                content_type: Some("table".to_string()),
                ..Default::default()
            };

            chunks.push(chunk);
        }

        chunks
    }

    /// Chunk CSV content.
    fn chunk_csv(
        &self,
        header: &str,
        data_rows: Vec<String>,
        item: &SourceItem,
        config: &ChunkConfig,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;
        let mut current_index = 0;

        let header_line = format!("{}\n", header);
        let header_tokens = count_tokens(&header_line);
        let max_tokens_for_rows = config.chunk_size.saturating_sub(header_tokens);

        let mut current_rows: Vec<&String> = Vec::new();
        let mut current_tokens = 0;

        for row in &data_rows {
            let row_tokens = count_tokens(row);

            if current_tokens + row_tokens > max_tokens_for_rows && !current_rows.is_empty() {
                let rows_text: String = current_rows.iter().map(|r| format!("{}\n", r)).collect();
                let chunk_content = format!("{}{}", header_line, rows_text);
                let token_count = count_tokens(&chunk_content);

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    chunk_content.clone(),
                    token_count,
                    current_index,
                    current_index + chunk_content.len(),
                    chunk_index,
                );

                chunk.metadata = ChunkMetadata {
                    content_type: Some("csv".to_string()),
                    ..Default::default()
                };

                chunks.push(chunk);
                chunk_index += 1;
                current_index += rows_text.len();

                current_rows = vec![row];
                current_tokens = row_tokens;
            } else {
                current_rows.push(row);
                current_tokens += row_tokens;
            }
        }

        // Last chunk
        if !current_rows.is_empty() {
            let rows_text: String = current_rows.iter().map(|r| format!("{}\n", r)).collect();
            let chunk_content = format!("{}{}", header_line, rows_text);
            let token_count = count_tokens(&chunk_content);

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                chunk_content.clone(),
                token_count,
                current_index,
                current_index + chunk_content.len(),
                chunk_index,
            );

            chunk.metadata = ChunkMetadata {
                content_type: Some("csv".to_string()),
                ..Default::default()
            };

            chunks.push(chunk);
        }

        chunks
    }
}

impl Default for TableChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for TableChunker {
    fn name(&self) -> &'static str {
        "table"
    }

    fn description(&self) -> &'static str {
        "Chunks tables (markdown/CSV) while preserving headers in each chunk"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Detect table type and parse
        if self.is_markdown_table(content) {
            if let Some((header, separator, data_rows)) = self.parse_markdown_table(content) {
                return Ok(self.chunk_markdown_table(&header, &separator, data_rows, item, config));
            }
        } else if let Some((header, data_rows)) = self.parse_csv(content) {
            return Ok(self.chunk_csv(&header, data_rows, item, config));
        }

        // Fallback: treat as single chunk
        let token_count = count_tokens(content);
        Ok(vec![Chunk::new(
            item.id,
            item.source_id,
            item.source_kind,
            content.clone(),
            token_count,
            0,
            content.len(),
            0,
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    fn create_table_item(content: &str) -> SourceItem {
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
    fn test_markdown_table() {
        let chunker = TableChunker::new();
        let content = r#"| Name | Age | City |
|------|-----|------|
| Alice | 30 | NYC |
| Bob | 25 | LA |
| Charlie | 35 | SF |
"#;
        let item = create_table_item(content);
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
        // Each chunk should contain the header
        assert!(chunks[0].content.contains("Name"));
    }

    #[test]
    fn test_csv() {
        let chunker = TableChunker::new();
        let content = "name,age,city\nalice,30,nyc\nbob,25,la\n";
        let item = create_table_item(content);
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }
}
