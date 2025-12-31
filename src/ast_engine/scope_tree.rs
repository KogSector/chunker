//! Scope tree construction for hierarchical code structure.
//!
//! Builds a tree representing the scope hierarchy of code entities,
//! useful for context enrichment and understanding code organization.

use std::collections::HashMap;

use crate::ast_engine::entity_extractor::{CodeEntity, EntityType};
use crate::ast_engine::parser::{AstNode, NodeKind};

/// A node in the scope tree.
#[derive(Debug, Clone)]
pub struct ScopeNode {
    /// Name of this scope.
    pub name: String,
    /// Type of entity that defines this scope.
    pub scope_type: ScopeType,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// Child scope names.
    pub children: Vec<String>,
    /// Full path from root (e.g., "module.Class.method").
    pub full_path: String,
}

/// Types of scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeType {
    Module,
    Class,
    Function,
    Method,
    Block,
}

impl From<NodeKind> for ScopeType {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Module => ScopeType::Module,
            NodeKind::Class | NodeKind::Struct | NodeKind::Interface | NodeKind::Trait | NodeKind::Enum => {
                ScopeType::Class
            }
            NodeKind::Function => ScopeType::Function,
            NodeKind::Method => ScopeType::Method,
            NodeKind::Block => ScopeType::Block,
            _ => ScopeType::Block,
        }
    }
}

impl From<EntityType> for ScopeType {
    fn from(entity_type: EntityType) -> Self {
        match entity_type {
            EntityType::Module => ScopeType::Module,
            EntityType::Class | EntityType::Struct | EntityType::Interface | EntityType::Trait | EntityType::Enum => {
                ScopeType::Class
            }
            EntityType::Function => ScopeType::Function,
            EntityType::Method => ScopeType::Method,
            _ => ScopeType::Block,
        }
    }
}

/// Hierarchical scope tree for a source file.
#[derive(Debug)]
pub struct ScopeTree {
    /// Root scope name (usually the module/file name).
    pub root_scope: String,
    /// Map from scope path to child scope names.
    pub scopes: HashMap<String, Vec<String>>,
    /// Map from scope path to scope node.
    pub scope_nodes: HashMap<String, ScopeNode>,
}

impl ScopeTree {
    /// Build a scope tree from a list of AST nodes.
    pub fn from_nodes(nodes: &[AstNode], root_name: &str) -> Self {
        let mut tree = Self {
            root_scope: root_name.to_string(),
            scopes: HashMap::new(),
            scope_nodes: HashMap::new(),
        };

        // Initialize root scope
        tree.scopes.insert(root_name.to_string(), Vec::new());
        tree.scope_nodes.insert(
            root_name.to_string(),
            ScopeNode {
                name: root_name.to_string(),
                scope_type: ScopeType::Module,
                start_line: 1,
                end_line: usize::MAX,
                children: Vec::new(),
                full_path: root_name.to_string(),
            },
        );

        // Process nodes to build tree
        tree.build_from_nodes(nodes, root_name);

        tree
    }

    /// Build a scope tree from code entities.
    pub fn from_entities(entities: &[CodeEntity], root_name: &str) -> Self {
        let mut tree = Self {
            root_scope: root_name.to_string(),
            scopes: HashMap::new(),
            scope_nodes: HashMap::new(),
        };

        // Initialize root scope
        tree.scopes.insert(root_name.to_string(), Vec::new());
        tree.scope_nodes.insert(
            root_name.to_string(),
            ScopeNode {
                name: root_name.to_string(),
                scope_type: ScopeType::Module,
                start_line: 1,
                end_line: usize::MAX,
                children: Vec::new(),
                full_path: root_name.to_string(),
            },
        );

        // Process entities to build tree
        for entity in entities {
            if entity.is_definition() {
                tree.add_entity(entity, root_name);
            }
        }

        tree
    }

    /// Build the tree from AST nodes.
    fn build_from_nodes(&mut self, nodes: &[AstNode], root_name: &str) {
        // Stack of (scope_path, end_line) for tracking current scope
        let mut scope_stack: Vec<(String, usize)> = vec![(root_name.to_string(), usize::MAX)];

        for node in nodes {
            // Pop scopes that have ended
            while scope_stack.len() > 1 {
                let (_, end_line) = scope_stack.last().unwrap();
                if node.start_line > *end_line {
                    scope_stack.pop();
                } else {
                    break;
                }
            }

            // Check if this node creates a new scope
            if Self::creates_scope(node.kind) {
                if let Some(name) = &node.name {
                    let parent_path = &scope_stack.last().unwrap().0;
                    let full_path = format!("{}.{}", parent_path, name);

                    // Add to parent's children
                    if let Some(parent_children) = self.scopes.get_mut(parent_path) {
                        parent_children.push(name.clone());
                    }

                    // Also update parent node's children
                    if let Some(parent_node) = self.scope_nodes.get_mut(parent_path) {
                        parent_node.children.push(name.clone());
                    }

                    // Create new scope
                    self.scopes.insert(full_path.clone(), Vec::new());
                    self.scope_nodes.insert(
                        full_path.clone(),
                        ScopeNode {
                            name: name.clone(),
                            scope_type: ScopeType::from(node.kind),
                            start_line: node.start_line,
                            end_line: node.end_line,
                            children: Vec::new(),
                            full_path: full_path.clone(),
                        },
                    );

                    // Push onto stack
                    scope_stack.push((full_path, node.end_line));
                }
            }
        }
    }

