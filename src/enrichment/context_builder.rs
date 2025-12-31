//! Context builder for creating rich embedding prefixes.
//!
//! Generates contextual prefixes for code chunks that improve
//! embedding quality by providing file, scope, and semantic information.

use std::collections::HashMap;

use crate::ast_engine::entity_extractor::{CodeEntity, EntityType, Import};
use crate::ast_engine::scope_tree::ScopeTree;
use crate::types::Chunk;

/// Context information for a chunk.
#[derive(Debug, Clone)]
pub struct ChunkContext {
    /// File path (relative to repository root).
    pub file_path: String,
    /// Repository name (if available).
    pub repository: Option<String>,
    /// Branch name (if available).
    pub branch: Option<String>,
    /// Programming language.
    pub language: String,
    /// Current scope path (e.g., "Module.Class.method").
    pub scope: String,
    /// Entities defined in this chunk.
    pub definitions: Vec<EntitySummary>,
    /// Dependencies/imports used.
    pub dependencies: Vec<String>,
    /// Related documentation (if any).
    pub documentation: Option<String>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

/// Summary of an entity for context.
#[derive(Debug, Clone)]
pub struct EntitySummary {
    /// Entity name.
    pub name: String,
    /// Entity type.
    pub entity_type: EntityType,
    /// Signature (for functions/methods).
    pub signature: Option<String>,
}

impl From<&CodeEntity> for EntitySummary {
    fn from(entity: &CodeEntity) -> Self {
        Self {
            name: entity.name.clone(),
            entity_type: entity.entity_type,
            signature: entity.signature.clone(),
        }
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
                        format!("{:?} {}", d.entity_type, d.name)
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
                parts.push(format!(
                    "# Dependencies: {} (+{} more)",
                    truncated.join(", "),
                    context.dependencies.len() - 5
                ));
            }
        }

        // Documentation
        if let Some(ref doc) = context.documentation {
            let doc_preview = if doc.len() > 100 {
                format!("{}...", &doc[..97])
            } else {
                doc.clone()
            };
            parts.push(format!("# Doc: {}", doc_preview));
        }

        let mut prefix = parts.join("\n");

        // Truncate if too long
        if prefix.len() > self.max_prefix_length {
            prefix = prefix[..self.max_prefix_length].to_string();
            // Don't cut in the middle of a line
            if let Some(last_newline) = prefix.rfind('\n') {
                prefix = prefix[..last_newline].to_string();
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

    /// Build context from entities and imports for a chunk.
    pub fn build_context_from_entities(
        &self,
        entities: &[CodeEntity],
        imports: &[Import],
        file_path: &str,
        language: &str,
        scope_tree: Option<&ScopeTree>,
        chunk_start_line: usize,
        chunk_end_line: usize,
    ) -> ChunkContext {
        // Find entities in this chunk's range
        let definitions: Vec<EntitySummary> = entities
            .iter()
            .filter(|e| e.start_line >= chunk_start_line && e.end_line <= chunk_end_line)
            .map(EntitySummary::from)
            .collect();

        // Find scope at chunk start
        let scope = scope_tree
            .and_then(|tree| tree.get_scope_path_at_line(chunk_start_line))
            .unwrap_or_default();

        // Collect dependencies from imports
        let dependencies: Vec<String> = imports.iter().map(|i| i.module.clone()).collect();

        ChunkContext {
            file_path: file_path.to_string(),
            repository: None,
            branch: None,
            language: language.to_string(),
            scope,
            definitions,
            dependencies,
            documentation: None,
            metadata: HashMap::new(),
        }
    }

    /// Enrich multiple chunks with context.
    pub fn enrich_all(
        &self,
        chunks: Vec<Chunk>,
        entities: &[CodeEntity],
        imports: &[Import],
        file_path: &str,
        language: &str,
        scope_tree: Option<&ScopeTree>,
    ) -> Vec<EnrichedChunk> {
        chunks
            .into_iter()
            .map(|chunk| {
                // Get line range from metadata if available
                let (start_line, end_line) = chunk
                    .metadata
                    .line_range
                    .unwrap_or_else(|| {
                        // Estimate from content
                        let lines = chunk.content.lines().count();
                        (1, 1 + lines)
                    });

                let context = self.build_context_from_entities(
                    entities,
                    imports,
                    file_path,
                    language,
                    scope_tree,
                    start_line,
                    end_line,
                );

                self.enrich(chunk, context)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entity(name: &str, entity_type: EntityType, signature: Option<&str>) -> CodeEntity {
        CodeEntity {
            name: name.to_string(),
            entity_type,
            scope_path: name.to_string(),
            start_line: 1,
            end_line: 10,
            start_byte: 0,
            end_byte: 100,
            signature: signature.map(String::from),
            docstring: None,
            dependencies: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_build_prefix() {
        let builder = ContextBuilder::new();
        let context = ChunkContext {
            file_path: "src/services/user.py".to_string(),
            repository: Some("my-app".to_string()),
            branch: None,
            language: "python".to_string(),
            scope: "UserService.getUser".to_string(),
            definitions: vec![EntitySummary {
                name: "getUser".to_string(),
                entity_type: EntityType::Method,
                signature: Some("async def getUser(id: str) -> User".to_string()),
            }],
            dependencies: vec!["sqlalchemy".to_string(), "asyncio".to_string()],
            documentation: None,
            metadata: HashMap::new(),
        };

        let prefix = builder.build_prefix(&context);

        assert!(prefix.contains("# File: src/services/user.py"));
        assert!(prefix.contains("# Language: python"));
        assert!(prefix.contains("# Scope: UserService.getUser"));
        assert!(prefix.contains("# Defines: async def getUser(id: str) -> User"));
        assert!(prefix.contains("# Dependencies: sqlalchemy, asyncio"));
    }

    #[test]
    fn test_enrich_chunk() {
        use uuid::Uuid;
        use crate::types::SourceKind;
        
        let builder = ContextBuilder::new();
        let chunk = Chunk::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            SourceKind::CodeRepo,
            "def hello(): pass".to_string(),
            5, // token_count
            0, // start_index
            17, // end_index
            0, // chunk_index
        );
        let context = ChunkContext {
            file_path: "test.py".to_string(),
            repository: None,
            branch: None,
            language: "python".to_string(),
            scope: "".to_string(),
            definitions: vec![],
            dependencies: vec![],
            documentation: None,
            metadata: HashMap::new(),
        };

        let enriched = builder.enrich(chunk, context);

        assert!(enriched.enriched_content.contains("# File: test.py"));
        assert!(enriched.enriched_content.contains("def hello(): pass"));
    }

    #[test]
    fn test_max_prefix_length() {
        let builder = ContextBuilder::new().with_max_prefix_length(50);
        let context = ChunkContext {
            file_path: "very/long/path/to/file/that/should/be/truncated.py".to_string(),
            repository: None,
            branch: None,
            language: "python".to_string(),
            scope: "SomeVeryLongScope.WithMoreNesting.AndEvenMore".to_string(),
            definitions: vec![],
            dependencies: vec![],
            documentation: None,
            metadata: HashMap::new(),
        };

        let prefix = builder.build_prefix(&context);

        assert!(prefix.len() <= 50);
    }
}
