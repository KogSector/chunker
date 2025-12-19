# Integration Guide

This guide explains how to integrate the Chunker Service with your data pipeline and downstream services.

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                        Data Service                           │
│                                                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐   │
│  │   GitHub    │  │    Jira     │  │    Notion/Slack     │   │
│  │  Connector  │  │  Connector  │  │     Connectors      │   │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘   │
│         │                │                     │              │
│         └────────────────┼─────────────────────┘              │
│                          │                                    │
│                          ▼                                    │
│         ┌────────────────────────────────────────┐            │
│         │     Normalize to SourceItem            │            │
│         │  (id, source_id, source_kind,          │            │
│         │   content_type, content, metadata)     │            │
│         └────────────────────┬───────────────────┘            │
└──────────────────────────────┼────────────────────────────────┘
                               │
                               │ POST /chunk/jobs
                               ▼
┌──────────────────────────────────────────────────────────────┐
│                      Chunker Service                          │
│                                                               │
│  1. Route to appropriate chunker based on source_kind        │
│  2. Split content into semantic chunks                        │
│  3. Add metadata (line numbers, symbols, sections)            │
│  4. Send chunks to downstream services                        │
│                                                               │
└──────────────────────────────┬────────────────────────────────┘
                               │
               ┌───────────────┼───────────────┐
               ▼               ▼               ▼
        ┌──────────┐    ┌──────────┐    ┌──────────┐
        │ Embedding│    │  Graph   │    │  Vector  │
        │ Service  │    │ Service  │    │   DB     │
        └──────────┘    └──────────┘    └──────────┘
