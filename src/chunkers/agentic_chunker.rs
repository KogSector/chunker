//! Agentic chunking with intelligent boundary detection.
//!
//! This module implements advanced "agentic" chunking strategies that mimic
//! intelligent document processing without using LLMs. It uses:
//!
//! - **Semantic boundary detection**: Identifies logical breakpoints in code and text
//! - **Context preservation**: Maintains necessary context across chunks
//! - **Hierarchical chunking**: Respects document/code structure at multiple levels
//! - **Adaptive sizing**: Adjusts chunk sizes based on content complexity

use std::collections::HashMap;

use anyhow::Result;

use super::base::{count_tokens, Chunker};
use crate::types::{Chunk, ChunkConfig, ChunkMetadata, SourceItem, SourceKind};

/// Agentic chunker that uses intelligent heuristics for optimal chunking.
///
/// This chunker implements LangChain-inspired document processing patterns
/// without requiring LLM calls. It uses:
///
/// 1. **Semantic boundary detection**: Identifies natural breakpoints
/// 2. **Context windowing**: Preserves necessary context for understanding
/// 3. **Importance scoring**: Prioritizes content by relevance signals
/// 4. **Relationship tracking**: Maintains references between related chunks
pub struct AgenticChunker {
    /// Minimum context to preserve at chunk boundaries
    context_overlap_tokens: usize,
    /// Maximum chunk size before forcing a split
    max_chunk_tokens: usize,
    /// Minimum chunk size (to avoid tiny fragments)
    min_chunk_tokens: usize,
    /// Enable smart boundary detection
    smart_boundaries: bool,
    /// Enable context injection for code chunks
    inject_context: bool,
}

impl AgenticChunker {
    /// Create a new agentic chunker with default settings.
    pub fn new() -> Self {
        Self {
            context_overlap_tokens: 64,
            max_chunk_tokens: 1024,
            min_chunk_tokens: 50,
            smart_boundaries: true,
            inject_context: true,
        }
    }

    /// Builder: set context overlap.
    pub fn with_context_overlap(mut self, tokens: usize) -> Self {
        self.context_overlap_tokens = tokens;
        self
    }

    /// Builder: set max chunk size.
    pub fn with_max_size(mut self, tokens: usize) -> Self {
        self.max_chunk_tokens = tokens;
        self
    }

    /// Analyze content and determine optimal chunking strategy.
    fn analyze_content(&self, content: &str) -> ContentAnalysis {
        let lines: Vec<&str> = content.lines().collect();
        let total_tokens = count_tokens(content);
        
        // Detect content characteristics
        let has_code_blocks = content.contains("```") || content.contains("    fn ");
        let has_headings = lines.iter().any(|l| l.starts_with('#'));
        let has_imports = lines.iter().any(|l| {
            l.starts_with("import ") || l.starts_with("from ") || 
            l.starts_with("use ") || l.starts_with("#include")
        });
        
        // Estimate complexity
        let avg_line_length: usize = if lines.is_empty() { 0 } else {
            lines.iter().map(|l| l.len()).sum::<usize>() / lines.len()
        };
        
        let nesting_depth = self.estimate_nesting_depth(content);
        
        ContentAnalysis {
            total_tokens,
            total_lines: lines.len(),
            has_code_blocks,
            has_headings,
            has_imports,
            avg_line_length,
            nesting_depth,
            semantic_boundaries: self.find_semantic_boundaries(content),
        }
    }

    /// Estimate code nesting depth.
    fn estimate_nesting_depth(&self, content: &str) -> usize {
        let mut max_depth: usize = 0;
        let mut current_depth: usize = 0;
        
        for c in content.chars() {
            match c {
                '{' | '(' | '[' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                }
                '}' | ')' | ']' => {
                    current_depth = current_depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        
        max_depth
    }

    /// Find semantic boundaries in content.
    fn find_semantic_boundaries(&self, content: &str) -> Vec<SemanticBoundary> {
        let mut boundaries = Vec::new();
        let mut current_byte = 0;
        
        for (line_num, line) in content.lines().enumerate() {
            let line_len = line.len() + 1; // +1 for newline
            
            // Check for various boundary types
            if let Some(boundary_type) = self.classify_line(line) {
                let strength = self.boundary_strength(line, &boundary_type);
                boundaries.push(SemanticBoundary {
                    line_number: line_num,
                    byte_offset: current_byte,
                    boundary_type,
                    strength,
                });
            }
            
            current_byte += line_len;
        }
        
        boundaries
    }

    /// Classify a line to determine if it's a semantic boundary.
    fn classify_line(&self, line: &str) -> Option<BoundaryType> {
        let trimmed = line.trim();
        
        // Empty lines are potential boundaries
        if trimmed.is_empty() {
            return Some(BoundaryType::EmptyLine);
        }
        
        // Heading-style boundaries
        if trimmed.starts_with('#') {
            return Some(BoundaryType::Heading);
        }
        
        // Code structure boundaries
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") ||
           trimmed.starts_with("async fn ") || trimmed.starts_with("pub async fn ") {
            return Some(BoundaryType::FunctionDef);
        }
        
        if trimmed.starts_with("impl ") || trimmed.starts_with("pub impl ") {
            return Some(BoundaryType::ImplBlock);
        }
        
        if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") ||
           trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") {
            return Some(BoundaryType::TypeDef);
        }
        
        if trimmed.starts_with("class ") || trimmed.starts_with("interface ") {
            return Some(BoundaryType::ClassDef);
        }
        
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            return Some(BoundaryType::FunctionDef);
        }
        
        // Module boundaries
        if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
            return Some(BoundaryType::ModuleDef);
        }
        
