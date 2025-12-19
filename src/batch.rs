//! Batch processing utilities for large-scale chunking.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::router::ChunkingRouter;
use crate::types::{Chunk, ChunkConfig, SourceItem, SourceKind};

/// Configuration for batch processing.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum items to process concurrently
    pub concurrency: usize,
    /// Maximum chunks to buffer before sending downstream
    pub buffer_size: usize,
    /// Whether to continue on individual item failures
    pub continue_on_error: bool,
    /// Maximum content size per item (bytes) before splitting
    pub max_content_size: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            buffer_size: 100,
            continue_on_error: true,
            max_content_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Result of batch processing.
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub total_items: usize,
    pub processed_items: usize,
    pub failed_items: usize,
    pub total_chunks: usize,
    pub errors: Vec<BatchError>,
}

/// Error during batch processing.
#[derive(Debug, Clone)]
pub struct BatchError {
    pub item_id: Uuid,
    pub error: String,
}

/// Batch processor for large-scale chunking operations.
pub struct BatchProcessor {
    router: Arc<ChunkingRouter>,
    config: BatchConfig,
}

impl BatchProcessor {
    /// Create a new batch processor.
    pub fn new(router: Arc<ChunkingRouter>, config: BatchConfig) -> Self {
        Self { router, config }
    }

    /// Process a batch of items and return all chunks.
    pub async fn process_batch(
        &self,
        items: Vec<SourceItem>,
        chunk_config: &ChunkConfig,
    ) -> Result<(Vec<Chunk>, BatchResult)> {
        let total_items = items.len();
        let mut all_chunks = Vec::new();
        let mut processed_items = 0;
        let mut failed_items = 0;
        let mut errors = Vec::new();

        info!(total_items, "Starting batch processing");

        for item in items {
            match self.process_single_item(&item, chunk_config).await {
                Ok(chunks) => {
                    all_chunks.extend(chunks);
                    processed_items += 1;
                }
                Err(e) => {
                    let error = BatchError {
                        item_id: item.id,
                        error: e.to_string(),
                    };
                    errors.push(error);
                    failed_items += 1;

                    if !self.config.continue_on_error {
                        return Err(e);
                    }

                    warn!(item_id = %item.id, error = %e, "Failed to process item");
                }
            }
        }

        let result = BatchResult {
            total_items,
            processed_items,
            failed_items,
            total_chunks: all_chunks.len(),
            errors,
        };

        info!(
            processed = processed_items,
            failed = failed_items,
            chunks = result.total_chunks,
            "Batch processing complete"
        );

        Ok((all_chunks, result))
    }

    /// Process a batch with streaming output.
    pub async fn process_batch_streaming(
        &self,
        items: Vec<SourceItem>,
        chunk_config: &ChunkConfig,
        sender: mpsc::Sender<Vec<Chunk>>,
    ) -> Result<BatchResult> {
        let total_items = items.len();
        let mut processed_items = 0;
        let mut failed_items = 0;
        let mut total_chunks = 0;
        let mut errors = Vec::new();
        let mut buffer = Vec::with_capacity(self.config.buffer_size);

        for item in items {
            match self.process_single_item(&item, chunk_config).await {
                Ok(chunks) => {
                    total_chunks += chunks.len();
                    buffer.extend(chunks);
                    processed_items += 1;

                    // Send when buffer is full
                    if buffer.len() >= self.config.buffer_size {
                        if sender.send(buffer.clone()).await.is_err() {
                            warn!("Receiver dropped, stopping batch processing");
                            break;
                        }
                        buffer.clear();
                    }
                }
                Err(e) => {
                    errors.push(BatchError {
                        item_id: item.id,
                        error: e.to_string(),
                    });
                    failed_items += 1;

                    if !self.config.continue_on_error {
                        return Err(e);
                    }
                }
            }
        }

        // Send remaining chunks
        if !buffer.is_empty() {
            let _ = sender.send(buffer).await;
        }

        Ok(BatchResult {
            total_items,
            processed_items,
            failed_items,
            total_chunks,
            errors,
        })
    }

    /// Process a single item, splitting large content if necessary.
    async fn process_single_item(
        &self,
        item: &SourceItem,
        config: &ChunkConfig,
    ) -> Result<Vec<Chunk>> {
        // Check if content is too large and needs pre-splitting
        if item.content.len() > self.config.max_content_size {
            debug!(
                item_id = %item.id,
                content_size = item.content.len(),
                "Content exceeds max size, pre-splitting"
            );
            return self.process_large_item(item, config);
        }

        let chunker = self.router.get_chunker(item);
        let item_config = self.router.get_config(item);

        // Merge configs
        let merged_config = ChunkConfig {
            chunk_size: config.chunk_size,
            chunk_overlap: config.chunk_overlap,
            min_chars_per_sentence: config.min_chars_per_sentence,
            preserve_whitespace: config.preserve_whitespace,
            language: item_config.language.or(config.language.clone()),
        };

        chunker.chunk(item, &merged_config)
    }

