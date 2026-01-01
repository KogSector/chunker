//! Context builder for creating rich embedding prefixes.
//!
//! Generates contextual prefixes for code chunks that improve
//! embedding quality by providing file, scope, and semantic information.
//!
//! This module receives normalized input from code-normalize-fetch and
//! adds context prefixes for better embedding quality.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::types::Chunk;

/// Type of entity for context display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Variable,
    Constant,
}

impl EntityType {
    /// Get display name for the entity type.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Function => "function",
            EntityType::Method => "method",
            EntityType::Class => "class",
            EntityType::Struct => "struct",
            EntityType::Enum => "enum",
            EntityType::Interface => "interface",
            EntityType::Trait => "trait",
            EntityType::Module => "module",
            EntityType::Variable => "variable",
            EntityType::Constant => "constant",
        }
    }
}

/// Summary of an entity for context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    /// Entity name.
    pub name: String,
    /// Entity type.
    pub entity_type: EntityType,
    /// Signature (for functions/methods).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Context information for a chunk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkContext {
    /// File path (relative to repository root).
    pub file_path: String,
    /// Repository name (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Branch name (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Programming language.
    #[serde(default)]
    pub language: String,
    /// Current scope path (e.g., "Module.Class.method").
    #[serde(default)]
    pub scope: String,
    /// Entities defined in this chunk.
    #[serde(default)]
    pub definitions: Vec<EntitySummary>,
    /// Dependencies/imports used.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Related documentation (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// Additional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ChunkContext {
    /// Create a new chunk context with minimal info.
    pub fn new(file_path: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            language: language.into(),
            ..Default::default()
        }
    }

    /// Set scope path.
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = scope.into();
        self
    }

    /// Add a definition.
    pub fn with_definition(mut self, entity: EntitySummary) -> Self {
        self.definitions.push(entity);
        self
    }

    /// Add dependencies.
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// A chunk enriched with context.
#[derive(Debug, Clone)]
pub struct EnrichedChunk {
    /// The original chunk.
    pub chunk: Chunk,
    /// Context information.
    pub context: ChunkContext,
    /// Full content with context prefix.
    pub enriched_content: String,
}

impl EnrichedChunk {
    /// Get the content to embed (with context prefix).
    pub fn embedding_content(&self) -> &str {
        &self.enriched_content
    }

    /// Get the original content (without prefix).
    pub fn original_content(&self) -> &str {
        &self.chunk.content
    }
}

/// Builder for creating context prefixes.
pub struct ContextBuilder {
    /// Whether to include file path in prefix.
    include_file_path: bool,
    /// Whether to include scope information.
    include_scope: bool,
    /// Whether to include definitions summary.
    include_definitions: bool,
    /// Whether to include dependencies.
    include_dependencies: bool,
    /// Maximum prefix length (in characters).
    max_prefix_length: usize,
    /// Separator between prefix and content.
    separator: String,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            include_file_path: true,
            include_scope: true,
            include_definitions: true,
            include_dependencies: true,
            max_prefix_length: 500,
            separator: "\n---\n".to_string(),
        }
    }
}

impl ContextBuilder {
    /// Create a new context builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to include file path.
    pub fn with_file_path(mut self, include: bool) -> Self {
        self.include_file_path = include;
        self
    }

    /// Set whether to include scope.
    pub fn with_scope(mut self, include: bool) -> Self {
        self.include_scope = include;
        self
    }

    /// Set whether to include definitions.
    pub fn with_definitions(mut self, include: bool) -> Self {
        self.include_definitions = include;
        self
    }

    /// Set whether to include dependencies.
    pub fn with_dependencies(mut self, include: bool) -> Self {
        self.include_dependencies = include;
        self
    }

    /// Set maximum prefix length.
    pub fn with_max_prefix_length(mut self, max_length: usize) -> Self {
        self.max_prefix_length = max_length;
        self
    }

