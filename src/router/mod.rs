//! Chunking strategy router.

use std::sync::Arc;

use crate::chunkers::{
    AgenticChunker, ChatChunker, CodeChunker, Chunker, DocumentChunker, 
    RecursiveChunker, SentenceChunker, TableChunker, TicketingChunker, TokenChunker,
};
use crate::types::{ChunkConfig, ChunkingConfig, SourceItem, SourceKind};

/// Router that selects the appropriate chunker based on source type.
///
/// The router examines the source kind and content type to determine
/// which chunking strategy to use for optimal results.
pub struct ChunkingRouter {
    /// Token chunker (fallback)
    token_chunker: Arc<TokenChunker>,
    /// Sentence chunker (for plain text)
    sentence_chunker: Arc<SentenceChunker>,
    /// Recursive chunker (for structured text)
    recursive_chunker: Arc<RecursiveChunker>,
    /// Code chunker (for source code)
    code_chunker: Arc<CodeChunker>,
    /// Document chunker (for markdown/wiki)
    document_chunker: Arc<DocumentChunker>,
    /// Chat chunker (for messages)
    chat_chunker: Arc<ChatChunker>,
    /// Ticketing chunker (for issues/PRs)
    ticketing_chunker: Arc<TicketingChunker>,
    /// Table chunker (for markdown tables/CSV)
    table_chunker: Arc<TableChunker>,
    /// Agentic chunker (for intelligent boundary detection)
    agentic_chunker: Arc<AgenticChunker>,
    /// Default chunk configuration
    default_config: ChunkConfig,
}

impl ChunkingRouter {
    /// Create a new chunking router with the given configuration.
    pub fn new(config: &ChunkingConfig) -> Self {
        Self {
            token_chunker: Arc::new(TokenChunker::new()),
            sentence_chunker: Arc::new(SentenceChunker::new()),
            recursive_chunker: Arc::new(RecursiveChunker::new()),
            code_chunker: Arc::new(CodeChunker::new()),
            document_chunker: Arc::new(DocumentChunker::new()),
            chat_chunker: Arc::new(ChatChunker::new()),
            ticketing_chunker: Arc::new(TicketingChunker::new()),
            table_chunker: Arc::new(TableChunker::new()),
            agentic_chunker: Arc::new(AgenticChunker::new()),
            default_config: ChunkConfig {
                chunk_size: config.default_chunk_size,
                chunk_overlap: config.default_chunk_overlap,
                min_chars_per_sentence: config.min_chars_per_sentence,
                preserve_whitespace: false,
                language: None,
            },
        }
    }

    /// Get the appropriate chunker for the given source item.
    pub fn get_chunker(&self, item: &SourceItem) -> Arc<dyn Chunker> {
        // First, check content type for overrides
        if let Some(chunker) = self.match_content_type(&item.content_type) {
            return chunker;
        }

        // Then, match by source kind
        match item.source_kind {
            SourceKind::CodeRepo => Arc::clone(&self.code_chunker) as Arc<dyn Chunker>,
            SourceKind::Document => Arc::clone(&self.document_chunker) as Arc<dyn Chunker>,
            SourceKind::Wiki => Arc::clone(&self.document_chunker) as Arc<dyn Chunker>,
            SourceKind::Chat => Arc::clone(&self.chat_chunker) as Arc<dyn Chunker>,
            SourceKind::Email => Arc::clone(&self.chat_chunker) as Arc<dyn Chunker>,
            SourceKind::Ticketing => Arc::clone(&self.ticketing_chunker) as Arc<dyn Chunker>,
            SourceKind::Web => Arc::clone(&self.recursive_chunker) as Arc<dyn Chunker>,
            SourceKind::Other => Arc::clone(&self.sentence_chunker) as Arc<dyn Chunker>,
        }
    }

