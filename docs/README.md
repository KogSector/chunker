# Chunker Service Documentation

A high-performance, production-ready chunking service for RAG (Retrieval-Augmented Generation) pipelines, written in Rust.

## Quick Start

### Installation

```bash
# Clone the repository
cd chunker

# Build the project
cargo build --release

# Run the service
cargo run --release
```

### Configuration

Set environment variables or create a `.env` file:

```bash
# Server
PORT=3017

# Chunking defaults
CHUNK_SIZE=512
CHUNK_OVERLAP=50
MIN_CHARS_PER_SENTENCE=12

# Downstream services (optional)
EMBEDDING_SERVICE_URL=http://localhost:3018
GRAPH_SERVICE_URL=http://localhost:3019

# Concurrency
MAX_CONCURRENT_JOBS=4

# Logging
RUST_LOG=chunker=info,tower_http=debug
```

### Basic Usage

Start a chunking job:

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
        "content": "fn main() { println!(\"Hello, world!\"); }",
        "metadata": {"path": "main.rs"}
      }
    ]
  }'
```

Check job status:

```bash
curl http://localhost:3017/chunk/jobs/{job_id}
```

## Documentation Index

- [API Reference](api.md) - Complete HTTP API documentation
- [Chunking Strategies](chunking-strategies.md) - Deep dive into each chunker
- [Integration Guide](integration.md) - How to integrate with your pipeline
- [Configuration Guide](configuration.md) - All configuration options

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                      Data Service                          │
│  (GitHub, Jira, Notion, Slack connectors)                  │
└─────────────────────────┬──────────────────────────────────┘
                          │ POST /chunk/jobs
                          ▼
┌────────────────────────────────────────────────────────────┐
│                    Chunker Service                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  HTTP API   │──│ Job Queue   │──│  Strategy Router    │ │
│  └─────────────┘  └─────────────┘  └──────────┬──────────┘ │
│                                                │            │
│  ┌─────────────────────────────────────────────┴──────────┐│
│  │                    Chunkers                             ││
│  │  ┌───────┐ ┌──────────┐ ┌───────────┐ ┌──────────────┐ ││
│  │  │ Token │ │ Sentence │ │ Recursive │ │    Code      │ ││
│  │  └───────┘ └──────────┘ └───────────┘ │ (tree-sitter)│ ││
│  │                                        └──────────────┘ ││
│  │  ┌──────────┐ ┌────────┐ ┌───────────────────────────┐ ││
│  │  │ Document │ │  Chat  │ │        Ticketing          │ ││
│  │  │(markdown)│ │(slack) │ │     (issues/PRs)          │ ││
│  │  └──────────┘ └────────┘ └───────────────────────────┘ ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────┬──────────────────────────────────┘
                          │ Chunks
                          ▼
┌────────────────────────────────────────────────────────────┐
│              Embedding Service / Graph Service              │
└────────────────────────────────────────────────────────────┘
```

## Supported Content Types

| Source Kind | Chunker Used | Description |
|-------------|--------------|-------------|
| `code_repo` | CodeChunker | AST-aware code chunking with tree-sitter |
| `document` | DocumentChunker | Heading-aware markdown/text chunking |
| `wiki` | DocumentChunker | Same as document |
| `chat` | ChatChunker | Conversation window chunking |
| `ticketing` | TicketingChunker | Issue/PR structure-aware chunking |
| `email` | ChatChunker | Thread-aware email chunking |
| `web` | RecursiveChunker | HTML-aware recursive chunking |
| `other` | SentenceChunker | Sentence-boundary chunking |

## Supported Programming Languages

The CodeChunker supports AST-aware chunking for:

- Rust
- Python
- JavaScript / JSX
- TypeScript / TSX
- Go
- Java
- C / C++
- Ruby

## License

MIT License
