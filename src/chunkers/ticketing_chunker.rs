//! Ticketing chunker for issues, PRs, and tickets.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Ticketing chunker for issues, PRs, Jira tickets, and similar content.
///
/// This chunker understands the structure of tickets/issues:
/// - Title and description
/// - Comments and discussions
/// - Status and metadata
pub struct TicketingChunker {
    /// Whether to include metadata header
    include_metadata: bool,
    /// Whether to separate comments
    separate_comments: bool,
}

impl TicketingChunker {
    /// Create a new ticketing chunker.
    pub fn new() -> Self {
        Self {
            include_metadata: true,
            separate_comments: true,
        }
    }

    /// Parse ticket from JSON format.
    fn parse_ticket_json(&self, content: &str) -> Option<Ticket> {
        serde_json::from_str(content).ok()
    }

    /// Parse ticket from structured text format.
    fn parse_ticket_text(&self, content: &str) -> Ticket {
        let mut ticket = Ticket::default();
        let mut current_section = "description";
        let mut section_content = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Check for section headers
            if trimmed.starts_with("Title:") || trimmed.starts_with("Summary:") {
                ticket.title = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
            } else if trimmed.starts_with("Description:") {
                if !section_content.is_empty() {
                    self.save_section(&mut ticket, current_section, &section_content);
                }
                current_section = "description";
                section_content = trimmed.strip_prefix("Description:").unwrap_or("").trim().to_string();
            } else if trimmed.starts_with("Comments:") || trimmed.starts_with("Discussion:") {
                if !section_content.is_empty() {
                    self.save_section(&mut ticket, current_section, &section_content);
                }
                current_section = "comments";
                section_content = String::new();
            } else if trimmed.starts_with("Status:") {
                ticket.status = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
            } else if trimmed.starts_with("Priority:") {
                ticket.priority = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
            } else if trimmed.starts_with("Assignee:") {
                ticket.assignee = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
            } else if trimmed.starts_with("Reporter:") || trimmed.starts_with("Author:") {
                ticket.reporter = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
            } else if trimmed.starts_with("- ") && current_section == "comments" {
                // Comment in list format
                let comment_text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
                ticket.comments.push(Comment {
                    author: None,
                    body: comment_text.to_string(),
                });
            } else {
                // Regular content
                if !section_content.is_empty() {
                    section_content.push('\n');
                }
                section_content.push_str(trimmed);
            }
        }

        // Save last section
        if !section_content.is_empty() {
            self.save_section(&mut ticket, current_section, &section_content);
        }

        // If we have no structured content, treat the whole thing as description
        if ticket.title.is_none() && ticket.description.is_none() && ticket.comments.is_empty() {
            ticket.description = Some(content.to_string());
        }

