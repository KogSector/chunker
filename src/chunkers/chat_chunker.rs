//! Chat chunker for conversation windows.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Chat chunker for conversation-based content like Slack, Discord, or Teams.
///
/// This chunker groups messages into conversation windows that maintain
/// context while respecting token limits.
pub struct ChatChunker {
    /// Maximum messages per chunk (0 = no limit)
    max_messages_per_chunk: usize,
    /// Include speaker names in output
    include_speakers: bool,
}

impl ChatChunker {
    /// Create a new chat chunker with default settings.
    pub fn new() -> Self {
        Self {
            max_messages_per_chunk: 0, // No message limit, use token limit
            include_speakers: true,
        }
    }

    /// Set the maximum messages per chunk.
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages_per_chunk = max;
        self
    }

    /// Parse chat content from JSON format.
    fn parse_chat_json(&self, content: &str) -> Option<ChatThread> {
        serde_json::from_str(content).ok()
    }

    /// Parse chat content from plain text format.
    /// Expected format:
    /// ```text
    /// [timestamp] speaker: message
    /// ```
    fn parse_chat_text(&self, content: &str) -> ChatThread {
        let mut messages = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse "[timestamp] speaker: message" format
            if let Some((meta, text)) = line.split_once(": ") {
                let (timestamp, speaker) = if meta.starts_with('[') {
                    if let Some(end) = meta.find(']') {
                        let ts = &meta[1..end];
                        let spk = meta[end + 1..].trim();
                        (Some(ts.to_string()), spk.to_string())
                    } else {
                        (None, meta.to_string())
                    }
                } else {
                    (None, meta.to_string())
                };

                messages.push(ChatMessage {
                    user: speaker,
                    text: text.to_string(),
                    ts: timestamp,
                });
            } else {
                // Treat as continuation of previous message or standalone
                messages.push(ChatMessage {
                    user: "unknown".to_string(),
                    text: line.to_string(),
                    ts: None,
                });
            }
        }

        ChatThread {
            channel: None,
            thread_ts: None,
            messages,
        }
    }

    /// Format a message for inclusion in a chunk.
    fn format_message(&self, msg: &ChatMessage) -> String {
        if self.include_speakers {
            format!("{}: {}", msg.user, msg.text)
        } else {
            msg.text.clone()
        }
    }
}

/// Represents a chat thread with messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatThread {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<String>,
    messages: Vec<ChatMessage>,
}

/// Represents a single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    user: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ts: Option<String>,
}

impl Default for ChatChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for ChatChunker {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn description(&self) -> &'static str {
        "Conversation window chunker for chat and messaging content"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Parse the chat content
        let thread = if item.content_type.contains("json") {
            self.parse_chat_json(content).unwrap_or_else(|| self.parse_chat_text(content))
        } else {
            self.parse_chat_text(content)
        };

        if thread.messages.is_empty() {
            return Ok(vec![]);
        }

        // Group messages into chunks
        let mut chunks = Vec::new();
        let mut current_messages: Vec<&ChatMessage> = Vec::new();
        let mut current_text = String::new();
        let mut current_tokens = 0;
        let mut chunk_index = 0;

        for msg in &thread.messages {
            let msg_text = self.format_message(msg);
            let msg_tokens = count_tokens(&msg_text);

            // Check if we should start a new chunk
            let should_split = 
                (current_tokens + msg_tokens > config.chunk_size && !current_messages.is_empty())
                || (self.max_messages_per_chunk > 0 
                    && current_messages.len() >= self.max_messages_per_chunk);

            if should_split {
                // Create chunk from current messages
                let token_count = count_tokens(&current_text);

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    current_text.clone(),
                    token_count,
                    0,
                    current_text.len(),
                    chunk_index,
                );

                // Add chat metadata
                let first_ts = current_messages.first()
                    .and_then(|m| m.ts.as_ref())
                    .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());

                chunk.metadata = ChunkMetadata::for_chat(
                    current_messages.first().map(|m| m.user.as_str()),
                    thread.thread_ts.as_deref(),
                    first_ts,
                );

                chunks.push(chunk);
                chunk_index += 1;

                current_messages.clear();
                current_text.clear();
                current_tokens = 0;
            }

            // Add message to current chunk
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(&msg_text);
            current_messages.push(msg);
            current_tokens += msg_tokens;
        }

        // Don't forget the last chunk
        if !current_messages.is_empty() {
            let token_count = count_tokens(&current_text);

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                current_text.clone(),
                token_count,
                0,
                current_text.len(),
                chunk_index,
            );

            let first_ts = current_messages.first()
                .and_then(|m| m.ts.as_ref())
                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());

            chunk.metadata = ChunkMetadata::for_chat(
                current_messages.first().map(|m| m.user.as_str()),
                thread.thread_ts.as_deref(),
                first_ts,
            );

            chunks.push(chunk);
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    fn create_chat_item(content: &str, content_type: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::Chat,
            content_type: content_type.to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_json_chat_parsing() {
        let chunker = ChatChunker::new();
        let content = r#"{"channel":"general","messages":[{"user":"alice","text":"Hello everyone!"},{"user":"bob","text":"Hi Alice!"},{"user":"charlie","text":"Hey there!"}]}"#;

        let item = create_chat_item(content, "application/json");
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("alice"));
        assert!(chunks[0].content.contains("bob"));
    }

    #[test]
    fn test_text_chat_parsing() {
        let chunker = ChatChunker::new();
        let content = r#"alice: Hello everyone!
bob: Hi Alice!
charlie: Hey there!"#;

        let item = create_chat_item(content, "text/plain");
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chat_splitting() {
        let chunker = ChatChunker::new();
        let content = (0..50)
            .map(|i| format!("user{}: Message number {} with some additional text here.", i % 5, i))
            .collect::<Vec<_>>()
            .join("\n");

        let item = create_chat_item(&content, "text/plain");
        let config = ChunkConfig::with_size(100);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(chunks.len() > 1);
    }
}
