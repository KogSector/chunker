//! Source types and request/response definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The kind of source the content comes from.
///
/// This determines which chunking strategy is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Code repository (GitHub, GitLab, etc.)
    CodeRepo,
    /// Generic document (PDF, Word, etc.)
    Document,
    /// Chat/messaging (Slack, Teams, Discord)
    Chat,
    /// Ticketing system (Jira, Linear, GitHub Issues)
    Ticketing,
    /// Wiki pages (Notion, Confluence)
    Wiki,
    /// Email threads
    Email,
    /// Web pages
    Web,
    /// Unknown or other sources
    Other,
}

impl SourceKind {
    /// Get the default content type for this source kind.
    pub fn default_content_type(&self) -> &'static str {
        match self {
            SourceKind::CodeRepo => "text/code",
            SourceKind::Document => "text/plain",
            SourceKind::Chat => "application/json",
            SourceKind::Ticketing => "text/markdown",
            SourceKind::Wiki => "text/markdown",
            SourceKind::Email => "text/plain",
            SourceKind::Web => "text/html",
            SourceKind::Other => "text/plain",
        }
    }

    /// Check if this source kind typically contains code.
    pub fn is_code(&self) -> bool {
        matches!(self, SourceKind::CodeRepo)
    }

    /// Check if this source kind is conversational.
    pub fn is_conversational(&self) -> bool {
        matches!(self, SourceKind::Chat | SourceKind::Email)
    }
}

impl std::fmt::Display for SourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceKind::CodeRepo => write!(f, "code_repo"),
            SourceKind::Document => write!(f, "document"),
            SourceKind::Chat => write!(f, "chat"),
            SourceKind::Ticketing => write!(f, "ticketing"),
            SourceKind::Wiki => write!(f, "wiki"),
            SourceKind::Email => write!(f, "email"),
            SourceKind::Web => write!(f, "web"),
            SourceKind::Other => write!(f, "other"),
        }
    }
}

/// A source item to be chunked.
///
/// This is the input unit received from the data service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceItem {
    /// Unique identifier for this source item
    pub id: Uuid,
    
    /// ID of the source (connected account/integration)
    pub source_id: Uuid,
    
    /// Kind of source
    pub source_kind: SourceKind,
    
    /// Content MIME type (e.g., "text/code:rust", "text/markdown")
    pub content_type: String,
    
    /// The actual content to chunk
    pub content: String,
    
    /// Additional metadata from the source
    pub metadata: serde_json::Value,
    
    /// When this item was created in the source system
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

impl SourceItem {
    /// Extract the language from the content type if it's code.
    ///
    /// For content types like "text/code:rust" or "text/code:python",
    /// returns the language identifier.
    pub fn extract_language(&self) -> Option<&str> {
        if self.content_type.starts_with("text/code:") {
            self.content_type.strip_prefix("text/code:")
        } else {
            // Try to get from metadata
            self.metadata.get("language").and_then(|v| v.as_str())
        }
    }

    /// Extract the file path from metadata.
    pub fn extract_path(&self) -> Option<&str> {
        self.metadata.get("path").and_then(|v| v.as_str())
    }

    /// Get content length in characters.
    pub fn content_len(&self) -> usize {
        self.content.len()
    }

    /// Check if this is a code item.
    pub fn is_code(&self) -> bool {
        self.source_kind.is_code() || self.content_type.starts_with("text/code:")
    }
}

/// Request to start a chunking job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartChunkJobRequest {
    /// ID of the source (connected account/integration)
    pub source_id: Uuid,
    
    /// Kind of source
    pub source_kind: SourceKind,
    
    /// Items to chunk
    pub items: Vec<SourceItem>,
}

/// Response when starting a chunking job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartChunkJobResponse {
    /// ID of the created job
    pub job_id: Uuid,
    
    /// Whether the job was accepted
    pub accepted: bool,
    
    /// Number of items queued
    pub items_count: usize,
    
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Status of a chunking job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkJobStatus {
    /// Job is queued but not started
    Pending,
    /// Job is currently running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed,
}

/// Response with job status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkJobStatusResponse {
    /// ID of the job
    pub job_id: Uuid,
    
    /// Current status
    pub status: ChunkJobStatus,
    
    /// Total items to process
    pub total_items: usize,
    
    /// Items processed so far
    pub processed_items: usize,
    
    /// Total chunks created
    pub chunks_created: usize,
    
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// When the job started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    
    /// When the job completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}