        ticket
    }

    /// Save content to the appropriate ticket section.
    fn save_section(&self, ticket: &mut Ticket, section: &str, content: &str) {
        match section {
            "description" => ticket.description = Some(content.to_string()),
            "comments" => {
                // If content is present but no comments yet, add as single comment
                if !content.is_empty() && ticket.comments.is_empty() {
                    ticket.comments.push(Comment {
                        author: None,
                        body: content.to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Format ticket header with metadata.
    fn format_header(&self, ticket: &Ticket) -> String {
        let mut parts = Vec::new();

        if let Some(ref title) = ticket.title {
            parts.push(format!("# {}", title));
        }

        if let Some(ref key) = ticket.key {
            parts.push(format!("**Ticket**: {}", key));
        }

        if let Some(ref status) = ticket.status {
            parts.push(format!("**Status**: {}", status));
        }

        if let Some(ref priority) = ticket.priority {
            parts.push(format!("**Priority**: {}", priority));
        }

        if let Some(ref assignee) = ticket.assignee {
            parts.push(format!("**Assignee**: {}", assignee));
        }

        parts.join("\n")
    }

    /// Format description section.
    fn format_description(&self, ticket: &Ticket) -> Option<String> {
        ticket.description.as_ref().map(|desc| {
            format!("## Description\n\n{}", desc)
        })
    }

    /// Format comments section.
    fn format_comments(&self, ticket: &Ticket) -> Option<String> {
        if ticket.comments.is_empty() {
            return None;
        }

        let mut output = String::from("## Comments\n\n");

        for comment in &ticket.comments {
            if let Some(ref author) = comment.author {
                output.push_str(&format!("**{}**:\n", author));
            }
            output.push_str(&comment.body);
            output.push_str("\n\n---\n\n");
        }

        Some(output)
    }
}

/// Represents a ticket/issue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Ticket {
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reporter: Option<String>,
    #[serde(default)]
    comments: Vec<Comment>,
}

/// Represents a comment on a ticket.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Comment {
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    body: String,
}

impl Default for TicketingChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for TicketingChunker {
    fn name(&self) -> &'static str {
        "ticketing"
    }

    fn description(&self) -> &'static str {
        "Structured chunker for issues, PRs, and tickets with metadata preservation"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Parse the ticket
        let ticket = if item.content_type.contains("json") {
            self.parse_ticket_json(content).unwrap_or_else(|| self.parse_ticket_text(content))
        } else {
            self.parse_ticket_text(content)
        };

        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        // Create header chunk if metadata is included
        let header = if self.include_metadata {
            Some(self.format_header(&ticket))
        } else {
            None
        };

        // Create description chunk
        if let Some(desc) = self.format_description(&ticket) {
            let full_content = match &header {
                Some(h) => format!("{}\n\n{}", h, desc),
                None => desc,
            };

            let token_count = count_tokens(&full_content);

            // If description fits in one chunk
            if token_count <= config.chunk_size {
                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    full_content.clone(),
                    token_count,
                    0,
                    full_content.len(),
                    chunk_index,
                );

                chunk.metadata = ChunkMetadata {
                    content_type: Some("description".to_string()),
                    ..Default::default()
                };

                chunks.push(chunk);
                chunk_index += 1;
            } else {
                // Split description into multiple chunks
                // Use recursive splitting approach
                let sentences: Vec<&str> = full_content.split(". ").collect();
                let mut current_text = String::new();
                let mut current_tokens = 0;

                for sentence in sentences {
                    let sent_tokens = count_tokens(sentence);

                    if current_tokens + sent_tokens > config.chunk_size && !current_text.is_empty() {
                        let mut chunk = Chunk::new(
                            item.id,
                            item.source_id,
                            item.source_kind,
                            current_text.clone(),
                            current_tokens,
                            0,
                            current_text.len(),
                            chunk_index,
                        );

                        chunk.metadata = ChunkMetadata {
                            content_type: Some("description".to_string()),
                            ..Default::default()
                        };

                        chunks.push(chunk);
                        chunk_index += 1;
                        current_text = sentence.to_string();
                        current_tokens = sent_tokens;
                    } else {
                        if !current_text.is_empty() {
                            current_text.push_str(". ");
                        }
                        current_text.push_str(sentence);
                        current_tokens += sent_tokens;
                    }
                }

                if !current_text.is_empty() {
                    let mut chunk = Chunk::new(
                        item.id,
                        item.source_id,
                        item.source_kind,
                        current_text.clone(),
                        current_tokens,
                        0,
                        current_text.len(),
                        chunk_index,
                    );

                    chunk.metadata = ChunkMetadata {
                        content_type: Some("description".to_string()),
                        ..Default::default()
                    };

                    chunks.push(chunk);
                    chunk_index += 1;
                }
            }
        }

        // Create comment chunks
        if self.separate_comments && !ticket.comments.is_empty() {
            let comments_content = self.format_comments(&ticket).unwrap_or_default();
            let token_count = count_tokens(&comments_content);

            if token_count <= config.chunk_size {
                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    comments_content.clone(),
                    token_count,
                    0,
                    comments_content.len(),
                    chunk_index,
                );

                chunk.metadata = ChunkMetadata {
                    content_type: Some("comments".to_string()),
                    ..Default::default()
                };

                chunks.push(chunk);
            } else {
                // Split comments - each comment as potential chunk
                for comment in &ticket.comments {
                    let comment_text = format!(
                        "{}{}",
                        comment.author.as_ref().map(|a| format!("**{}**: ", a)).unwrap_or_default(),
                        comment.body
                    );

                    let token_count = count_tokens(&comment_text);

                    let mut chunk = Chunk::new(
                        item.id,
                        item.source_id,
                        item.source_kind,
                        comment_text.clone(),
                        token_count,
                        0,
                        comment_text.len(),
                        chunk_index,
                    );

                    chunk.metadata = ChunkMetadata {
                        content_type: Some("comment".to_string()),
                        author: comment.author.clone(),
                        ..Default::default()
                    };

                    chunks.push(chunk);
                    chunk_index += 1;
                }
            }
        }

        // If no chunks were created, treat as plain text
        if chunks.is_empty() {
            let token_count = count_tokens(content);
            chunks.push(Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                content.clone(),
                token_count,
                0,
                content.len(),
                0,
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

    fn create_ticket_item(content: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::Ticketing,
            content_type: "text/plain".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_structured_ticket() {
        let chunker = TicketingChunker::new();
        let content = r#"Title: Fix OAuth token refresh
Status: In Progress
Priority: High

Description:
The OAuth token refresh is not working properly. When the access token expires,
the system should automatically refresh it but instead returns a 401 error.

Comments:
- Alice: I've noticed this happens after exactly 1 hour
- Bob: Looking into it, seems like we're not storing the refresh token
"#;

        let item = create_ticket_item(content);
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_json_ticket() {
        let chunker = TicketingChunker::new();
        let content = r#"{
            "key": "PROJ-123",
            "title": "Fix login bug",
            "description": "Users can't log in on mobile devices.",
            "status": "Open",
            "comments": [
                {"author": "alice", "body": "Can reproduce on iOS"},
                {"author": "bob", "body": "Fixed in latest commit"}
            ]
        }"#;

        let item = SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::Ticketing,
            content_type: "application/json".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        };

        let config = ChunkConfig::with_size(1000);
        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }
}