    /// Process a large item by splitting it first.
    fn process_large_item(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let chunker = self.router.get_chunker(item);
        let item_config = self.router.get_config(item);

        // For large items, we split content into manageable pieces first
        let content = &item.content;
        let piece_size = self.config.max_content_size;
        let mut all_chunks = Vec::new();
        let mut global_chunk_index = 0;

        // Split by natural boundaries (paragraphs, then by size)
        let pieces = split_large_content(content, piece_size);

        for (piece_idx, piece) in pieces.iter().enumerate() {
            // Create a sub-item for this piece
            let sub_item = SourceItem {
                id: item.id,
                source_id: item.source_id,
                source_kind: item.source_kind,
                content_type: item.content_type.clone(),
                content: piece.content.clone(),
                metadata: item.metadata.clone(),
                created_at: item.created_at,
            };

            let merged_config = ChunkConfig {
                chunk_size: config.chunk_size,
                chunk_overlap: config.chunk_overlap,
                min_chars_per_sentence: config.min_chars_per_sentence,
                preserve_whitespace: config.preserve_whitespace,
                language: item_config.language.clone().or(config.language.clone()),
            };

            match chunker.chunk(&sub_item, &merged_config) {
                Ok(mut chunks) => {
                    // Adjust indices to be relative to original content
                    for chunk in &mut chunks {
                        chunk.start_index += piece.start_offset;
                        chunk.end_index += piece.start_offset;
                        chunk.chunk_index = global_chunk_index;
                        global_chunk_index += 1;
                    }
                    all_chunks.extend(chunks);
                }
                Err(e) => {
                    warn!(
                        item_id = %item.id,
                        piece_idx,
                        error = %e,
                        "Failed to chunk piece, skipping"
                    );
                }
            }
        }

        Ok(all_chunks)
    }
}

/// A piece of content split from a larger document.
struct ContentPiece {
    content: String,
    start_offset: usize,
}

/// Split large content into manageable pieces.
fn split_large_content(content: &str, max_size: usize) -> Vec<ContentPiece> {
    let mut pieces = Vec::new();
    let mut current_start = 0;

    while current_start < content.len() {
        let remaining = content.len() - current_start;
        
        if remaining <= max_size {
            pieces.push(ContentPiece {
                content: content[current_start..].to_string(),
                start_offset: current_start,
            });
            break;
        }

        // Find a good split point (paragraph boundary)
        let search_end = (current_start + max_size).min(content.len());
        let search_range = &content[current_start..search_end];

        // Look for paragraph break
        let split_pos = if let Some(pos) = search_range.rfind("\n\n") {
            current_start + pos + 2
        } else if let Some(pos) = search_range.rfind("\n") {
            current_start + pos + 1
        } else {
            // No good break point, split at max size
            search_end
        };

        pieces.push(ContentPiece {
            content: content[current_start..split_pos].to_string(),
            start_offset: current_start,
        });

        current_start = split_pos;
    }

    pieces
}

/// Create `SourceItem` objects from repository file entries.
pub fn files_to_source_items(
    files: Vec<FileEntry>,
    source_id: Uuid,
) -> Vec<SourceItem> {
    files
        .into_iter()
        .map(|file| SourceItem {
            id: Uuid::new_v4(),
            source_id,
            source_kind: SourceKind::CodeRepo,
            content_type: format!("text/code:{}", file.language.as_deref().unwrap_or("text")),
            content: file.content,
            metadata: serde_json::json!({
                "path": file.path,
                "language": file.language,
            }),
            created_at: None,
        })
        .collect()
}

/// A file entry for batch processing.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub content: String,
    pub language: Option<String>,
}

/// Detect programming language from file extension.
pub fn detect_language(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    
    let lang = match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "jsx" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "c",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "cs" => "csharp",
        "md" | "markdown" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "sh" | "bash" => "bash",
        "ps1" => "powershell",
        _ => return None,
    };

    Some(lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_large_content() {
        let content = "Para 1.\n\nPara 2.\n\nPara 3.\n\nPara 4.";
        let pieces = split_large_content(content, 15);
        
        assert!(pieces.len() >= 2);
        // All pieces should be within size limit (roughly)
        for piece in &pieces {
            assert!(piece.content.len() <= 20); // Some flexibility
        }
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust".to_string()));
        assert_eq!(detect_language("app.py"), Some("python".to_string()));
        assert_eq!(detect_language("index.tsx"), Some("typescript".to_string()));
        assert_eq!(detect_language("unknown.xyz"), None);
    }
}
