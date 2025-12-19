# 🦛 Chunker Service

A **production-ready, high-performance chunking service** written in Rust for RAG (Retrieval-Augmented Generation) pipelines.

Built with inspiration from the best chunking libraries: [Chonkie](https://github.com/chonkie-ai/chonkie), [LlamaIndex](https://github.com/run-llama/llama_index), and [Code-Chunker](https://github.com/code-chunker/code-chunker).

---

## ✨ Features

- **10 Specialized Chunkers** - Token, Sentence, Recursive, Code (AST), Document, Chat, Ticketing, Table, + **Agentic**
- **Agentic Chunking** - Intelligent semantic boundary detection without LLMs (LangChain-inspired)
- **AST-Aware Code Chunking** - Uses tree-sitter for 9+ languages (Rust, Python, JS, TS, Go, Java, C/C++, Ruby)
- **Repository-Scale Symbol Extraction** - Extract functions, classes, structs across entire codebases
- **Production Scale** - Handles 1000s of files and 100+ page documents
- **Streaming Support** - Memory-efficient processing for large datasets
- **Embedding-Ready** - Output chunks ready for embedding services
- **Flexible API** - HTTP service with job queue or library interface

---

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/KogSector/chunker.git
cd chunker

# Build the project
cargo build --release

# Run the service
cargo run --release
```

### Environment Configuration

Create a `.env` file or set environment variables:

```bash
# Server
PORT=3017

# Chunking defaults
CHUNK_SIZE=512
CHUNK_OVERLAP=50

# Downstream services (optional)
EMBEDDING_SERVICE_URL=http://localhost:3018

# Logging
RUST_LOG=chunker=info
```

### Basic Usage

**Start a chunking job:**

```bash
curl -X POST http://localhost:3017/chunk/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "11111111-1111-1111-1111-111111111111",
    "source_kind": "code_repo",
    "items": [
      {
        "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "source_id": "11111111-1111-1111-1111-111111111111",
        "source_kind": "code_repo",
        "content_type": "text/code:rust",
        "content": "fn main() {\n    println!(\"Hello, world!\");\n}",
        "metadata": {"path": "main.rs"}
      }
    ]
  }'
```

**Check job status:**

```bash
curl http://localhost:3017/chunk/jobs/{job_id}
```

---

## 📦 Chunking Strategies

| Chunker | Best For | Key Feature |
|---------|----------|-------------|
| **TokenChunker** | Any text (fallback) | Fixed-size token chunks with overlap |
| **SentenceChunker** | Prose, articles | Respects sentence boundaries |
| **RecursiveChunker** | Structured text | Hierarchical multi-level splitting |
| **CodeChunker** | Source code | tree-sitter AST-aware semantic chunks |
| **DocumentChunker** | Markdown, wiki | Heading-aware section splitting |
| **ChatChunker** | Slack, Teams, Discord | Conversation window grouping |
| **TicketingChunker** | Jira, GitHub Issues, PRs | Issue structure-aware splitting |
| **TableChunker** | Tables (md/CSV) | Header preservation in each chunk |
| **AgenticChunker** | Complex code/docs | Intelligent semantic boundary detection |

### Automatic Routing

The service automatically selects the best chunker based on `source_kind`:

| Source Kind | Default Chunker |
|-------------|-----------------|
| `code_repo` | CodeChunker |
| `document` | DocumentChunker |
| `wiki` | DocumentChunker |
| `chat` | ChatChunker |
| `ticketing` | TicketingChunker |
| `email` | ChatChunker |
| `web` | RecursiveChunker |
| `other` | SentenceChunker |

### Using the Agentic Chunker

The AgenticChunker provides intelligent chunking by:
- Detecting semantic boundaries (functions, classes, headings)
- Preserving context across chunks (imports, type definitions)
- Scoring boundary strength for optimal split points

```rust
use chunker::{AgenticChunker, Chunker};

let chunker = AgenticChunker::new()
    .with_context_overlap(64)
    .with_max_size(1024);
    
let chunks = chunker.chunk(&item, &config)?;
```

---

## 🔧 Supported Languages (AST-Aware)

The CodeChunker uses tree-sitter for accurate semantic chunking:

| Language | Extensions | Chunk Points |
|----------|------------|--------------|
| Rust | `.rs` | functions, impls, structs, enums, traits |
| Python | `.py` | functions, classes, decorators |
| JavaScript | `.js`, `.jsx` | functions, classes, arrow functions |
| TypeScript | `.ts`, `.tsx` | functions, classes, interfaces, types |
| Go | `.go` | functions, methods, types |
| Java | `.java` | classes, methods, interfaces |
| C/C++ | `.c`, `.cpp` | functions, structs, classes |
| Ruby | `.rb` | methods, classes, modules |

---

## 📊 Scaling

### Repository-Wide Symbol Extraction

Extract and track symbols across entire repositories:

```rust
use chunker::{RepositoryContext, extract_symbols, Symbol};

// Extract symbols from a file
let symbols = extract_symbols(content, Some("rust"));

// Track symbols across files
let mut ctx = RepositoryContext::new();
for (path, content) in files {
    for symbol in extract_symbols(&content, detect_language(&path).as_deref()) {
        ctx.register_symbol(&path, symbol);
    }
}

