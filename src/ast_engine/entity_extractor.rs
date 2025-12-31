//! Entity extractor for code analysis.
//!
//! Extracts structured code entities (functions, classes, imports, etc.)
//! from parsed AST nodes with relationship information.

use std::collections::HashMap;

use crate::ast_engine::parser::{AstNode, NodeKind, ParsedFile};

/// Types of code entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Import,
    Variable,
    Constant,
}

impl From<NodeKind> for EntityType {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Function => EntityType::Function,
            NodeKind::Method => EntityType::Method,
            NodeKind::Class => EntityType::Class,
            NodeKind::Struct => EntityType::Struct,
            NodeKind::Enum => EntityType::Enum,
            NodeKind::Interface => EntityType::Interface,
            NodeKind::Trait => EntityType::Trait,
            NodeKind::Module => EntityType::Module,
            NodeKind::Import => EntityType::Import,
            NodeKind::Variable => EntityType::Variable,
            NodeKind::Constant => EntityType::Constant,
            _ => EntityType::Function, // Default fallback
        }
    }
}

/// A code entity extracted from the AST.
#[derive(Debug, Clone)]
pub struct CodeEntity {
    /// Name of the entity.
    pub name: String,
    /// Type of the entity.
    pub entity_type: EntityType,
    /// Full scope path (e.g., "Module.Class.method").
    pub scope_path: String,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset.
    pub end_byte: usize,
    /// Function/method signature (if applicable).
    pub signature: Option<String>,
    /// Documentation string (if found).
    pub docstring: Option<String>,
    /// Names of entities this one depends on.
    pub dependencies: Vec<String>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl CodeEntity {
    /// Get the number of lines this entity spans.
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Check if this entity is a definition (function, class, etc.).
    pub fn is_definition(&self) -> bool {
        matches!(
            self.entity_type,
            EntityType::Function
                | EntityType::Method
                | EntityType::Class
                | EntityType::Struct
                | EntityType::Enum
                | EntityType::Interface
                | EntityType::Trait
        )
    }
}

/// An import statement.
#[derive(Debug, Clone)]
pub struct Import {
    /// The module or package being imported.
    pub module: String,
    /// Specific items imported (if any).
    pub items: Vec<String>,
    /// Alias (if any).
    pub alias: Option<String>,
    /// Line number.
    pub line: usize,
    /// Whether this is a relative import.
    pub is_relative: bool,
}

/// Entity extractor for parsed files.
pub struct EntityExtractor;

impl EntityExtractor {
    /// Extract all entities from a parsed file.
    pub fn extract(parsed: &ParsedFile) -> Vec<CodeEntity> {
        let mut entities = Vec::new();
        let content = &parsed.content;

        for node in &parsed.nodes {
            if let Some(entity) = Self::node_to_entity(node, content, "") {
                entities.push(entity);
            }
        }

        entities
    }

    /// Extract entities with scope context.
    pub fn extract_with_scope(parsed: &ParsedFile, base_scope: &str) -> Vec<CodeEntity> {
        let mut entities = Vec::new();
        let content = &parsed.content;

        for node in &parsed.nodes {
            if let Some(entity) = Self::node_to_entity(node, content, base_scope) {
                entities.push(entity);
            }
        }

        entities
    }

    /// Convert an AST node to a code entity.
    fn node_to_entity(node: &AstNode, content: &str, parent_scope: &str) -> Option<CodeEntity> {
        // Skip nodes without names (unless they're imports)
        let name = match &node.name {
            Some(n) => n.clone(),
            None if node.kind == NodeKind::Import => {
                // For imports, try to extract the module name from content
                let text = &content[node.start_byte..node.end_byte];
                Self::extract_import_name(text).unwrap_or_else(|| "<unknown>".to_string())
            }
            None => return None,
        };

        // Build scope path
        let scope_path = if parent_scope.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", parent_scope, name)
        };

        // Extract signature for functions/methods
        let signature = if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
            Some(Self::extract_signature(node, content))
        } else {
            None
        };

        // Extract docstring
        let docstring = Self::extract_docstring(node, content);