    /// Add an entity to the tree.
    fn add_entity(&mut self, entity: &CodeEntity, root_name: &str) {
        // Determine parent scope from entity's scope path
        let parts: Vec<&str> = entity.scope_path.split('.').collect();
        let (parent_path, name) = if parts.len() > 1 {
            let parent = parts[..parts.len() - 1].join(".");
            let name = parts[parts.len() - 1].to_string();
            (format!("{}.{}", root_name, parent), name)
        } else {
            (root_name.to_string(), entity.name.clone())
        };

        let full_path = format!("{}.{}", parent_path, name);

        // Ensure parent exists
        if !self.scopes.contains_key(&parent_path) {
            self.scopes.insert(parent_path.clone(), Vec::new());
        }

        // Add to parent's children
        if let Some(parent_children) = self.scopes.get_mut(&parent_path) {
            if !parent_children.contains(&name) {
                parent_children.push(name.clone());
            }
        }

        // Create scope node
        if !self.scope_nodes.contains_key(&full_path) {
            self.scope_nodes.insert(
                full_path.clone(),
                ScopeNode {
                    name: name.clone(),
                    scope_type: ScopeType::from(entity.entity_type),
                    start_line: entity.start_line,
                    end_line: entity.end_line,
                    children: Vec::new(),
                    full_path,
                },
            );
        }
    }

    /// Check if a node kind creates a new scope.
    fn creates_scope(kind: NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::Class
                | NodeKind::Struct
                | NodeKind::Interface
                | NodeKind::Trait
                | NodeKind::Enum
                | NodeKind::Module
                | NodeKind::Impl
                | NodeKind::Function
                | NodeKind::Method
        )
    }

    /// Get the scope containing a given line.
    pub fn get_scope_at_line(&self, line: usize) -> Option<&ScopeNode> {
        let mut best_match: Option<&ScopeNode> = None;
        let mut best_span = usize::MAX;

        for node in self.scope_nodes.values() {
            if line >= node.start_line && line <= node.end_line {
                let span = node.end_line - node.start_line;
                if span < best_span {
                    best_span = span;
                    best_match = Some(node);
                }
            }
        }

        best_match
    }

    /// Get the full scope path for a line.
    pub fn get_scope_path_at_line(&self, line: usize) -> Option<String> {
        self.get_scope_at_line(line).map(|n| n.full_path.clone())
    }

    /// Get all child scopes of a given scope.
    pub fn get_children(&self, scope_path: &str) -> Vec<&ScopeNode> {
        self.scopes
            .get(scope_path)
            .map(|children| {
                children
                    .iter()
                    .filter_map(|name| {
                        let child_path = format!("{}.{}", scope_path, name);
                        self.scope_nodes.get(&child_path)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the parent scope of a given scope.
    pub fn get_parent(&self, scope_path: &str) -> Option<&ScopeNode> {
        let parts: Vec<&str> = scope_path.rsplitn(2, '.').collect();
        if parts.len() == 2 {
            self.scope_nodes.get(parts[1])
        } else {
            None
        }
    }

    /// Get all scopes as a flat list.
    pub fn all_scopes(&self) -> Vec<&ScopeNode> {
        self.scope_nodes.values().collect()
    }

    /// Get the depth of the scope tree.
    pub fn depth(&self) -> usize {
        self.scope_nodes
            .keys()
            .map(|path| path.matches('.').count() + 1)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(
        kind: NodeKind,
        name: &str,
        start_line: usize,
        end_line: usize,
    ) -> AstNode {
        AstNode {
            kind,
            name: Some(name.to_string()),
            start_byte: 0,
            end_byte: 100,
            start_line,
            end_line,
            start_col: 0,
            end_col: 0,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_build_scope_tree() {
        let nodes = vec![
            create_test_node(NodeKind::Class, "MyClass", 1, 20),
            create_test_node(NodeKind::Method, "method1", 2, 5),
            create_test_node(NodeKind::Method, "method2", 6, 10),
            create_test_node(NodeKind::Function, "helper", 22, 30),
        ];

        let tree = ScopeTree::from_nodes(&nodes, "module");

        // Root should have MyClass and helper as children
        let root_children = tree.scopes.get("module").unwrap();
        assert!(root_children.contains(&"MyClass".to_string()));
        assert!(root_children.contains(&"helper".to_string()));

        // MyClass should have methods as children
        let class_children = tree.scopes.get("module.MyClass").unwrap();
        assert!(class_children.contains(&"method1".to_string()));
        assert!(class_children.contains(&"method2".to_string()));
    }

    #[test]
    fn test_get_scope_at_line() {
        let nodes = vec![
            create_test_node(NodeKind::Class, "MyClass", 1, 20),
            create_test_node(NodeKind::Method, "method1", 2, 5),
        ];

        let tree = ScopeTree::from_nodes(&nodes, "module");

        // Line 3 should be in method1
        let scope = tree.get_scope_at_line(3).unwrap();
        assert_eq!(scope.name, "method1");

        // Line 15 should be in MyClass (but not in a method)
        let scope = tree.get_scope_at_line(15).unwrap();
        assert_eq!(scope.name, "MyClass");
    }

    #[test]
    fn test_scope_type_conversion() {
        assert_eq!(ScopeType::from(NodeKind::Class), ScopeType::Class);
        assert_eq!(ScopeType::from(NodeKind::Function), ScopeType::Function);
        assert_eq!(ScopeType::from(NodeKind::Method), ScopeType::Method);
        assert_eq!(ScopeType::from(NodeKind::Module), ScopeType::Module);
    }
}