    /// Match chunker by content type.
    fn match_content_type(&self, content_type: &str) -> Option<Arc<dyn Chunker>> {
        if content_type.starts_with("text/code:") || content_type.contains("x-source") {
            return Some(Arc::clone(&self.code_chunker) as Arc<dyn Chunker>);
        }

        if content_type.contains("markdown") || content_type.contains("x-markdown") {
            return Some(Arc::clone(&self.document_chunker) as Arc<dyn Chunker>);
        }

        if content_type.contains("json") && content_type.contains("chat") {
            return Some(Arc::clone(&self.chat_chunker) as Arc<dyn Chunker>);
        }

        if content_type.contains("csv") || content_type.contains("table") {
            return Some(Arc::clone(&self.table_chunker) as Arc<dyn Chunker>);
        }

        None
    }

    /// Get the chunk configuration for a source item.
    pub fn get_config(&self, item: &SourceItem) -> ChunkConfig {
        let mut config = self.default_config.clone();

        // Set language for code items
        if item.source_kind == SourceKind::CodeRepo || item.content_type.starts_with("text/code:") {
            config.language = item.extract_language().map(String::from);
        }

        config
    }

    /// Get the default chunk configuration.
    pub fn default_config(&self) -> &ChunkConfig {
        &self.default_config
    }

    /// Get a chunker by name.
    pub fn get_chunker_by_name(&self, name: &str) -> Option<Arc<dyn Chunker>> {
        match name.to_lowercase().as_str() {
            "token" => Some(Arc::clone(&self.token_chunker) as Arc<dyn Chunker>),
            "sentence" => Some(Arc::clone(&self.sentence_chunker) as Arc<dyn Chunker>),
            "recursive" => Some(Arc::clone(&self.recursive_chunker) as Arc<dyn Chunker>),
            "code" => Some(Arc::clone(&self.code_chunker) as Arc<dyn Chunker>),
            "document" | "markdown" => Some(Arc::clone(&self.document_chunker) as Arc<dyn Chunker>),
            "chat" => Some(Arc::clone(&self.chat_chunker) as Arc<dyn Chunker>),
            "ticketing" | "ticket" | "issue" => Some(Arc::clone(&self.ticketing_chunker) as Arc<dyn Chunker>),
            "table" | "csv" => Some(Arc::clone(&self.table_chunker) as Arc<dyn Chunker>),
            "agentic" | "smart" | "intelligent" => Some(Arc::clone(&self.agentic_chunker) as Arc<dyn Chunker>),
            _ => None,
        }
    }

    /// List all available chunkers.
    pub fn list_chunkers(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (self.token_chunker.name(), self.token_chunker.description()),
            (self.sentence_chunker.name(), self.sentence_chunker.description()),
            (self.recursive_chunker.name(), self.recursive_chunker.description()),
            (self.code_chunker.name(), self.code_chunker.description()),
            (self.document_chunker.name(), self.document_chunker.description()),
            (self.chat_chunker.name(), self.chat_chunker.description()),
            (self.ticketing_chunker.name(), self.ticketing_chunker.description()),
            (self.table_chunker.name(), self.table_chunker.description()),
            (self.agentic_chunker.name(), self.agentic_chunker.description()),
        ]
    }
}

impl Default for ChunkingRouter {
    fn default() -> Self {
        Self::new(&ChunkingConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_item(source_kind: SourceKind, content_type: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind,
            content_type: content_type.to_string(),
            content: "test content".to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_code_routing() {
        let router = ChunkingRouter::default();
        let item = create_item(SourceKind::CodeRepo, "text/code:rust");
        let chunker = router.get_chunker(&item);
        assert_eq!(chunker.name(), "code");
    }

    #[test]
    fn test_document_routing() {
        let router = ChunkingRouter::default();
        let item = create_item(SourceKind::Document, "text/markdown");
        let chunker = router.get_chunker(&item);
        assert_eq!(chunker.name(), "document");
    }

    #[test]
    fn test_chat_routing() {
        let router = ChunkingRouter::default();
        let item = create_item(SourceKind::Chat, "application/json");
        let chunker = router.get_chunker(&item);
        assert_eq!(chunker.name(), "chat");
    }

    #[test]
    fn test_ticketing_routing() {
        let router = ChunkingRouter::default();
        let item = create_item(SourceKind::Ticketing, "text/plain");
        let chunker = router.get_chunker(&item);
        assert_eq!(chunker.name(), "ticketing");
    }
}
