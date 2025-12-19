//! AST-aware code chunker using tree-sitter.

use anyhow::{anyhow, Result};
use tree_sitter::{Language, Node, Parser, Tree};

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem};

/// Code chunker that uses tree-sitter for AST-aware chunking.
///
/// This chunker parses code into an Abstract Syntax Tree and creates
/// chunks based on semantic code units like functions, classes, and methods.
/// This produces much better chunks for code than naive text splitting.
pub struct CodeChunker {
    /// Supported languages and their tree-sitter language bindings
    languages: std::collections::HashMap<String, Language>,
}

impl CodeChunker {
    /// Create a new code chunker with all supported languages.
    pub fn new() -> Self {
        let mut languages = std::collections::HashMap::new();

        // Register all supported languages
        languages.insert("rust".to_string(), tree_sitter_rust::language());
        languages.insert("rs".to_string(), tree_sitter_rust::language());
        languages.insert("python".to_string(), tree_sitter_python::language());
        languages.insert("py".to_string(), tree_sitter_python::language());
        languages.insert("javascript".to_string(), tree_sitter_javascript::language());
        languages.insert("js".to_string(), tree_sitter_javascript::language());
        languages.insert("jsx".to_string(), tree_sitter_javascript::language());
        languages.insert("typescript".to_string(), tree_sitter_typescript::language_typescript());
        languages.insert("ts".to_string(), tree_sitter_typescript::language_typescript());
        languages.insert("tsx".to_string(), tree_sitter_typescript::language_tsx());
        languages.insert("go".to_string(), tree_sitter_go::language());
        languages.insert("c".to_string(), tree_sitter_c::language());
        languages.insert("cpp".to_string(), tree_sitter_cpp::language());
        languages.insert("c++".to_string(), tree_sitter_cpp::language());
        languages.insert("java".to_string(), tree_sitter_java::language());
        languages.insert("ruby".to_string(), tree_sitter_ruby::language());
        languages.insert("rb".to_string(), tree_sitter_ruby::language());

        Self { languages }
    }

    /// Get the tree-sitter language for the given language identifier.
    fn get_language(&self, lang: &str) -> Option<&Language> {
        self.languages.get(&lang.to_lowercase())
    }

    /// Parse code with tree-sitter.
    fn parse_code(&self, code: &str, language: &Language) -> Result<Tree> {
        let mut parser = Parser::new();
        parser.set_language(language)?;

        parser
            .parse(code.as_bytes(), None)
            .ok_or_else(|| anyhow!("Failed to parse code"))
    }

