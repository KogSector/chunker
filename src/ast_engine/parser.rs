//! Tree-sitter based AST parser.
//!
//! Provides multi-language AST parsing with semantic boundary detection
//! and node extraction for intelligent code chunking.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Tree};
use tracing::debug;

use crate::processing::Language as ProgLanguage;

/// Types of AST nodes relevant for chunking and entity extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Impl,
    Module,
    Import,
    Variable,
    Constant,
    Decorator,
    Comment,
    Block,
    Other,
}

impl NodeKind {
    /// Get the boundary strength for this node kind (0.0 - 1.0).
    /// Higher values indicate stronger chunk boundaries.
    pub fn boundary_strength(&self) -> f32 {
        match self {
            NodeKind::Class => 1.0,
            NodeKind::Interface => 1.0,
            NodeKind::Struct => 1.0,
            NodeKind::Trait => 1.0,
            NodeKind::Enum => 1.0,
            NodeKind::Impl => 0.95,
            NodeKind::Module => 0.95,
            NodeKind::Function => 0.9,
            NodeKind::Method => 0.9,
            NodeKind::Block => 0.6,
            NodeKind::Constant => 0.5,
            NodeKind::Variable => 0.4,
            NodeKind::Import => 0.3,
            NodeKind::Comment => 0.2,
            NodeKind::Decorator => 0.1,
            NodeKind::Other => 0.3,
        }
    }
}

/// A potential chunk boundary detected in the AST.
#[derive(Debug, Clone)]
pub struct AstBoundary {
    /// Line number where the boundary occurs.
    pub line: usize,
    /// Byte offset in the source.
    pub byte_offset: usize,
    /// Strength of the boundary (0.0 - 1.0).
    pub strength: f32,
    /// Kind of node at this boundary.
    pub node_kind: NodeKind,
    /// Context information (e.g., function name).
    pub context: Option<String>,
}

/// An extracted AST node.
#[derive(Debug, Clone)]
pub struct AstNode {
    /// Kind of node.
    pub kind: NodeKind,
    /// Name of the node (if applicable).
    pub name: Option<String>,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// Start column.
    pub start_col: usize,
    /// End column.
    pub end_col: usize,
    /// Child nodes.
    pub children: Vec<AstNode>,
}

impl AstNode {
    /// Get the number of lines this node spans.
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Get the byte length of this node.
    pub fn byte_length(&self) -> usize {
        self.end_byte.saturating_sub(self.start_byte)
    }
}

/// Result of parsing a source file.
#[derive(Debug)]
pub struct ParsedFile {
    /// The source content.
    pub content: String,
    /// The detected language.
    pub language: ProgLanguage,
    /// The parsed tree (if successful).
    pub tree: Option<Tree>,
    /// Extracted AST nodes.
    pub nodes: Vec<AstNode>,
    /// Detected chunk boundaries.
    pub boundaries: Vec<AstBoundary>,
    /// Any parse errors encountered.
    pub parse_errors: Vec<String>,
}

impl ParsedFile {
    /// Check if parsing was successful.
    pub fn is_valid(&self) -> bool {
        self.tree.is_some() && self.parse_errors.is_empty()
    }
}

/// Tree-sitter based AST parser.
pub struct AstParser {
    parsers: HashMap<String, Parser>,
}

impl Default for AstParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AstParser {
    /// Create a new AST parser with all supported languages.
    pub fn new() -> Self {
        let mut parsers = HashMap::new();

        // Initialize parsers for each supported language
        for (name, lang) in Self::available_languages() {
            let mut parser = Parser::new();
            if parser.set_language(&lang).is_ok() {
                parsers.insert(name.to_string(), parser);
                debug!("Loaded tree-sitter parser: {}", name);
            }
        }

        Self { parsers }
    }

    /// Get all available tree-sitter languages.
    fn available_languages() -> Vec<(&'static str, Language)> {
        vec![
            ("python", tree_sitter_python::language()),
            ("javascript", tree_sitter_javascript::language()),
            ("typescript", tree_sitter_typescript::language_typescript()),
            ("tsx", tree_sitter_typescript::language_tsx()),
            ("go", tree_sitter_go::language()),
            ("rust", tree_sitter_rust::language()),
            ("java", tree_sitter_java::language()),
            ("c", tree_sitter_c::language()),
            ("cpp", tree_sitter_cpp::language()),
            ("ruby", tree_sitter_ruby::language()),
        ]
    }

    /// Check if a language is supported.
    pub fn supports_language(&self, language: &str) -> bool {
        self.parsers.contains_key(language)
    }

    /// Get list of supported languages.
    pub fn supported_languages(&self) -> Vec<String> {
        self.parsers.keys().cloned().collect()
    }

    /// Parse source code into an AST.
    pub fn parse(&self, content: &str, language: &str) -> Result<ParsedFile> {
        let parser = self
            .parsers
            .get(language)
            .ok_or_else(|| anyhow!("Language not supported: {}", language))?;

        // We need to create a new parser since Parser is not thread-safe
        let mut parser = Parser::new();
        let tree_sitter_lang = Self::get_language(language)?;
        parser.set_language(&tree_sitter_lang)?;

        let tree = parser
            .parse(content.as_bytes(), None)
            .ok_or_else(|| anyhow!("Failed to parse content"))?;

        let lang = ProgLanguage::from_str(language);
        
        // Extract nodes
        let nodes = self.extract_nodes(&tree, content, language);
        
        // Find boundaries
        let boundaries = self.find_boundaries(&nodes);
        
        // Check for parse errors
        let parse_errors = self.check_parse_errors(&tree);

        Ok(ParsedFile {
            content: content.to_string(),
            language: lang,
            tree: Some(tree),
            nodes,
            boundaries,
            parse_errors,
        })
    }

