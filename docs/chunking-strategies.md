# Chunking Strategies

This document provides a deep dive into each chunking strategy available in the service.

## Overview

The chunker service provides 7 specialized chunking strategies, each optimized for different content types:

| Chunker | Best For | Key Feature |
|---------|----------|-------------|
| TokenChunker | Any text | Fixed-size token chunks |
| SentenceChunker | Prose/articles | Respects sentence boundaries |
| RecursiveChunker | Structured text | Hierarchical splitting |
| CodeChunker | Source code | AST-aware semantic chunks |
| DocumentChunker | Markdown/wiki | Heading-aware splitting |
| ChatChunker | Conversations | Message window grouping |
| TicketingChunker | Issues/PRs | Structure-aware splitting |

---

## TokenChunker

**Use when:** You need fast, predictable chunking without semantic considerations.

The TokenChunker splits text into chunks of a fixed token size. It's the simplest and fastest chunker, ideal for:
- Baseline performance testing
- Content where structure doesn't matter
- Fallback for unknown content types

### How it works

1. Tokenize the entire text using tiktoken (cl100k_base encoding)
2. Split tokens into groups of `chunk_size` tokens
3. Optionally overlap by `chunk_overlap` tokens

### Configuration

```rust
ChunkConfig {
    chunk_size: 512,      // Max tokens per chunk
    chunk_overlap: 50,    // Tokens shared between chunks
}
```

### Example

Input:
```
The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs.
```

With `chunk_size: 10, chunk_overlap: 2`:
```
Chunk 0: "The quick brown fox jumps over the lazy dog."
Chunk 1: "lazy dog. Pack my box with five dozen"
Chunk 2: "five dozen liquor jugs."
```

---

## SentenceChunker

**Use when:** You have prose, articles, or documentation where sentence boundaries matter.

The SentenceChunker respects sentence boundaries, ensuring chunks don't cut sentences in half.

### How it works

1. Split text at sentence delimiters (`. `, `! `, `? `, `\n`)
2. Merge short sentences to meet minimum character requirements
3. Group sentences into chunks that fit within token limits
4. Include sentence-ending delimiters with the sentence

### Configuration

```rust
ChunkConfig {
    chunk_size: 512,
    min_chars_per_sentence: 12,  // Minimum chars to be a sentence
}
```

### Example

Input:
```
Hello. This is a test. Short. Sentences are great. They make for better chunks.
```

Output:
```
Chunk 0: "Hello. This is a test. Short."
Chunk 1: "Sentences are great. They make for better chunks."
```

---

## RecursiveChunker

**Use when:** You have structured text with paragraphs, sections, or hierarchical organization.

The RecursiveChunker tries multiple splitting strategies in order of preference, recursively splitting large pieces with finer-grained strategies.

### Splitting Hierarchy

1. `\n\n` - Paragraph boundaries
2. `\n` - Line boundaries
3. `. ` - Sentence boundaries
4. `! ` - Exclamation boundaries
5. `? ` - Question boundaries
6. `; ` - Semicolon boundaries
7. `, ` - Comma boundaries
8. ` ` - Word boundaries
9. Characters (last resort)

### How it works

1. Try to split at the most preferred level (paragraphs)
2. If any chunk is still too large, recursively split it with the next level
3. Merge small adjacent pieces when possible

### Example

Input:
```
# Introduction

This is the intro paragraph.

# Details

This section has more details.
And multiple lines.
```

With paragraph splitting first:
```
Chunk 0: "# Introduction\n\nThis is the intro paragraph."
Chunk 1: "# Details\n\nThis section has more details.\nAnd multiple lines."
```

---

## CodeChunker

**Use when:** You have source code and want semantically meaningful chunks.

The CodeChunker uses tree-sitter for AST-aware chunking, ensuring chunks align with code structure like functions, classes, and methods.

### Supported Languages

| Language | Extensions | Node Types |
|----------|------------|------------|
| Rust | `.rs` | function_item, impl_item, struct_item, enum_item, trait_item |
| Python | `.py` | function_definition, class_definition, decorated_definition |
| JavaScript | `.js`, `.jsx` | function_declaration, class_declaration, arrow_function |
| TypeScript | `.ts`, `.tsx` | function_declaration, interface_declaration, type_alias_declaration |
| Go | `.go` | function_declaration, method_declaration, type_declaration |
| Java | `.java` | class_declaration, method_declaration, interface_declaration |
| C/C++ | `.c`, `.cpp` | function_definition, struct_specifier, class_specifier |
| Ruby | `.rb` | method, class, module |

