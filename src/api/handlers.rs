//! HTTP request handlers for the chunking service.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::jobs::{JobProcessor, JobStore};
use crate::output::EmbeddingClient;
use crate::router::ChunkingRouter;
use crate::types::{
    ChunkingConfig, ChunkingProfile, StartChunkJobRequest, StartChunkJobResponse,
};

/// Application state shared across handlers.
pub struct AppState {
    pub router: ChunkingRouter,
    pub job_store: RwLock<JobStore>,
    pub config: ChunkingConfig,
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

/// Health check endpoint.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Start a chunking job.
pub async fn start_chunk_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartChunkJobRequest>,
) -> Result<Json<StartChunkJobResponse>, StatusCode> {
    let items_count = request.items.len();

    if items_count == 0 {
        return Ok(Json(StartChunkJobResponse {
            job_id: Uuid::nil(),
            accepted: false,
            items_count: 0,
            message: Some("No items provided".to_string()),
        }));
    }

    info!(
        source_id = %request.source_id,
        source_kind = %request.source_kind,
        items = items_count,
        "Received chunk job request"
    );

    // Create job
    let job_id = {
        let mut store = state.job_store.write().await;
        store.create_job(items_count)
    };

    // Create processor
    let embedding_client = state.config.embedding_service_url.as_ref().map(|url| {
        Arc::new(EmbeddingClient::new(url))
    });

    let router = Arc::new(ChunkingRouter::new(&state.config));
    let processor = JobProcessor::new(router, embedding_client);

    // Create a new job store for background processing
    // In production, you would share the actual state
    let background_store = Arc::new(RwLock::new(JobStore::new()));
    
    // Mark job as created in background store
    {
        let mut store = background_store.write().await;
        store.create_job(items_count);
    }

    // Spawn job processing
    tokio::spawn(async move {
        processor.process_job(job_id, request, background_store).await;
    });

    Ok(Json(StartChunkJobResponse {
        job_id,
        accepted: true,
        items_count,
        message: None,
    }))
}

/// Get job status.
pub async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.job_store.read().await;

    match store.get_job_status(job_id) {
        Some(status) => Ok(Json(status)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// List available profiles.
pub async fn list_profiles() -> Json<Vec<ChunkingProfile>> {
    Json(ChunkingProfile::defaults())
}

/// Get active profile response.
#[derive(Debug, Serialize)]
pub struct ActiveProfileResponse {
    name: String,
    chunk_size: usize,
    chunk_overlap: usize,
}

/// Get active profile.
pub async fn get_active_profile(
    State(state): State<Arc<AppState>>,
) -> Json<ActiveProfileResponse> {
    Json(ActiveProfileResponse {
        name: state.config.active_profile.clone(),
        chunk_size: state.config.default_chunk_size,
        chunk_overlap: state.config.default_chunk_overlap,
    })
}

/// Set active profile request.
#[derive(Debug, Deserialize)]
pub struct SetActiveProfileRequest {
    name: String,
}

/// Set active profile.
pub async fn set_active_profile(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<SetActiveProfileRequest>,
) -> Result<Json<ActiveProfileResponse>, StatusCode> {
    // Find the profile
    let profiles = ChunkingProfile::defaults();
    let profile = profiles
        .into_iter()
        .find(|p| p.name == request.name);

    match profile {
        Some(p) => Ok(Json(ActiveProfileResponse {
            name: p.name,
            chunk_size: p.chunk_size,
            chunk_overlap: p.chunk_overlap,
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// List available chunkers.
#[derive(Debug, Serialize)]
pub struct ChunkerInfo {
    name: String,
    description: String,
}

pub async fn list_chunkers(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ChunkerInfo>> {
    let chunkers: Vec<ChunkerInfo> = state
        .router
        .list_chunkers()
        .into_iter()
        .map(|(name, desc)| ChunkerInfo {
            name: name.to_string(),
            description: desc.to_string(),
        })
        .collect();

    Json(chunkers)
}