// Find where a symbol is defined
let locations = ctx.find_symbol_locations("process_data");
```

**Supported Symbol Types:**
- Functions, Methods
- Classes, Structs, Enums
- Interfaces, Traits, Types
- Modules

### Batch Processing

The batch processor handles large repositories efficiently:

```rust
use chunker::{BatchProcessor, BatchConfig, ChunkingRouter};

let router = Arc::new(ChunkingRouter::default());
let processor = BatchProcessor::new(router, BatchConfig {
    concurrency: 4,
    buffer_size: 100,
    continue_on_error: true,
    max_content_size: 10 * 1024 * 1024, // 10MB
});

let (chunks, result) = processor.process_batch(items, &config).await?;
println!("Processed {} items → {} chunks", result.processed_items, result.total_chunks);
```

### Large Documents

Documents over 10MB are automatically split at paragraph boundaries before chunking. The batch processor:

- Streams chunks to avoid memory bloat
- Maintains chunk indices relative to original content
- Continues processing on individual failures

---

## 🔌 API Reference

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/chunk/jobs` | Start chunking job |
| GET | `/chunk/jobs/{job_id}` | Get job status |
| GET | `/chunk/profiles` | List profiles |
| GET | `/chunk/profiles/active` | Get active profile |
| PUT | `/chunk/profiles/active` | Set active profile |

### Input Format (SourceItem)

```json
{
  "id": "uuid",
  "source_id": "uuid",
  "source_kind": "code_repo | document | chat | ticketing | wiki | email | web | other",
  "content_type": "text/code:rust | text/markdown | application/json | text/plain",
  "content": "... content to chunk ...",
  "metadata": { "path": "...", "language": "..." }
}
```

### Output Format (Chunk)

```json
{
  "id": "uuid",
  "source_item_id": "uuid",
  "source_id": "uuid",
  "source_kind": "code_repo",
  "content": "fn main() { ... }",
  "token_count": 45,
  "start_index": 0,
  "end_index": 120,
  "chunk_index": 0,
  "metadata": {
    "language": "rust",
    "path": "src/main.rs",
    "symbol_name": "main",
    "line_range": [1, 15]
  }
}
```

---

## 🔗 Embedding Integration

### Protocol

The chunker sends chunks to the embedding service via HTTP:

```
POST {EMBEDDING_SERVICE_URL}/embed/chunks
```

**Request:**
```json
{
  "chunks": [
    { "id": "...", "content": "...", "metadata": {...} }
  ]
}
```

**Response:**
```json
{
  "embedded_count": 100,
  "errors": []
}
```

### Required Chunk Fields

| Field | Required | Purpose |
|-------|----------|---------|
| `id` | ✓ | Unique identifier |
| `content` | ✓ | Text to embed |
| `source_id` | ✓ | Links to source |
| `metadata` | ✓ | Filtering/retrieval |
| `token_count` | ○ | Batching optimization |

---

## ⚙️ Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3017` | HTTP server port |
| `CHUNK_SIZE` | `512` | Max tokens per chunk |
| `CHUNK_OVERLAP` | `50` | Token overlap between chunks |
| `MIN_CHARS_PER_SENTENCE` | `12` | Min chars for sentence |
| `EMBEDDING_SERVICE_URL` | - | Downstream embedding service |
| `MAX_CONCURRENT_JOBS` | `4` | Parallel job limit |
| `RUST_LOG` | `chunker=info` | Log level |

### Chunking Profiles

| Profile | Chunk Size | Overlap | Use Case |
|---------|------------|---------|----------|
| `default` | 512 | 50 | General purpose |
| `small` | 256 | 25 | Fine-grained retrieval |
| `large` | 1024 | 100 | More context |
| `code` | 768 | 64 | Source code |

---

## 📁 Project Structure

```
chunker/
├── Cargo.toml
├── README.md
├── .env.example
├── config/
│   └── default.toml
├── docs/
│   ├── README.md
│   ├── api.md
│   ├── chunking-strategies.md
│   ├── configuration.md
│   └── integration.md
└── src/
    ├── main.rs           # HTTP server entry point
    ├── lib.rs            # Library exports
    ├── batch.rs          # Batch processing
    ├── api/              # HTTP handlers
    ├── chunkers/         # All chunking strategies
    ├── jobs/             # Job queue & processing
    ├── output/           # Embedding client
    ├── router/           # Strategy routing
    └── types/            # Core types
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test

# Run specific test
cargo test test_rust_function_chunking
```

---

## 📈 Benchmarks

| Content Type | Size | Time | Chunks |
|--------------|------|------|--------|
| Code (Rust) | 1MB | ~50ms | ~200 |
| Markdown | 1MB | ~30ms | ~150 |
| Chat (JSON) | 1MB | ~25ms | ~100 |

---

## 📄 License

MIT License

---

## 🙏 Acknowledgments

This project was built with inspiration from:

- [Chonkie](https://github.com/chonkie-ai/chonkie) - Python chunking library with semantic, neural, and code chunkers
- [LlamaIndex](https://github.com/run-llama/llama_index) - Enterprise RAG framework
- [tree-sitter](https://tree-sitter.github.io/) - Incremental parsing library
