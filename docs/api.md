# HTTP API Reference

## Endpoints

### Health Check

**GET `/health`**

Check if the service is running.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

---

### Start Chunking Job

**POST `/chunk/jobs`**

Start a new chunking job for a batch of source items.

**Request Body:**
```json
{
  "source_id": "uuid",
  "source_kind": "code_repo | document | chat | ticketing | wiki | email | web | other",
  "items": [
    {
      "id": "uuid",
      "source_id": "uuid",
      "source_kind": "code_repo",
      "content_type": "text/code:rust",
      "content": "fn main() { ... }",
      "metadata": {
        "path": "src/main.rs",
        "language": "rust",
        "repo": "org/repo",
        "branch": "main"
      },
      "created_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

**Response:**
```json
{
  "job_id": "uuid",
  "accepted": true,
  "items_count": 10,
  "message": null
}
```

**Status Codes:**
- `200` - Job created successfully
- `400` - Invalid request body

---

### Get Job Status

**GET `/chunk/jobs/{job_id}`**

Get the status of a chunking job.

**Response:**
```json
{
  "job_id": "uuid",
  "status": "pending | running | completed | failed",
  "total_items": 10,
  "processed_items": 5,
  "chunks_created": 47,
  "error": null,
  "started_at": "2024-01-01T00:00:00Z",
  "completed_at": null
}
```

**Status Codes:**
- `200` - Job found
- `404` - Job not found

---

### List Profiles

**GET `/chunk/profiles`**

List all available chunking profiles.

**Response:**
```json
[
  {
    "name": "default",
    "description": "Default balanced profile for general use",
    "chunk_size": 512,
    "chunk_overlap": 50,
    "active": true
  },
  {
    "name": "small",
    "description": "Smaller chunks for fine-grained retrieval",
    "chunk_size": 256,
    "chunk_overlap": 25,
    "active": false
  }
]
```

---

### Get Active Profile

**GET `/chunk/profiles/active`**

Get the currently active chunking profile.

**Response:**
```json
{
  "name": "default",
  "chunk_size": 512,
  "chunk_overlap": 50
}
```

---

### Set Active Profile

**PUT `/chunk/profiles/active`**

Set the active chunking profile.

**Request Body:**
```json
{
  "name": "small"
}
```

**Response:**
```json
{
  "name": "small",
  "chunk_size": 256,
  "chunk_overlap": 25
}
```

**Status Codes:**
- `200` - Profile activated
- `404` - Profile not found

---

## Data Types

### SourceKind

```
code_repo   - Source code repositories (GitHub, GitLab)
document    - Generic documents (PDF, Word, text files)
chat        - Chat/messaging (Slack, Teams, Discord)
ticketing   - Issue trackers (Jira, Linear, GitHub Issues)
wiki        - Wiki pages (Notion, Confluence)
email       - Email threads
web         - Web pages
other       - Other/unknown sources
```

### Content Types

The `content_type` field follows MIME type conventions with extensions:

```
text/code:rust          - Rust source code
text/code:python        - Python source code
text/code:javascript    - JavaScript source code
text/code:typescript    - TypeScript source code
text/markdown           - Markdown documents
text/plain              - Plain text
application/json        - JSON content (used for chat threads)
text/html               - HTML content
text/x-diff             - Diff/patch content
```

### ChunkJobStatus

```
pending    - Job created but not started
running    - Job is currently processing
completed  - Job finished successfully
failed     - Job encountered an error
```

---

## Example Payloads

### Code Repository File

```json
{
  "source_id": "11111111-1111-1111-1111-111111111111",
  "source_kind": "code_repo",
  "items": [
    {
      "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      "source_id": "11111111-1111-1111-1111-111111111111",
      "source_kind": "code_repo",
      "content_type": "text/code:typescript",
      "content": "export function add(a: number, b: number) { return a + b; }",
      "metadata": {
        "provider": "github",
        "repo": "acme/project",
        "branch": "main",
        "path": "src/utils/math.ts",
        "sha": "abc123",
        "language": "typescript"
      }
    }
  ]
}
```

### Slack Chat Thread

```json
{
  "source_id": "33333333-3333-3333-3333-333333333333",
  "source_kind": "chat",
  "items": [
    {
      "id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
      "source_id": "33333333-3333-3333-3333-333333333333",
      "source_kind": "chat",
      "content_type": "application/json",
      "content": "{\"channel\":\"#incidents\",\"messages\":[{\"user\":\"alice\",\"text\":\"API is down\"}]}",
      "metadata": {
        "provider": "slack",
        "channel_id": "C123",
        "channel_name": "incidents"
      }
    }
  ]
}
```

### Jira Ticket

```json
{
  "source_id": "44444444-4444-4444-4444-444444444444",
  "source_kind": "ticketing",
  "items": [
    {
      "id": "dddddddd-dddd-dddd-dddd-dddddddddddd",
      "source_id": "44444444-4444-4444-4444-444444444444",
      "source_kind": "ticketing",
      "content_type": "text/plain",
      "content": "Title: Fix OAuth bug\n\nDescription:\nThe refresh token is not being stored.\n\nComments:\n- Alice: observed 401 from GitHub",
      "metadata": {
        "provider": "jira",
        "issue_key": "PROJ-123",
        "status": "In Progress",
        "priority": "High"
      }
    }
  ]
}
```