    /// Get the tree-sitter language for a language name.
    fn get_language(name: &str) -> Result<Language> {
        match name {
            "python" => Ok(tree_sitter_python::language()),
            "javascript" => Ok(tree_sitter_javascript::language()),
            "typescript" => Ok(tree_sitter_typescript::language_typescript()),
            "tsx" => Ok(tree_sitter_typescript::language_tsx()),
            "go" => Ok(tree_sitter_go::language()),
            "rust" => Ok(tree_sitter_rust::language()),
            "java" => Ok(tree_sitter_java::language()),
            "c" => Ok(tree_sitter_c::language()),
            "cpp" => Ok(tree_sitter_cpp::language()),
            "ruby" => Ok(tree_sitter_ruby::language()),
            _ => Err(anyhow!("Language not supported: {}", name)),
        }
    }

    /// Extract all relevant nodes from the AST.
    fn extract_nodes(&self, tree: &Tree, content: &str, language: &str) -> Vec<AstNode> {
        let mut nodes = Vec::new();
        let node_types = crate::ast_engine::languages::get_node_types(language);

        self.visit_node(tree.root_node(), content, &node_types, &mut nodes);

        // Sort by start position
        nodes.sort_by_key(|n| (n.start_line, n.start_byte));
        nodes
    }

    /// Recursively visit nodes and extract relevant ones.
    fn visit_node(
        &self,
        node: tree_sitter::Node,
        content: &str,
        node_types: &HashMap<&str, NodeKind>,
        nodes: &mut Vec<AstNode>,
    ) {
        if let Some(&kind) = node_types.get(node.kind()) {
            let name = self.extract_node_name(&node, content);
            
            nodes.push(AstNode {
                kind,
                name,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                start_col: node.start_position().column,
                end_col: node.end_position().column,
                children: Vec::new(),
            });
        }

        // Visit children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, content, node_types, nodes);
        }
    }

    /// Extract the name from a node (e.g., function name, class name).
    fn extract_node_name(&self, node: &tree_sitter::Node, content: &str) -> Option<String> {
        // Look for identifier or name children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let child_type = child.kind();
            if matches!(child_type, "identifier" | "name" | "property_identifier" | "type_identifier") {
                return Some(content[child.start_byte()..child.end_byte()].to_string());
            }
        }
        None
    }

    /// Find chunk boundaries from extracted nodes.
    fn find_boundaries(&self, nodes: &[AstNode]) -> Vec<AstBoundary> {
        let mut boundaries: Vec<AstBoundary> = nodes
            .iter()
            .map(|node| AstBoundary {
                line: node.start_line,
                byte_offset: node.start_byte,
                strength: node.kind.boundary_strength(),
                node_kind: node.kind,
                context: node.name.clone(),
            })
            .collect();

        // Sort by line, then by strength (higher first)
        boundaries.sort_by(|a, b| {
            a.line
                .cmp(&b.line)
                .then(b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal))
        });

        boundaries
    }

    /// Check for parse errors in the tree.
    fn check_parse_errors(&self, tree: &Tree) -> Vec<String> {
        let mut errors = Vec::new();
        
        fn visit_for_errors(node: tree_sitter::Node, errors: &mut Vec<String>) {
            if node.is_error() || node.is_missing() {
                let pos = node.start_position();
                errors.push(format!(
                    "Parse error at line {}, column {}",
                    pos.row + 1,
                    pos.column
                ));
            }
            
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_for_errors(child, errors);
            }
        }

        visit_for_errors(tree.root_node(), &mut errors);
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python() {
        let parser = AstParser::new();
        let code = r#"
def hello(name: str) -> str:
    """Say hello."""
    return f"Hello, {name}!"

class Greeter:
    def greet(self, name: str) -> str:
        return hello(name)
"#;

        let result = parser.parse(code, "python").unwrap();
        
        assert!(result.is_valid());
        assert!(!result.nodes.is_empty());
        
        // Should find function and class
        let kinds: Vec<_> = result.nodes.iter().map(|n| n.kind).collect();
        assert!(kinds.contains(&NodeKind::Function));
        assert!(kinds.contains(&NodeKind::Class));
    }

    #[test]
    fn test_parse_rust() {
        let parser = AstParser::new();
        let code = r#"
fn main() {
    println!("Hello, world!");
}

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
"#;

        let result = parser.parse(code, "rust").unwrap();
        
        assert!(result.is_valid());
        
        let kinds: Vec<_> = result.nodes.iter().map(|n| n.kind).collect();
        assert!(kinds.contains(&NodeKind::Function));
        assert!(kinds.contains(&NodeKind::Struct));
        assert!(kinds.contains(&NodeKind::Impl));
    }

    #[test]
    fn test_boundary_detection() {
        let parser = AstParser::new();
        let code = "def foo(): pass\ndef bar(): pass";
        
        let result = parser.parse(code, "python").unwrap();
        
        // Should have boundaries for both functions
        assert!(result.boundaries.len() >= 2);
    }

    #[test]
    fn test_unsupported_language() {
        let parser = AstParser::new();
        let result = parser.parse("code", "unknown_lang");
        
        assert!(result.is_err());
    }
}