### How it works

1. Parse code into AST using tree-sitter
2. Identify semantic units (functions, classes, methods)
3. Group related units that fit within token limits
4. Preserve comments attached to their target code

### Metadata

Code chunks include rich metadata:

```json
{
  "language": "rust",
  "path": "src/lib.rs",
  "symbol_name": "process_data",
  "parent_symbol": "DataProcessor",
  "line_range": [45, 67]
}
```

### Example

Input:
```rust
/// Process the input data
fn process(data: &str) -> Result<String> {
    // ... implementation
}

/// Validate the input
fn validate(input: &str) -> bool {
    // ... implementation
}
```

Output chunks will keep functions together with their doc comments:
```
Chunk 0: "/// Process the input data\nfn process(data: &str) -> Result<String> { ... }"
Chunk 1: "/// Validate the input\nfn validate(input: &str) -> bool { ... }"
```

---

## DocumentChunker

**Use when:** You have markdown, wiki pages, or documentation with headings.

The DocumentChunker understands markdown structure and preserves section hierarchy.

### How it works

1. Split document at heading boundaries (`#`, `##`, `###`, etc.)
2. Preserve heading with its content
3. Split large sections by paragraphs
4. Handle code blocks as atomic units

### Heading Preservation

Each chunk retains context about its document location:

```json
{
  "section": "Getting Started",
  "path": "docs/README.md"
}
```

### Example

Input:
```markdown
# Introduction

Welcome to the project.

## Installation

Run `npm install` to get started.

## Usage

Import the module and call `init()`.
```

Output:
```
Chunk 0: "# Introduction\n\nWelcome to the project."
Chunk 1: "## Installation\n\nRun `npm install` to get started."
Chunk 2: "## Usage\n\nImport the module and call `init()`."
```

---

## ChatChunker

**Use when:** You have conversation threads from Slack, Discord, Teams, etc.

The ChatChunker groups messages into conversation windows while preserving speaker context.

### Supported Formats

**JSON format:**
```json
{
  "channel": "#general",
  "messages": [
    {"user": "alice", "text": "Hello!", "ts": "1234567890"}
  ]
}
```

**Text format:**
```
alice: Hello!
bob: Hi there!
```

### How it works

1. Parse messages from JSON or text format
2. Group messages by conversation window
3. Respect token limits while keeping conversations coherent
4. Preserve speaker names and timestamps

### Metadata

```json
{
  "author": "alice",
  "thread_id": "1234567890",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## TicketingChunker

**Use when:** You have issues, PRs, Jira tickets, or similar structured content.

The TicketingChunker understands ticket structure and separates different parts appropriately.

### Supported Structure

- **Title/Summary** - The ticket headline
- **Description** - Main body content
- **Comments** - Discussion thread
- **Metadata** - Status, priority, assignee, etc.

### How it works

1. Parse ticket structure (JSON or text format)
2. Create separate chunks for description and comments
3. Include metadata header for context
4. Handle inline code specially

### Metadata

```json
{
  "content_type": "description | comment",
  "author": "alice"
}
```

### Example

Input:
```
Title: Fix login bug
Status: Open
Priority: High

Description:
Users cannot log in on mobile devices.

Comments:
- Alice: Can reproduce on iOS
- Bob: Fixed in PR #123
```

Output:
```
Chunk 0 (description): "# Fix login bug\n**Status**: Open\n**Priority**: High\n\n## Description\n\nUsers cannot log in on mobile devices."
Chunk 1 (comments): "## Comments\n\n**Alice**: Can reproduce on iOS\n\n---\n\n**Bob**: Fixed in PR #123"
```

---

## Choosing the Right Chunker

```
                    ┌─────────────────────────────────────┐
                    │       What type of content?         │
                    └────────────────┬────────────────────┘
                                     │
        ┌────────────┬───────────────┼───────────────┬────────────┐
        ▼            ▼               ▼               ▼            ▼
   ┌─────────┐  ┌─────────┐    ┌─────────┐    ┌─────────┐   ┌──────────┐
   │  Code   │  │  Docs   │    │  Chat   │    │ Tickets │   │  Other   │
   └────┬────┘  └────┬────┘    └────┬────┘    └────┬────┘   └────┬─────┘
        │            │              │              │              │
        ▼            ▼              ▼              ▼              ▼
   CodeChunker  DocumentChunker ChatChunker TicketingChunker SentenceChunker
```

The service automatically selects the appropriate chunker based on `source_kind` and `content_type`, but you can override this by specifying a chunker explicitly.
