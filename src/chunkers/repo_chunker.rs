//! Repository-scale code chunking with enhanced tree-sitter integration.
//!
//! This module provides advanced code chunking capabilities for processing
//! entire repositories efficiently. It includes:
//!
//! - **Multi-file context awareness**: Tracks relationships across files
//! - **Symbol graph building**: Understands imports, references, and dependencies
//! - **Adaptive chunking**: Adjusts strategy based on file size and complexity
//! - **Parallel processing**: Efficient handling of large codebases

use std::collections::HashMap;

use anyhow::Result;

use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem, SourceKind};

/// Repository-wide chunking context for tracking cross-file relationships.
#[derive(Debug, Default)]
pub struct RepositoryContext {
    /// Map of file path -> defined symbols
    pub symbols: HashMap<String, Vec<Symbol>>,
    /// Map of file path -> imported symbols
    pub imports: HashMap<String, Vec<Import>>,
    /// Map of symbol name -> defining file paths
    pub symbol_locations: HashMap<String, Vec<String>>,
    /// Total files processed
    pub files_processed: usize,
    /// Total chunks created
    pub chunks_created: usize,
}

impl RepositoryContext {
    /// Create a new empty repository context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a symbol defined in a file.
    pub fn register_symbol(&mut self, file_path: &str, symbol: Symbol) {
        self.symbol_locations
            .entry(symbol.name.clone())
            .or_default()
            .push(file_path.to_string());
        
        self.symbols
            .entry(file_path.to_string())
            .or_default()
            .push(symbol);
    }

    /// Register an import in a file.
    pub fn register_import(&mut self, file_path: &str, import: Import) {
        self.imports
            .entry(file_path.to_string())
            .or_default()
            .push(import);
    }

    /// Find files that define a given symbol.
    pub fn find_symbol_locations(&self, symbol_name: &str) -> Vec<&str> {
        self.symbol_locations
            .get(symbol_name)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get all symbols defined in a file.
    pub fn get_file_symbols(&self, file_path: &str) -> &[Symbol] {
        self.symbols.get(file_path).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// A symbol extracted from code.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name (function, class, struct, etc.)
    pub name: String,
    /// Symbol type
    pub symbol_type: SymbolType,
    /// Byte range in file
    pub byte_range: (usize, usize),
    /// Line range in file
    pub line_range: (usize, usize),
    /// Parent symbol (e.g., class for a method)
    pub parent: Option<String>,
    /// Documentation if present
    pub documentation: Option<String>,
}

/// Types of code symbols.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
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
    Type,
}

/// An import statement from code.
#[derive(Debug, Clone)]
pub struct Import {
    /// The module/package being imported
    pub module_path: String,
    /// Specific symbols imported (if any)
    pub symbols: Vec<String>,
    /// Whether this is a wildcard import
    pub is_wildcard: bool,
}

/// Strategy for handling large files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LargeFileStrategy {
    /// Split by semantic units (functions, classes)
    SplitBySymbols,
    /// Split by approximate token count
    SplitByTokens,
    /// Split by lines
    SplitByLines,
    /// Use hierarchical splitting (try symbols, fall back to lines)
    Hierarchical,
}

/// Configuration for repository-scale chunking.
#[derive(Debug, Clone)]
pub struct RepoChunkConfig {
    /// Maximum tokens per chunk
    pub max_chunk_tokens: usize,
    /// Minimum tokens per chunk
    pub min_chunk_tokens: usize,
    /// Strategy for large files
    pub large_file_strategy: LargeFileStrategy,
    /// Threshold (bytes) for considering a file "large"
    pub large_file_threshold: usize,
    /// Whether to include context from imports
    pub include_import_context: bool,
    /// Whether to include surrounding context for symbols
    pub include_symbol_context: bool,
    /// Lines of context before/after symbols
    pub symbol_context_lines: usize,
}

impl Default for RepoChunkConfig {
    fn default() -> Self {
        Self {
            max_chunk_tokens: 1024,
            min_chunk_tokens: 50,
            large_file_strategy: LargeFileStrategy::Hierarchical,
            large_file_threshold: 100 * 1024, // 100KB
            include_import_context: true,
            include_symbol_context: true,
            symbol_context_lines: 2,
        }
    }
}

/// Extract symbols from Rust code without tree-sitter (regex-based fallback).
pub fn extract_rust_symbols(content: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut current_parent: Option<String> = None;
    
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        
        // Track impl blocks for method parents
        if trimmed.starts_with("impl ") {
            if let Some(name) = extract_impl_name(trimmed) {
                current_parent = Some(name);
            }
        } else if trimmed == "}" && current_parent.is_some() {
            current_parent = None;
        }
        
        // Extract function symbols
        if let Some(name) = extract_function_name(trimmed) {
            let sym_type = if current_parent.is_some() {
                SymbolType::Method
            } else {
                SymbolType::Function
            };
            
            symbols.push(Symbol {
                name,
                symbol_type: sym_type,
                byte_range: (0, 0), // Would need proper byte tracking
                line_range: (line_num, line_num),
                parent: current_parent.clone(),
                documentation: None,
            });
        }
        
        // Extract struct/enum symbols
        if let Some((name, sym_type)) = extract_type_def(trimmed) {
            symbols.push(Symbol {
                name,
                symbol_type: sym_type,
                byte_range: (0, 0),
                line_range: (line_num, line_num),
                parent: None,
                documentation: None,
            });
        }
    }
    
    symbols
}

