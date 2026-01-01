# Chunker Service

High-performance, production-ready chunking service for RAG pipelines. Supports multiple content types including code, documents, chat, and tickets.

## Purpose

The chunker is the **text segmentation** step in the knowledge pipeline:

```
code-normalize-fetch → chunker → embeddings → relation-graph
data-connector       ↗
```

It intelligently splits content into semantic chunks optimized for embedding and retrieval.

## Features

- **Multiple Chunkers**: Code, document, chat, ticket-optimized strategies
- **Entity-Aware**: Uses entity boundaries from code-normalize-fetch
- **Context Enrichment**: Adds file/scope context prefixes for better embeddings
- **Configurable**: Profiles for different chunk sizes and overlaps
- **Batch Processing**: Efficient handling of large content sets

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     CHUNKER                               │
│                   Port: 3002                              │
├──────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│  │   Router    │  │  Chunkers   │  │ Enrichment  │       │
│  │             │  │             │  │             │       │
│  │ • Route by  │  │ • Code      │  │ • Context   │       │
│  │   source    │  │ • Document  │  │   Builder   │       │
│  │   type      │  │ • Chat      │  │ • Prefixes  │       │
│  │             │  │ • Agentic   │  │             │       │
│  └─────────────┘  └─────────────┘  └─────────────┘       │
│                                                           │
└──────────────────────────────────────────────────────────┘
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/chunk` | POST | Chunk content items |
| `/chunk/batch` | POST | Batch chunking |
| `/profiles` | GET | List chunking profiles |

## Chunking Profiles

| Profile | Chunk Size | Overlap | Use Case |
|---------|------------|---------|----------|
| `default` | 512 | 50 | General purpose |
| `small` | 256 | 25 | Fine-grained retrieval |
| `large` | 1024 | 100 | More context |
| `code` | 768 | 64 | Code-optimized |

## Request Format

```json
POST /chunk
{
  "source_id": "uuid",
  "source_kind": "code_repo",
  "items": [
    {
      "id": "uuid",
      "content_type": "text/code:python",
      "content": "def hello():\n    print('Hello')",
      "metadata": {
        "path": "src/main.py",
        "language": "python"
      }
    }
  ],
  "profile": "code"
}
```

## Code Chunking with Entities

When receiving normalized code from code-normalize-fetch, use entity boundaries:

```json
{
  "item": {...},
  "entities": [
    {
      "name": "hello",
      "entity_type": "function",
      "start_line": 1,
      "end_line": 2
    }
  ]
}
```

## Quick Start

```bash
# Build
cargo build --release

# Run
cargo run

# Test
cargo test
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3002` | Server port |
| `CHUNK_SIZE` | `512` | Default chunk size |
| `CHUNK_OVERLAP` | `50` | Default overlap |
| `EMBEDDING_SERVICE_URL` | - | Embeddings service |

## License

MIT