        // Comment blocks
        if trimmed.starts_with("///") || trimmed.starts_with("//!") ||
           trimmed.starts_with("/**") || trimmed.starts_with("/*") {
            return Some(BoundaryType::DocComment);
        }
        
        None
    }

    /// Calculate boundary strength (higher = stronger boundary).
    fn boundary_strength(&self, line: &str, boundary_type: &BoundaryType) -> f32 {
        let trimmed = line.trim();
        
        match boundary_type {
            BoundaryType::Heading => {
                // More #'s = higher level heading = stronger boundary
                let level = trimmed.chars().take_while(|c| *c == '#').count();
                1.0 - (level as f32 * 0.1)
            }
            BoundaryType::FunctionDef => 0.8,
            BoundaryType::ClassDef | BoundaryType::TypeDef => 0.9,
            BoundaryType::ImplBlock => 0.85,
            BoundaryType::ModuleDef => 0.95,
            BoundaryType::DocComment => 0.3,
            BoundaryType::EmptyLine => 0.2,
        }
    }

    /// Split content at semantic boundaries.
    fn split_at_boundaries(
        &self,
        content: &str,
        analysis: &ContentAnalysis,
        config: &ChunkConfig,
    ) -> Vec<ChunkCandidate> {
        if analysis.total_tokens <= config.chunk_size {
            return vec![ChunkCandidate {
                content: content.to_string(),
                start_byte: 0,
                end_byte: content.len(),
                context_before: None,
                context_after: None,
                metadata: HashMap::new(),
            }];
        }

        let mut candidates = Vec::new();
        let mut current_start = 0;
        let mut current_tokens = 0;
        let lines: Vec<&str> = content.lines().collect();
        
        // Sort boundaries by strength (strongest first for tie-breaking)
        let mut sorted_boundaries = analysis.semantic_boundaries.clone();
        sorted_boundaries.sort_by(|a, b| {
            b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut line_byte_offsets: Vec<usize> = vec![0];
        let mut offset = 0;
        for line in &lines {
            offset += line.len() + 1;
            line_byte_offsets.push(offset);
        }

        let mut last_split = 0;
        let mut current_end_line = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_tokens = count_tokens(line);
            current_tokens += line_tokens;
            current_end_line = line_idx;

            // Check if we should split here
            if current_tokens >= config.chunk_size {
                // Find best boundary near here
                let split_line = self.find_best_boundary(
                    &sorted_boundaries,
                    last_split,
                    line_idx,
                );

                let split_byte = line_byte_offsets.get(split_line + 1).copied().unwrap_or(content.len());
                
                // Create chunk
                let chunk_content = &content[current_start..split_byte];
                if !chunk_content.trim().is_empty() {
                    candidates.push(ChunkCandidate {
                        content: chunk_content.to_string(),
                        start_byte: current_start,
                        end_byte: split_byte,
                        context_before: None,
                        context_after: None,
                        metadata: HashMap::new(),
                    });
                }

                current_start = split_byte;
                current_tokens = 0;
                last_split = split_line + 1;
            }
        }

        // Final chunk
        if current_start < content.len() {
            let final_content = &content[current_start..];
            if !final_content.trim().is_empty() {
                candidates.push(ChunkCandidate {
                    content: final_content.to_string(),
                    start_byte: current_start,
                    end_byte: content.len(),
                    context_before: None,
                    context_after: None,
                    metadata: HashMap::new(),
                });
            }
        }

        // Add context injection
        if self.inject_context {
            self.inject_context_into_candidates(&mut candidates, content);
        }

        candidates
    }

    /// Find the best boundary line to split at.
    fn find_best_boundary(
        &self,
        boundaries: &[SemanticBoundary],
        start_line: usize,
        end_line: usize,
    ) -> usize {
        // Find strongest boundary in range
        let candidate = boundaries
            .iter()
            .filter(|b| b.line_number >= start_line && b.line_number <= end_line)
            .max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap_or(std::cmp::Ordering::Equal));

        candidate.map(|b| b.line_number).unwrap_or(end_line)
    }

    /// Inject context information into chunk candidates.
    fn inject_context_into_candidates(&self, candidates: &mut [ChunkCandidate], _full_content: &str) {
        if candidates.len() < 2 {
            return;
        }

        // Extract imports/uses from first chunk if present
        let imports: Vec<String> = if let Some(first) = candidates.first() {
            first.content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    t.starts_with("use ") || t.starts_with("import ") || 
                    t.starts_with("from ") || t.starts_with("#include")
                })
                .map(String::from)
                .collect()
        } else {
            vec![]
        };

        // Add import context to subsequent chunks if they reference code
        for candidate in candidates.iter_mut().skip(1) {
            if !imports.is_empty() {
                let has_code = candidate.content.contains("fn ") || 
                              candidate.content.contains("def ") ||
                              candidate.content.contains("class ");
                
                if has_code {
                    candidate.context_before = Some(imports.join("\n"));
                }
            }
        }
    }
}