fn extract_function_name(line: &str) -> Option<String> {
    let patterns = [
        "pub async fn ", "async fn ", "pub fn ", "fn ",
        "pub const fn ", "const fn ", "pub unsafe fn ", "unsafe fn ",
    ];
    
    for pattern in patterns {
        if line.starts_with(pattern) {
            let rest = &line[pattern.len()..];
            let name = rest.split(|c: char| c == '(' || c == '<' || c.is_whitespace())
                .next()?
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn extract_impl_name(line: &str) -> Option<String> {
    // Handle "impl Trait for Type" and "impl Type"
    let rest = line.strip_prefix("impl ")?;
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '<');
    
    // Skip trait bounds
    let name_part = if rest.contains(" for ") {
        rest.split(" for ").nth(1)?
    } else {
        rest
    };
    
    let name = name_part
        .split(|c: char| c == '<' || c == '{' || c.is_whitespace())
        .next()?
        .to_string();
    
    if !name.is_empty() { Some(name) } else { None }
}

fn extract_type_def(line: &str) -> Option<(String, SymbolType)> {
    let patterns = [
        ("pub struct ", SymbolType::Struct),
        ("struct ", SymbolType::Struct),
        ("pub enum ", SymbolType::Enum),
        ("enum ", SymbolType::Enum),
        ("pub trait ", SymbolType::Trait),
        ("trait ", SymbolType::Trait),
        ("pub type ", SymbolType::Type),
        ("type ", SymbolType::Type),
    ];
    
    for (pattern, sym_type) in patterns {
        if line.starts_with(pattern) {
            let rest = &line[pattern.len()..];
            let name = rest.split(|c: char| c == '<' || c == '{' || c == '(' || c.is_whitespace())
                .next()?
                .to_string();
            if !name.is_empty() {
                return Some((name, sym_type));
            }
        }
    }
    None
}

/// Extract symbols from Python code.
pub fn extract_python_symbols(content: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut current_class: Option<String> = None;
    let mut class_indent = 0;
    
    for (line_num, line) in content.lines().enumerate() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        
        // Track class scope
        if trimmed.starts_with("class ") {
            if let Some(name) = extract_python_class_name(trimmed) {
                current_class = Some(name.clone());
                class_indent = indent;
                symbols.push(Symbol {
                    name,
                    symbol_type: SymbolType::Class,
                    byte_range: (0, 0),
                    line_range: (line_num, line_num),
                    parent: None,
                    documentation: None,
                });
            }
        } else if current_class.is_some() && indent <= class_indent && !trimmed.is_empty() {
            current_class = None;
        }
        
        // Extract function/method definitions
        if let Some(name) = extract_python_function_name(trimmed) {
            let sym_type = if current_class.is_some() {
                SymbolType::Method
            } else {
                SymbolType::Function
            };
            
            symbols.push(Symbol {
                name,
                symbol_type: sym_type,
                byte_range: (0, 0),
                line_range: (line_num, line_num),
                parent: current_class.clone(),
                documentation: None,
            });
        }
    }
    
    symbols
}

fn extract_python_class_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("class ")?;
    let name = rest.split(|c: char| c == '(' || c == ':' || c.is_whitespace())
        .next()?
        .to_string();
    if !name.is_empty() { Some(name) } else { None }
}