        Some(CodeEntity {
            name,
            entity_type: EntityType::from(node.kind),
            scope_path,
            start_line: node.start_line,
            end_line: node.end_line,
            start_byte: node.start_byte,
            end_byte: node.end_byte,
            signature,
            docstring,
            dependencies: Vec::new(), // TODO: Extract from content
            metadata: HashMap::new(),
        })
    }

    /// Extract function/method signature from content.
    fn extract_signature(node: &AstNode, content: &str) -> String {
        let text = &content[node.start_byte..node.end_byte];
        
        // Find the first line or up to the opening brace/colon
        let first_line = text.lines().next().unwrap_or("");
        
        // Trim and clean up
        first_line
            .trim()
            .trim_end_matches('{')
            .trim_end_matches(':')
            .trim()
            .to_string()
    }

    /// Extract docstring if present.
    fn extract_docstring(node: &AstNode, content: &str) -> Option<String> {
        let text = &content[node.start_byte..node.end_byte];
        
        // Look for docstrings in the content
        // Python: """ or '''
        // Rust: /// or //!
        // JS/TS: /** */
        
        for line in text.lines().skip(1).take(5) {
            let trimmed = line.trim();
            
            // Python docstrings
            if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                // Find the closing quotes
                let start = trimmed.find(|c| c == '"' || c == '\'').unwrap_or(0);
                let doc_content = &trimmed[3..];
                if let Some(end) = doc_content.find("\"\"\"").or_else(|| doc_content.find("'''")) {
                    return Some(doc_content[..end].trim().to_string());
                } else {
                    return Some(doc_content.trim().to_string());
                }
            }
            
            // Rust doc comments
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                return Some(trimmed[3..].trim().to_string());
            }
            
            // JSDoc
            if trimmed.starts_with("/**") {
                let cleaned = trimmed
                    .trim_start_matches("/**")
                    .trim_end_matches("*/")
                    .trim_start_matches('*')
                    .trim();
                return Some(cleaned.to_string());
            }
        }
        
        None
    }

    /// Extract import module name from import statement text.
    fn extract_import_name(text: &str) -> Option<String> {
        let trimmed = text.trim();
        
        // Python: import X, from X import Y
        if trimmed.starts_with("from ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        } else if trimmed.starts_with("import ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].trim_end_matches(',').to_string());
            }
        }
        
        // JS/TS: import X from 'Y'
        if trimmed.starts_with("import ") {
            if let Some(from_pos) = trimmed.find(" from ") {
                let module_part = &trimmed[from_pos + 6..];
                let module = module_part
                    .trim()
                    .trim_start_matches(|c| c == '\'' || c == '"')
                    .trim_end_matches(|c| c == '\'' || c == '"' || c == ';');
                return Some(module.to_string());
            }
        }
        
        // Rust: use X::Y
        if trimmed.starts_with("use ") {
            let module_part = &trimmed[4..];
            let module = module_part
                .trim()
                .trim_end_matches(';')
                .split("::")
                .next()
                .unwrap_or("");
            return Some(module.to_string());
        }
        
        // Go: import "X"
        if trimmed.starts_with("import ") {
            let module_part = &trimmed[7..];
            let module = module_part
                .trim()
                .trim_start_matches(|c| c == '(' || c == '"')
                .trim_end_matches(|c| c == ')' || c == '"');
            return Some(module.to_string());
        }
        
        None
    }

    /// Extract imports from a parsed file.
    pub fn extract_imports(parsed: &ParsedFile) -> Vec<Import> {
        let mut imports = Vec::new();
        let content = &parsed.content;

        for node in &parsed.nodes {
            if node.kind == NodeKind::Import {
                let text = &content[node.start_byte..node.end_byte];
                if let Some(import) = Self::parse_import(text, node.start_line) {
                    imports.push(import);
                }
            }
        }

        imports
    }

    /// Parse an import statement.
    fn parse_import(text: &str, line: usize) -> Option<Import> {
        let module = Self::extract_import_name(text)?;
        
        // Extract imported items if present
        let items = Self::extract_import_items(text);
        
        // Check for alias
        let alias = if text.contains(" as ") {
            let parts: Vec<&str> = text.split(" as ").collect();
            if parts.len() >= 2 {
                Some(parts[1].trim().trim_end_matches(|c| c == ';' || c == ',').to_string())
            } else {
                None
            }
        } else {
            None
        };
        
        // Check for relative import (Python)
        let is_relative = text.contains("from .") || text.contains("from ..");

        Some(Import {
            module,
            items,
            alias,
            line,
            is_relative,
        })
    }

    /// Extract specific items from an import statement.
    fn extract_import_items(text: &str) -> Vec<String> {
        let mut items = Vec::new();
        
        // Python: from X import a, b, c
        if let Some(import_pos) = text.find(" import ") {
            let items_part = &text[import_pos + 8..];
            for item in items_part.split(',') {
                let clean = item
                    .trim()
                    .split(" as ")
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(|c| c == ';' || c == ')');
                if !clean.is_empty() && clean != "*" {
                    items.push(clean.to_string());
                }
            }
        }
        
        // JS/TS: import { a, b, c } from 'X'
        if text.contains('{') && text.contains('}') {
            if let Some(start) = text.find('{') {
                if let Some(end) = text.find('}') {
                    let items_part = &text[start + 1..end];
                    for item in items_part.split(',') {
                        let clean = item
                            .trim()
                            .split(" as ")
                            .next()
                            .unwrap_or("")
                            .trim();
                        if !clean.is_empty() {
                            items.push(clean.to_string());
                        }
                    }
                }
            }
        }
        
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_import_name() {
        assert_eq!(
            EntityExtractor::extract_import_name("from os import path"),
            Some("os".to_string())
        );
        assert_eq!(
            EntityExtractor::extract_import_name("import json"),
            Some("json".to_string())
        );
    }

    #[test]
    fn test_extract_js_import_name() {
        assert_eq!(
            EntityExtractor::extract_import_name("import React from 'react'"),
            Some("react".to_string())
        );
        assert_eq!(
            EntityExtractor::extract_import_name("import { useState } from 'react'"),
            Some("react".to_string())
        );
    }

    #[test]
    fn test_extract_rust_import_name() {
        assert_eq!(
            EntityExtractor::extract_import_name("use std::collections::HashMap;"),
            Some("std".to_string())
        );
    }

    #[test]
    fn test_extract_import_items() {
        let items = EntityExtractor::extract_import_items("from os import path, getcwd, listdir");
        assert_eq!(items, vec!["path", "getcwd", "listdir"]);
        
        let items = EntityExtractor::extract_import_items("import { a, b, c } from 'module'");
        assert_eq!(items, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_entity_type_from_node_kind() {
        assert_eq!(EntityType::from(NodeKind::Function), EntityType::Function);
        assert_eq!(EntityType::from(NodeKind::Class), EntityType::Class);
        assert_eq!(EntityType::from(NodeKind::Struct), EntityType::Struct);
    }
}