```

## Data Service Integration

### Step 1: Normalize Content to SourceItem

When your data service fetches content from various sources, normalize it to the `SourceItem` format:

```rust
struct SourceItem {
    id: Uuid,              // Stable ID for this item
    source_id: Uuid,       // Connected account/integration ID
    source_kind: SourceKind,
    content_type: String,  // MIME type with extensions
    content: String,       // Raw text content
    metadata: Value,       // Source-specific metadata
    created_at: Option<DateTime<Utc>>,
}
```

### Step 2: Determine Source Kind

Map your content source to the appropriate `SourceKind`:

| Source | SourceKind | Content Type | Notes |
|--------|------------|--------------|-------|
| GitHub code files | `code_repo` | `text/code:{lang}` | Set language in content_type |
| GitHub issues | `ticketing` | `text/markdown` | Include comments in content |
| GitHub PRs | `ticketing` | `text/markdown` | Include reviews and comments |
| Jira tickets | `ticketing` | `text/plain` | Format as structured text |
| Notion pages | `wiki` | `text/markdown` | Export as markdown |
| Confluence | `wiki` | `text/markdown` | Export as markdown |
| Slack threads | `chat` | `application/json` | Use JSON format for messages |
| Teams messages | `chat` | `application/json` | Use JSON format for messages |
| Email threads | `email` | `text/plain` | Format as conversation |
| Web pages | `web` | `text/html` | Raw HTML or extracted text |

### Step 3: Call the Chunker Service

```rust
// Example using reqwest
async fn send_to_chunker(items: Vec<SourceItem>) -> Result<Uuid> {
    let request = StartChunkJobRequest {
        source_id: items[0].source_id,
        source_kind: items[0].source_kind,
        items,
    };

    let response = client
        .post("http://localhost:3017/chunk/jobs")
        .json(&request)
        .send()
        .await?;

    let result: StartChunkJobResponse = response.json().await?;
    Ok(result.job_id)
}
```

### Step 4: Poll for Completion (Optional)

```rust
async fn wait_for_job(job_id: Uuid) -> Result<ChunkJobStatusResponse> {
    loop {
        let response = client
            .get(&format!("http://localhost:3017/chunk/jobs/{}", job_id))
            .send()
            .await?;

        let status: ChunkJobStatusResponse = response.json().await?;

        match status.status {
            ChunkJobStatus::Completed => return Ok(status),
            ChunkJobStatus::Failed => {
                return Err(anyhow!("Job failed: {:?}", status.error));
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}
```

## Embedding Service Integration

### Chunk Output Format

The Chunker Service sends chunks to the embedding service in this format:

```json
{
  "chunks": [
    {
      "id": "uuid",
      "source_item_id": "uuid",
      "source_id": "uuid",
      "content": "The actual chunk text...",
      "metadata": {
        "language": "rust",
        "path": "src/main.rs",
        "line_range": [10, 25]
      }
    }
  ]
}
```

### Expected Endpoints

Your embedding service should implement:

**POST `/embed/chunks`**
```json
// Request
{
  "chunks": [
    {"id": "...", "content": "...", "metadata": {...}}
  ]
}

// Response
{
  "embedded_count": 10,
  "errors": []
}
```

**GET `/health`**
```json
{"status": "healthy"}
```

### Configuration

```bash
EMBEDDING_SERVICE_URL=http://localhost:3018
```

## GitHub Integration Example

### Code Files

```rust
fn normalize_github_file(file: GitHubFile) -> SourceItem {
    let language = detect_language(&file.path);
    
    SourceItem {
        id: Uuid::new_v5(&NAMESPACE, &format!("{}:{}", repo, file.path)),
        source_id: repo_source_id,
        source_kind: SourceKind::CodeRepo,
        content_type: format!("text/code:{}", language),
        content: file.content,
        metadata: json!({
            "provider": "github",
            "repo": repo_full_name,
            "branch": branch,
            "path": file.path,
            "sha": file.sha,
            "language": language
        }),
        created_at: None,
    }
}
```

### Issues

```rust
fn normalize_github_issue(issue: GitHubIssue, comments: Vec<GitHubComment>) -> SourceItem {
    let content = format!(
        "Title: {}\nState: {}\n\nDescription:\n{}\n\nComments:\n{}",
        issue.title,
        issue.state,
        issue.body,
        comments.iter()
            .map(|c| format!("- {}: {}", c.user.login, c.body))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SourceItem {
        id: Uuid::new_v5(&NAMESPACE, &format!("issue:{}", issue.id)),
        source_id: repo_source_id,
        source_kind: SourceKind::Ticketing,
        content_type: "text/markdown".to_string(),
        content,
        metadata: json!({
            "provider": "github",
            "repo": repo_full_name,
            "issue_number": issue.number,
            "state": issue.state,
            "author": issue.user.login,
            "url": issue.html_url
        }),
        created_at: Some(issue.created_at),
    }
}
```

### Pull Requests

```rust
fn normalize_github_pr(pr: GitHubPR, reviews: Vec<Review>) -> SourceItem {
    let content = format!(
        "# PR #{}: {}\n\n## Description\n{}\n\n## Reviews\n{}",
        pr.number,
        pr.title,
        pr.body,
        reviews.iter()
            .map(|r| format!("**{}** ({}): {}", r.user.login, r.state, r.body))
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    SourceItem {
        id: Uuid::new_v5(&NAMESPACE, &format!("pr:{}", pr.id)),
        source_id: repo_source_id,
        source_kind: SourceKind::Ticketing,
        content_type: "text/markdown".to_string(),
        content,
        metadata: json!({
            "provider": "github",
            "repo": repo_full_name,
            "pr_number": pr.number,
            "state": pr.state,
            "author": pr.user.login,
            "base_branch": pr.base.ref_name,
            "head_branch": pr.head.ref_name,
            "url": pr.html_url
        }),
        created_at: Some(pr.created_at),
    }
}
```

## Slack Integration Example

```rust
fn normalize_slack_thread(channel: &str, messages: Vec<SlackMessage>) -> SourceItem {
    let thread = json!({
        "channel": channel,
        "thread_ts": messages[0].ts,
        "messages": messages.iter().map(|m| json!({
            "user": m.user,
            "text": m.text,
            "ts": m.ts
        })).collect::<Vec<_>>()
    });

    SourceItem {
        id: Uuid::new_v5(&NAMESPACE, &format!("slack:{}:{}", channel, messages[0].ts)),
        source_id: workspace_source_id,
        source_kind: SourceKind::Chat,
        content_type: "application/json".to_string(),
        content: serde_json::to_string(&thread).unwrap(),
        metadata: json!({
            "provider": "slack",
            "workspace_id": workspace_id,
            "channel_id": channel_id,
            "channel_name": channel
        }),
        created_at: Some(parse_slack_ts(messages[0].ts)),
    }
}
```

## Jira Integration Example

```rust
fn normalize_jira_issue(issue: JiraIssue) -> SourceItem {
    let content = format!(
        "Title: {}\nKey: {}\nStatus: {}\nPriority: {}\n\nDescription:\n{}\n\nComments:\n{}",
        issue.fields.summary,
        issue.key,
        issue.fields.status.name,
        issue.fields.priority.name,
        issue.fields.description.unwrap_or_default(),
        issue.fields.comment.comments.iter()
            .map(|c| format!("- {}: {}", c.author.display_name, c.body))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SourceItem {
        id: Uuid::new_v5(&NAMESPACE, &format!("jira:{}", issue.id)),
        source_id: jira_source_id,
        source_kind: SourceKind::Ticketing,
        content_type: "text/plain".to_string(),
        content,
        metadata: json!({
            "provider": "jira",
            "cloud_id": cloud_id,
            "project_key": issue.fields.project.key,
            "issue_key": issue.key,
            "issue_type": issue.fields.issuetype.name,
            "status": issue.fields.status.name,
            "priority": issue.fields.priority.name,
            "assignee": issue.fields.assignee.map(|a| a.display_name),
            "reporter": issue.fields.reporter.display_name,
            "url": format!("{}/browse/{}", jira_base_url, issue.key)
        }),
        created_at: Some(issue.fields.created),
    }
}
```

## Error Handling

### Graceful Degradation

The chunker service is designed to continue processing even when individual items fail:

1. If parsing fails for a code file, it falls back to line-based chunking
2. If a single item in a batch fails, other items still get processed
3. Errors are logged but don't stop the job

### Retry Logic

Implement retry logic in your data service:

```rust
async fn send_with_retry(items: Vec<SourceItem>, max_retries: u32) -> Result<Uuid> {
    for attempt in 0..max_retries {
        match send_to_chunker(items.clone()).await {
            Ok(job_id) => return Ok(job_id),
            Err(e) if attempt < max_retries - 1 => {
                tracing::warn!("Chunker request failed, retrying: {}", e);
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

## Performance Tips

1. **Batch items** - Send multiple items in a single request (up to 100)
2. **Use async** - Don't wait for job completion synchronously
3. **Set appropriate chunk sizes** - Larger chunks = fewer chunks = faster processing
4. **Monitor job status** - Use the job status endpoint for long-running batches