    /// Set the separator between prefix and content.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Build context prefix for a chunk.
    pub fn build_prefix(&self, context: &ChunkContext) -> String {
        let mut parts = Vec::new();

        // File path
        if self.include_file_path {
            parts.push(format!("# File: {}", context.file_path));
            if !context.language.is_empty() {
                parts.push(format!("# Language: {}", context.language));
            }
        }

        // Repository info
        if let Some(ref repo) = context.repository {
            parts.push(format!("# Repository: {}", repo));
        }

        // Scope
        if self.include_scope && !context.scope.is_empty() {
            parts.push(format!("# Scope: {}", context.scope));
        }

        // Definitions
        if self.include_definitions && !context.definitions.is_empty() {
            let defs: Vec<String> = context
                .definitions
                .iter()
                .map(|d| {
                    if let Some(ref sig) = d.signature {
                        sig.clone()
                    } else {
                        format!("{} {}", d.entity_type.as_str(), d.name)
                    }
                })
                .collect();
            
            if defs.len() == 1 {
                parts.push(format!("# Defines: {}", defs[0]));
            } else if !defs.is_empty() {
                parts.push(format!("# Defines: {}", defs.join(", ")));
            }
        }

        // Dependencies
        if self.include_dependencies && !context.dependencies.is_empty() {
            let deps = context.dependencies.join(", ");
            if deps.len() <= 100 {
                parts.push(format!("# Dependencies: {}", deps));
            } else {
                // Truncate long dependency lists
                let truncated: Vec<_> = context.dependencies.iter().take(5).cloned().collect();
                parts.push(format!("# Dependencies: {} ...", truncated.join(", ")));
            }
        }

        // Documentation
        if let Some(ref doc) = context.documentation {
            let doc_line = if doc.len() > 100 {
                format!("{}...", &doc[..97])
            } else {
                doc.clone()
            };
            parts.push(format!("# Doc: {}", doc_line));
        }

        // Enforce max length
        let mut prefix = parts.join("\n");
        if prefix.len() > self.max_prefix_length {
            prefix = prefix[..self.max_prefix_length].to_string();
            // Find last newline to avoid partial lines
            if let Some(idx) = prefix.rfind('\n') {
                prefix.truncate(idx);
            }
        }

        prefix
    }

    /// Enrich a chunk with context.
    pub fn enrich(&self, chunk: Chunk, context: ChunkContext) -> EnrichedChunk {
        let prefix = self.build_prefix(&context);
        let enriched_content = if prefix.is_empty() {
            chunk.content.clone()
        } else {
            format!("{}{}{}", prefix, self.separator, chunk.content)
        };

        EnrichedChunk {
            chunk,
            context,
            enriched_content,
        }
    }

    /// Enrich multiple chunks with file-level context.
    pub fn enrich_all(
        &self,
        chunks: Vec<Chunk>,
        file_path: &str,
        language: &str,
        definitions: Vec<EntitySummary>,
        dependencies: Vec<String>,
    ) -> Vec<EnrichedChunk> {
        chunks
            .into_iter()
            .map(|chunk| {
                // Find definitions in this chunk's line range
                let chunk_defs: Vec<_> = definitions
                    .iter()
                    .filter(|_d| {
                        // Would filter by line range if we had that info
                        // For now, include all defs
                        true
                    })
                    .cloned()
                    .collect();

                let context = ChunkContext {
                    file_path: file_path.to_string(),
                    language: language.to_string(),
                    definitions: chunk_defs,
                    dependencies: dependencies.clone(),
                    ..Default::default()
                };

                self.enrich(chunk, context)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Chunk, ChunkMetadata};

    #[test]
    fn test_context_prefix() {
        let builder = ContextBuilder::new();
        let context = ChunkContext {
            file_path: "src/main.py".to_string(),
            language: "python".to_string(),
            scope: "main".to_string(),
            definitions: vec![EntitySummary {
                name: "process".to_string(),
                entity_type: EntityType::Function,
                signature: Some("def process(data: list) -> dict".to_string()),
            }],
            dependencies: vec!["json".to_string(), "os".to_string()],
            ..Default::default()
        };

        let prefix = builder.build_prefix(&context);
        
        assert!(prefix.contains("File: src/main.py"));
        assert!(prefix.contains("Language: python"));
        assert!(prefix.contains("Scope: main"));
        assert!(prefix.contains("def process(data: list) -> dict"));
        assert!(prefix.contains("Dependencies:"));
    }

    #[test]
    fn test_enrich_chunk() {
        let builder = ContextBuilder::new();
        let chunk = Chunk::new("def hello():\n    print('Hello')");
        let context = ChunkContext::new("hello.py", "python");
        
        let enriched = builder.enrich(chunk, context);
        
        assert!(enriched.enriched_content.contains("File: hello.py"));
        assert!(enriched.enriched_content.contains("def hello()"));
    }
}
