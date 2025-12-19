# Configuration Guide

Complete reference for all configuration options in the Chunker Service.

## Environment Variables

### Server Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3017` | HTTP server port |
| `RUST_LOG` | `chunker=info` | Log level configuration |

### Chunking Defaults

| Variable | Default | Description |
|----------|---------|-------------|
| `CHUNK_SIZE` | `512` | Default maximum tokens per chunk |
| `CHUNK_OVERLAP` | `50` | Default token overlap between chunks |
| `MIN_CHARS_PER_SENTENCE` | `12` | Minimum characters to be a sentence |

### Downstream Services

| Variable | Default | Description |
|----------|---------|-------------|
| `EMBEDDING_SERVICE_URL` | *none* | URL of the embedding service |
| `GRAPH_SERVICE_URL` | *none* | URL of the graph service |

### Processing

| Variable | Default | Description |
|----------|---------|-------------|
| `MAX_CONCURRENT_JOBS` | `4` | Maximum jobs to process simultaneously |
| `ACTIVE_PROFILE` | `default` | Default chunking profile to use |

## Example .env File

```bash
# Server
PORT=3017
RUST_LOG=chunker=info,tower_http=debug

# Chunking defaults
CHUNK_SIZE=512
CHUNK_OVERLAP=50
MIN_CHARS_PER_SENTENCE=12

# Downstream services
EMBEDDING_SERVICE_URL=http://localhost:3018
GRAPH_SERVICE_URL=http://localhost:3019

# Processing
MAX_CONCURRENT_JOBS=4
ACTIVE_PROFILE=default
```

## Chunking Profiles

Profiles are pre-configured chunk settings for common use cases.

### Built-in Profiles

#### Default
```json
{
  "name": "default",
  "description": "Default balanced profile for general use",
  "chunk_size": 512,
  "chunk_overlap": 50
}
```

Best for: Most content types, balanced between context and granularity.

#### Small
```json
{
  "name": "small",
  "description": "Smaller chunks for fine-grained retrieval",
  "chunk_size": 256,
  "chunk_overlap": 25
}
```

Best for: When you need more precise retrieval, question-answering.

#### Large
```json
{
  "name": "large",
  "description": "Larger chunks for more context",
  "chunk_size": 1024,
  "chunk_overlap": 100
}
```

Best for: When more context is important, summarization tasks.

#### Code
```json
{
  "name": "code",
  "description": "Optimized for code with function-aware splitting",
  "chunk_size": 768,
  "chunk_overlap": 64
}
```

Best for: Source code repositories, larger function tolerance.

### Selecting a Profile

Use the profile API to change the active profile:

```bash
# Get current profile
curl http://localhost:3017/chunk/profiles/active

# Set new profile
curl -X PUT http://localhost:3017/chunk/profiles/active \
  -H "Content-Type: application/json" \
  -d '{"name": "small"}'
```

## Chunk Size Guidelines

### By Use Case

| Use Case | Recommended Size | Overlap |
|----------|------------------|---------|
| FAQ/Q&A | 256 | 25 |
| Code search | 512-768 | 50-64 |
| Documentation | 512 | 50 |
| Chat logs | 256 | 25 |
| Long documents | 1024 | 100 |

### By Embedding Model

Different embedding models have different context windows:

| Model | Max Tokens | Recommended Chunk Size |
|-------|------------|------------------------|
| text-embedding-ada-002 | 8191 | 512-1024 |
| bge-large | 512 | 384-512 |
| sentence-transformers | 256-512 | 256 |
| voyage-02 | 4000 | 512-1024 |

## Content Type Mapping

The service automatically selects chunkers based on content type:

| Content Type | Chunker |
|--------------|---------|
| `text/code:*` | CodeChunker |
| `text/markdown` | DocumentChunker |
| `text/x-markdown` | DocumentChunker |
| `text/html` | RecursiveChunker |
| `application/json` + chat | ChatChunker |
| `text/plain` | SentenceChunker |

Override by explicitly setting `source_kind` in your request.

## Logging Configuration

Use `RUST_LOG` for fine-grained logging control:

```bash
# All chunker logs at debug level
RUST_LOG=chunker=debug

# Only job processing logs
RUST_LOG=chunker::jobs=debug

# HTTP traces
RUST_LOG=chunker=info,tower_http=trace

# Specific chunkers
RUST_LOG=chunker::chunkers::code=debug

# Multiple targets
RUST_LOG=chunker=info,tower_http=debug,tree_sitter=warn
```

### Log Levels

- `error` - Only errors
- `warn` - Warnings and errors
- `info` - General operation info (default)
- `debug` - Detailed operation logs
- `trace` - Very verbose, including request/response bodies

## Performance Tuning

### Memory Usage

Large files can consume significant memory. Consider:

1. **Batch size**: Process fewer items per request
2. **Chunk size**: Larger chunks = fewer allocations
3. **Concurrent jobs**: Lower `MAX_CONCURRENT_JOBS` for memory-constrained systems

### CPU Usage

Tree-sitter parsing is CPU-intensive:

1. Code files take longer than plain text
2. Large files with complex ASTs are expensive
3. Consider separate workers for code vs. documents

### Network

If using downstream services:

1. Batch chunks to embedding service (default: 50 per request)
2. Use connection pooling (handled by reqwest)
3. Set appropriate timeouts

## Docker Configuration

```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/chunker /usr/local/bin/
EXPOSE 3017
CMD ["chunker"]
```

```yaml
# docker-compose.yml
version: '3.8'
services:
  chunker:
    build: .
    ports:
      - "3017:3017"
    environment:
      - RUST_LOG=chunker=info
      - CHUNK_SIZE=512
      - EMBEDDING_SERVICE_URL=http://embedding:3018
    depends_on:
      - embedding
```

## Health Checks

For orchestration systems:

```yaml
# Kubernetes
livenessProbe:
  httpGet:
    path: /health
    port: 3017
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health
    port: 3017
  initialDelaySeconds: 2
  periodSeconds: 5
```

```yaml
# Docker Compose
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:3017/health"]
  interval: 10s
  timeout: 5s
  retries: 3
```
