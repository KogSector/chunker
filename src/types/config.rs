//! Configuration types for chunking.

use serde::{Deserialize, Serialize};

use crate::{DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE, DEFAULT_MIN_CHARS_PER_SENTENCE};

/// Global chunking service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    /// Default chunk size in tokens
    pub default_chunk_size: usize,
    
    /// Default chunk overlap in tokens
    pub default_chunk_overlap: usize,
    
    /// Minimum characters per sentence
    pub min_chars_per_sentence: usize,
    
    /// URL of the embedding service
    pub embedding_service_url: Option<String>,
    
    /// URL of the graph service
    pub graph_service_url: Option<String>,
    
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: usize,
    
    /// Active chunking profile name
    pub active_profile: String,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            default_chunk_size: DEFAULT_CHUNK_SIZE,
            default_chunk_overlap: DEFAULT_CHUNK_OVERLAP,
            min_chars_per_sentence: DEFAULT_MIN_CHARS_PER_SENTENCE,
            embedding_service_url: None,
            graph_service_url: None,
            max_concurrent_jobs: 4,
            active_profile: "default".to_string(),
        }
    }
}

impl ChunkingConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            default_chunk_size: std::env::var("CHUNK_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CHUNK_SIZE),
            default_chunk_overlap: std::env::var("CHUNK_OVERLAP")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CHUNK_OVERLAP),
            min_chars_per_sentence: std::env::var("MIN_CHARS_PER_SENTENCE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MIN_CHARS_PER_SENTENCE),
            embedding_service_url: std::env::var("EMBEDDING_SERVICE_URL").ok(),
            graph_service_url: std::env::var("GRAPH_SERVICE_URL").ok(),
            max_concurrent_jobs: std::env::var("MAX_CONCURRENT_JOBS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            active_profile: std::env::var("ACTIVE_PROFILE")
                .unwrap_or_else(|_| "default".to_string()),
        }
    }
}

/// Configuration for individual chunk operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkConfig {
    /// Maximum tokens per chunk
    pub chunk_size: usize,
    
    /// Tokens to overlap between chunks
    pub chunk_overlap: usize,
    
    /// Minimum characters per sentence
    pub min_chars_per_sentence: usize,
    
    /// Whether to preserve whitespace
    pub preserve_whitespace: bool,
    
    /// Language for code chunking (if applicable)
    pub language: Option<String>,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            chunk_overlap: DEFAULT_CHUNK_OVERLAP,
            min_chars_per_sentence: DEFAULT_MIN_CHARS_PER_SENTENCE,
            preserve_whitespace: false,
            language: None,
        }
    }
}

impl ChunkConfig {
    /// Create a config with the given chunk size.
    pub fn with_size(size: usize) -> Self {
        Self {
            chunk_size: size,
            ..Default::default()
        }
    }

    /// Set the overlap.
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.chunk_overlap = overlap;
        self
    }

    /// Set the language.
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }
}

/// A named chunking profile with preset configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingProfile {
    /// Profile name
    pub name: String,
    
    /// Profile description
    pub description: String,
    
    /// Chunk size for this profile
    pub chunk_size: usize,
    
    /// Chunk overlap for this profile
    pub chunk_overlap: usize,
    
    /// Whether this profile is active
    pub active: bool,
}

impl ChunkingProfile {
    /// Create default profiles.
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                name: "default".to_string(),
                description: "Default balanced profile for general use".to_string(),
                chunk_size: 512,
                chunk_overlap: 50,
                active: true,
            },
            Self {
                name: "small".to_string(),
                description: "Smaller chunks for fine-grained retrieval".to_string(),
                chunk_size: 256,
                chunk_overlap: 25,
                active: false,
            },
            Self {
                name: "large".to_string(),
                description: "Larger chunks for more context".to_string(),
                chunk_size: 1024,
                chunk_overlap: 100,
                active: false,
            },
            Self {
                name: "code".to_string(),
                description: "Optimized for code with function-aware splitting".to_string(),
                chunk_size: 768,
                chunk_overlap: 64,
                active: false,
            },
        ]
    }
}

/// Chunking policy that defines rules for chunking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingPolicy {
    /// Policy name
    pub name: String,
    
    /// Maximum chunk size (hard limit)
    pub max_chunk_size: usize,
    
    /// Minimum chunk size (to avoid tiny chunks)
    pub min_chunk_size: usize,
    
    /// Whether to use sentence boundaries
    pub respect_sentence_boundaries: bool,
    
    /// Whether to use paragraph boundaries
    pub respect_paragraph_boundaries: bool,
    
    /// Whether to use code structure (for code)
    pub respect_code_structure: bool,
}

impl Default for ChunkingPolicy {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            max_chunk_size: 1024,
            min_chunk_size: 50,
            respect_sentence_boundaries: true,
            respect_paragraph_boundaries: true,
            respect_code_structure: true,
        }
    }
}