fn extract_python_function_name(line: &str) -> Option<String> {
    let patterns = ["async def ", "def "];
    
    for pattern in patterns {
        if line.starts_with(pattern) {
            let rest = &line[pattern.len()..];
            let name = rest.split(|c: char| c == '(' || c.is_whitespace())
                .next()?
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Extract symbols from JavaScript/TypeScript code.
pub fn extract_js_symbols(content: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut current_class: Option<String> = None;
    
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        
        // Class definitions
        if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
            if let Some(name) = extract_js_class_name(trimmed) {
                current_class = Some(name.clone());
                symbols.push(Symbol {
                    name,
                    symbol_type: SymbolType::Class,
                    byte_range: (0, 0),
                    line_range: (line_num, line_num),
                    parent: None,
                    documentation: None,
                });
            }
        }
        
        // Function definitions
        if let Some(name) = extract_js_function_name(trimmed) {
            let sym_type = if current_class.is_some() {
                SymbolType::Method
            } else {
                SymbolType::Function
            };
            
            symbols.push(Symbol {
                name,
                symbol_type: sym_type,
                byte_range: (0, 0),
                line_range: (line_num, line_num),
                parent: current_class.clone(),
                documentation: None,
            });
        }
        
        // Interface/type definitions (TypeScript)
        if let Some(name) = extract_ts_interface(trimmed) {
            symbols.push(Symbol {
                name,
                symbol_type: SymbolType::Interface,
                byte_range: (0, 0),
                line_range: (line_num, line_num),
                parent: None,
                documentation: None,
            });
        }
        
        // End of class block (simple heuristic)
        if trimmed == "}" && current_class.is_some() {
            current_class = None;
        }
    }
    
    symbols
}

fn extract_js_class_name(line: &str) -> Option<String> {
    let rest = if line.starts_with("export ") {
        line.strip_prefix("export class ")?
    } else {
        line.strip_prefix("class ")?
    };
    
    let name = rest.split(|c: char| c == '{' || c == ' ' || c == '<')
        .next()?
        .trim()
        .to_string();
    if !name.is_empty() { Some(name) } else { None }
}

fn extract_js_function_name(line: &str) -> Option<String> {
    // Arrow functions with const/let
    for kw in ["const ", "let ", "var "] {
        if line.starts_with(kw) && line.contains("=>") {
            let rest = &line[kw.len()..];
            let name = rest.split(|c: char| c == ' ' || c == '=' || c == ':')
                .next()?
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    
    // Regular function definitions
    let patterns = [
        "export async function ",
        "export function ",
        "async function ",
        "function ",
    ];
    
    for pattern in patterns {
        if line.starts_with(pattern) {
            let rest = &line[pattern.len()..];
            let name = rest.split(|c: char| c == '(' || c == '<' || c.is_whitespace())
                .next()?
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    
    None
}

fn extract_ts_interface(line: &str) -> Option<String> {
    let patterns = ["export interface ", "interface ", "export type ", "type "];
    
    for pattern in patterns {
        if line.starts_with(pattern) {
            let rest = &line[pattern.len()..];
            let name = rest.split(|c: char| c == '<' || c == '{' || c == '=' || c.is_whitespace())
                .next()?
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Extract symbols based on detected language.
pub fn extract_symbols(content: &str, language: Option<&str>) -> Vec<Symbol> {
    match language {
        Some("rust") => extract_rust_symbols(content),
        Some("python") => extract_python_symbols(content),
        Some("javascript") | Some("typescript") | Some("jsx") | Some("tsx") => {
            extract_js_symbols(content)
        }
        _ => {
            // Try to detect language from content
            if content.contains("fn ") && content.contains("->") {
                extract_rust_symbols(content)
            } else if content.contains("def ") && content.contains("self") {
                extract_python_symbols(content)
            } else if content.contains("function") || content.contains("=>") {
                extract_js_symbols(content)
            } else {
                vec![]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_symbols() {
        let content = r#"
use std::io;

pub struct MyStruct {
    value: i32,
}

impl MyStruct {
    pub fn new() -> Self {
        Self { value: 0 }
    }
    
    fn private_method(&self) {}
}

pub fn standalone_function() {
    println!("hello");
}

pub enum MyEnum {
    Variant1,
    Variant2,
}
"#;
        let symbols = extract_rust_symbols(content);
        
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyStruct"));
        assert!(names.contains(&"new"));
        assert!(names.contains(&"private_method"));
        assert!(names.contains(&"standalone_function"));
        assert!(names.contains(&"MyEnum"));
    }

    #[test]
    fn test_extract_python_symbols() {
        let content = r#"
import os

class MyClass:
    def __init__(self):
        self.value = 0
    
    def method(self):
        pass

def standalone_function():
    print("hello")

async def async_function():
    pass
"#;
        let symbols = extract_python_symbols(content);
        
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"__init__"));
        assert!(names.contains(&"method"));
        assert!(names.contains(&"standalone_function"));
        assert!(names.contains(&"async_function"));
    }

    #[test]
    fn test_extract_js_symbols() {
        let content = r#"
import { foo } from 'bar';

class MyClass {
    constructor() {
        this.value = 0;
    }
    
    method() {
        return this.value;
    }
}

function standaloneFunction() {
    console.log("hello");
}

const arrowFunc = () => {
    return 42;
};

export interface MyInterface {
    field: string;
}
"#;
        let symbols = extract_js_symbols(content);
        
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"standaloneFunction"));
        assert!(names.contains(&"arrowFunc"));
        assert!(names.contains(&"MyInterface"));
    }

    #[test]
    fn test_repository_context() {
        let mut ctx = RepositoryContext::new();
        
        ctx.register_symbol("src/main.rs", Symbol {
            name: "main".to_string(),
            symbol_type: SymbolType::Function,
            byte_range: (0, 100),
            line_range: (1, 10),
            parent: None,
            documentation: None,
        });
        
        ctx.register_symbol("src/lib.rs", Symbol {
            name: "process".to_string(),
            symbol_type: SymbolType::Function,
            byte_range: (0, 50),
            line_range: (1, 5),
            parent: None,
            documentation: None,
        });
        
        assert_eq!(ctx.find_symbol_locations("main"), vec!["src/main.rs"]);
        assert_eq!(ctx.get_file_symbols("src/lib.rs").len(), 1);
    }
}