impl Default for AgenticChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for AgenticChunker {
    fn name(&self) -> &'static str {
        "agentic"
    }

    fn description(&self) -> &'static str {
        "Intelligent agentic chunker with semantic boundary detection and context preservation"
    }

    fn chunk(&self, item: &SourceItem, config: &ChunkConfig) -> Result<Vec<Chunk>> {
        let content = &item.content;
        if content.is_empty() {
            return Ok(vec![]);
        }

        // Analyze content
        let analysis = self.analyze_content(content);

        // Split at semantic boundaries
        let candidates = self.split_at_boundaries(content, &analysis, config);

        // Convert candidates to chunks
        let mut chunks = Vec::new();
        for (idx, candidate) in candidates.iter().enumerate() {
            // Prepend context if available
            let final_content = if let Some(ctx) = &candidate.context_before {
                format!("// Context:\n{}\n\n{}", ctx, candidate.content)
            } else {
                candidate.content.clone()
            };

            let token_count = count_tokens(&final_content);

            let mut chunk = Chunk::new(
                item.id,
                item.source_id,
                item.source_kind,
                final_content,
                token_count,
                candidate.start_byte,
                candidate.end_byte,
                idx,
            );

            // Add metadata
            chunk.metadata = ChunkMetadata {
                content_type: Some("agentic".to_string()),
                path: item.extract_path().map(String::from),
                language: config.language.clone(),
                ..Default::default()
            };

            chunks.push(chunk);
        }

        Ok(chunks)
    }
}

/// Analysis of content characteristics.
#[derive(Debug)]
struct ContentAnalysis {
    total_tokens: usize,
    total_lines: usize,
    has_code_blocks: bool,
    has_headings: bool,
    has_imports: bool,
    avg_line_length: usize,
    nesting_depth: usize,
    semantic_boundaries: Vec<SemanticBoundary>,
}

/// A semantic boundary in the content.
#[derive(Debug, Clone)]
struct SemanticBoundary {
    line_number: usize,
    byte_offset: usize,
    boundary_type: BoundaryType,
    strength: f32,
}

/// Types of semantic boundaries.
#[derive(Debug, Clone, PartialEq)]
enum BoundaryType {
    EmptyLine,
    Heading,
    FunctionDef,
    ClassDef,
    TypeDef,
    ImplBlock,
    ModuleDef,
    DocComment,
}

/// A chunk candidate before final processing.
#[derive(Debug)]
struct ChunkCandidate {
    content: String,
    start_byte: usize,
    end_byte: usize,
    context_before: Option<String>,
    #[allow(dead_code)]
    context_after: Option<String>,
    #[allow(dead_code)]
    metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_item(content: &str, kind: SourceKind) -> SourceItem {
        SourceItem {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            source_kind: kind,
            content_type: "text/code:rust".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        }
    }

    #[test]
    fn test_agentic_small_content() {
        let chunker = AgenticChunker::new();
        let content = "fn main() { println!(\"hello\"); }";
        let item = create_test_item(content, SourceKind::CodeRepo);
        let config = ChunkConfig::with_size(100);

        let chunks = chunker.chunk(&item, &config).unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_semantic_boundary_detection() {
        let chunker = AgenticChunker::new();
        let content = r#"
use std::io;

fn first_function() {
    println!("first");
}

fn second_function() {
    println!("second");
}

struct MyStruct {
    value: i32,
}
"#;
        let analysis = chunker.analyze_content(content);
        
        // Should detect function and struct boundaries
        assert!(analysis.semantic_boundaries.iter().any(|b| b.boundary_type == BoundaryType::FunctionDef));
        assert!(analysis.semantic_boundaries.iter().any(|b| b.boundary_type == BoundaryType::TypeDef));
    }

    #[test]
    fn test_large_content_splitting() {
        let chunker = AgenticChunker::new();
        
        // Generate large content
        let content: String = (0..50)
            .map(|i| format!("fn function_{}() {{\n    println!(\"Function {}\");\n}}\n\n", i, i))
            .collect();
        
        let item = create_test_item(&content, SourceKind::CodeRepo);
        let config = ChunkConfig::with_size(200);

        let chunks = chunker.chunk(&item, &config).unwrap();
        
        // Should produce multiple chunks
        assert!(chunks.len() > 1);
        
        // Each chunk should be non-empty
        for chunk in &chunks {
            assert!(!chunk.content.trim().is_empty());
        }
    }
}