    /// Get the node types that represent points of interest for chunking.
    fn get_chunk_node_types(language: &str) -> Vec<&'static str> {
        match language.to_lowercase().as_str() {
            "rust" | "rs" => vec![
                "function_item",
                "impl_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "mod_item",
                "const_item",
                "static_item",
                "type_item",
            ],
            "python" | "py" => vec![
                "function_definition",
                "class_definition",
                "decorated_definition",
            ],
            "javascript" | "js" | "jsx" => vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "method_definition",
                "export_statement",
            ],
            "typescript" | "ts" | "tsx" => vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "method_definition",
                "interface_declaration",
                "type_alias_declaration",
                "export_statement",
            ],
            "go" => vec![
                "function_declaration",
                "method_declaration",
                "type_declaration",
                "const_declaration",
                "var_declaration",
            ],
            "java" => vec![
                "class_declaration",
                "method_declaration",
                "interface_declaration",
                "constructor_declaration",
            ],
            "c" | "cpp" | "c++" => vec![
                "function_definition",
                "struct_specifier",
                "class_specifier",
                "namespace_definition",
            ],
            "ruby" | "rb" => vec![
                "method",
                "class",
                "module",
                "singleton_method",
            ],
            _ => vec!["function", "class", "method"],
        }
    }

    /// Collect all nodes of interest from the AST.
    fn collect_chunk_nodes<'a>(
        &self,
        node: Node<'a>,
        chunk_types: &[&str],
        nodes: &mut Vec<Node<'a>>,
    ) {
        if chunk_types.contains(&node.kind()) {
            nodes.push(node);
        } else {
            // Recurse into children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.collect_chunk_nodes(child, chunk_types, nodes);
            }
        }
    }

    /// Extract text for a node, including any preceding comments.
    fn extract_node_text<'a>(
        &self,
        node: Node<'a>,
        source: &'a [u8],
        tree: &'a Tree,
    ) -> (String, usize, usize) {
        // Look for preceding comments or decorators
        let mut start_byte = node.start_byte();
        let end_byte = node.end_byte();

        // Walk backwards to find attached comments
        if let Some(prev) = self.find_preceding_comment(node, tree) {
            start_byte = prev.start_byte();
        }

        let text = String::from_utf8_lossy(&source[start_byte..end_byte]).to_string();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        (text, start_line, end_line)
    }

    /// Find a comment node immediately preceding the given node.
    fn find_preceding_comment<'a>(&self, node: Node<'a>, _tree: &'a Tree) -> Option<Node<'a>> {
        let mut prev = node.prev_sibling();

        while let Some(p) = prev {
            if p.kind().contains("comment") {
                return Some(p);
            } else if p.kind() == "decorated_definition" || p.kind().contains("decorator") {
                // Include decorators (Python)
                return Some(p);
            } else if !p.kind().trim().is_empty() {
                // Non-empty, non-comment node found
                break;
            }
            prev = p.prev_sibling();
        }

        None
    }

    /// Group nodes into chunks that fit within token limits.
    fn group_nodes_into_chunks<'a>(
        &self,
        nodes: Vec<Node<'a>>,
        source: &'a [u8],
        tree: &'a Tree,
        chunk_size: usize,
        item: &SourceItem,
        language: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_nodes: Vec<(String, usize, usize)> = Vec::new();
        let mut current_tokens = 0;
        let mut chunk_index = 0;

        for node in nodes {
            let (text, start_line, end_line) = self.extract_node_text(node, source, tree);
            let node_tokens = count_tokens(&text);

            // If single node exceeds chunk size, we need to handle it specially
            if node_tokens > chunk_size {
                // First, flush current accumulated nodes
                if !current_nodes.is_empty() {
                    let chunk = self.create_chunk_from_nodes(
                        &current_nodes,
                        item,
                        chunk_index,
                        language,
                    );
                    chunks.push(chunk);
                    chunk_index += 1;
                    current_nodes.clear();
                    current_tokens = 0;
                }

                // Add the large node as its own chunk(s)
                // For very large functions, we might need to split them
                let large_chunks = self.split_large_node(
                    &text,
                    start_line,
                    end_line,
                    chunk_size,
                    item,
                    &mut chunk_index,
                    language,
                );
                chunks.extend(large_chunks);
            } else if current_tokens + node_tokens > chunk_size {
                // Current chunk is full, start a new one
                let chunk = self.create_chunk_from_nodes(
                    &current_nodes,
                    item,
                    chunk_index,
                    language,
                );
                chunks.push(chunk);
                chunk_index += 1;

                current_nodes = vec![(text, start_line, end_line)];
                current_tokens = node_tokens;
            } else {
                // Add to current chunk
                current_nodes.push((text, start_line, end_line));
                current_tokens += node_tokens;
            }
        }

        // Don't forget the last chunk
        if !current_nodes.is_empty() {
            let chunk = self.create_chunk_from_nodes(
                &current_nodes,
                item,
                chunk_index,
                language,
            );
            chunks.push(chunk);
        }

        chunks
    }

    /// Create a chunk from accumulated node texts.
    fn create_chunk_from_nodes(
        &self,
        nodes: &[(String, usize, usize)],
        item: &SourceItem,
        chunk_index: usize,
        language: &str,
    ) -> Chunk {
        let content: String = nodes.iter().map(|(t, _, _)| t.as_str()).collect::<Vec<_>>().join("\n\n");
        let token_count = count_tokens(&content);

        let start_line = nodes.first().map(|(_, s, _)| *s).unwrap_or(1);
        let end_line = nodes.last().map(|(_, _, e)| *e).unwrap_or(1);

        // Calculate character positions (approximate)
        let start_index = 0; // Would need to track properly
        let end_index = content.len();

        let mut chunk = Chunk::new(
            item.id,
            item.source_id,
            item.source_kind,
            content,
            token_count,
            start_index,
            end_index,
            chunk_index,
        );

        // Add code-specific metadata
        chunk.metadata = ChunkMetadata::for_code(language, item.extract_path())
            .with_lines(start_line, end_line);

        chunk
    }

    /// Split a large node (e.g., a huge function) into smaller chunks.
    fn split_large_node(
        &self,
        text: &str,
        start_line: usize,
        end_line: usize,
        chunk_size: usize,
        item: &SourceItem,
        chunk_index: &mut usize,
        language: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut current_text = String::new();
        let mut current_start = start_line;

        for (i, line) in lines.iter().enumerate() {
            let test_text = if current_text.is_empty() {
                line.to_string()
            } else {
                format!("{}\n{}", current_text, line)
            };

            if count_tokens(&test_text) > chunk_size && !current_text.is_empty() {
                // Create chunk from current text
                let token_count = count_tokens(&current_text);
                let current_end = start_line + i - 1;

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    current_text.clone(),
                    token_count,
                    0,
                    current_text.len(),
                    *chunk_index,
                );

                chunk.metadata = ChunkMetadata::for_code(language, item.extract_path())
                    .with_lines(current_start, current_end);

                chunks.push(chunk);
                *chunk_index += 1;

                current_text = line.to_string();
                current_start = start_line + i;
            } else {
                current_text = test_text;
            }
        }

        // Last chunk
        if !current_text.is_empty() {
            let token_count = count_tokens(&current_text);

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                current_text.clone(),
                token_count,
                0,
                current_text.len(),
                *chunk_index,
            );

            chunk.metadata = ChunkMetadata::for_code(language, item.extract_path())
                .with_lines(current_start, end_line);

            chunks.push(chunk);
            *chunk_index += 1;
        }

        chunks
    }

    /// Fallback: simple line-based chunking when parsing fails.
    fn fallback_chunk(&self, item: &SourceItem, config: &ChunkConfig, language: &str) -> Vec<Chunk> {
        let lines: Vec<&str> = item.content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_lines = Vec::new();
        let mut current_tokens = 0;
        let mut chunk_index = 0;
        let mut start_line = 1;

        for (i, line) in lines.iter().enumerate() {
            let line_tokens = count_tokens(line);

            if current_tokens + line_tokens > config.chunk_size && !current_lines.is_empty() {
                let content = current_lines.join("\n");
                let token_count = count_tokens(&content);
                let end_line = i;

                let mut chunk = Chunk::new(
                    item.id,
                    item.source_id,
                    item.source_kind,
                    content.clone(),
                    token_count,
                    0,
                    content.len(),
                    chunk_index,
                );

                chunk.metadata = ChunkMetadata::for_code(language, item.extract_path())
                    .with_lines(start_line, end_line);

                chunks.push(chunk);
                chunk_index += 1;
                current_lines = vec![*line];
                current_tokens = line_tokens;
                start_line = i + 1;
            } else {
                current_lines.push(*line);
                current_tokens += line_tokens;
            }
        }

        // Last chunk
        if !current_lines.is_empty() {
            let content = current_lines.join("\n");
            let token_count = count_tokens(&content);
            let end_line = lines.len();

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                content.clone(),
                token_count,
                0,
                content.len(),
                chunk_index,
            );

            chunk.metadata = ChunkMetadata::for_code(language, item.extract_path())
                .with_lines(start_line, end_line);

            chunks.push(chunk);
        }

        chunks
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
        "AST-aware code chunker using tree-sitter for semantic code splitting"
    }

    fn supports_language(&self, language: Option<&str>) -> bool {
        match language {
            Some(lang) => self.get_language(lang).is_some(),
            None => false,
        }
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Determine the language
        let language = config.language.as_deref()
            .or_else(|| item.extract_language())
            .unwrap_or("text");

        // Get tree-sitter language
        let ts_language = match self.get_language(language) {
            Some(lang) => lang,
            None => {
                // Fallback to line-based chunking for unsupported languages
                return Ok(self.fallback_chunk(item, config, language));
            }
        };

        // Parse the code
        let tree = match self.parse_code(content, ts_language) {
            Ok(t) => t,
            Err(_) => {
                // Fallback if parsing fails
                return Ok(self.fallback_chunk(item, config, language));
            }
        };

        let root_node = tree.root_node();

        // Check for parse errors
        if root_node.has_error() {
            // Try to chunk anyway, but note the error
            tracing::warn!("Code has syntax errors, chunking may be imprecise");
        }

        // Collect nodes of interest
        let chunk_types = Self::get_chunk_node_types(language);
        let mut nodes = Vec::new();
        self.collect_chunk_nodes(root_node, &chunk_types, &mut nodes);

        // If no suitable nodes found, use top-level children
        if nodes.is_empty() {
            let mut cursor = root_node.walk();
            nodes = root_node.children(&mut cursor).collect();
        }

        // If still no nodes, fallback
        if nodes.is_empty() {
            return Ok(self.fallback_chunk(item, config, language));
        }

        // Group nodes into chunks
        let source = content.as_bytes();
        let chunks = self.group_nodes_into_chunks(
            nodes,
            source,
            &tree,
            config.chunk_size,
            item,
            language,
        );

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceKind;
    use uuid::Uuid;

    fn create_code_item(content: &str, language: &str) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: SourceKind::CodeRepo,
            content_type: format!("text/code:{}", language),
            content: content.to_string(),
            metadata: serde_json::json!({"path": "test.rs", "language": language}),
            created_at: None,
        }
    }

    #[test]
    fn test_rust_function_chunking() {
        let chunker = CodeChunker::new();
        let code = r#"
fn hello() {
    println!("Hello, world!");
}

fn goodbye() {
    println!("Goodbye, world!");
}
"#;
        let item = create_code_item(code, "rust");
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_python_function_chunking() {
        let chunker = CodeChunker::new();
        let code = r#"
def hello():
    print("Hello, world!")

def goodbye():
    print("Goodbye, world!")
"#;
        let item = create_code_item(code, "python");
        let config = ChunkConfig::with_size(1000);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_language_support() {
        let chunker = CodeChunker::new();
        assert!(chunker.supports_language(Some("rust")));
        assert!(chunker.supports_language(Some("python")));
        assert!(chunker.supports_language(Some("javascript")));
        assert!(!chunker.supports_language(Some("unknown_lang")));
    }
}
