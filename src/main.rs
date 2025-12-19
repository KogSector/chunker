//! Chunker Service - Main Entry Point
//!
//! A high-performance chunking service for RAG pipelines.

use anyhow::Result;
use axum::{
    routing::{get, post, put},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use chunker::api::handlers::{self, AppState};
use chunker::jobs::JobStore;
use chunker::router::ChunkingRouter;
use chunker::types::ChunkingConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chunker=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = ChunkingConfig::from_env();

    info!("Starting Chunker Service v{}", env!("CARGO_PKG_VERSION"));
    info!("Default chunk size: {} tokens", config.default_chunk_size);

    // Initialize components
    let router = ChunkingRouter::new(&config);
    let job_store = JobStore::new();

    let state = Arc::new(AppState {
        router,
        job_store: RwLock::new(job_store),
        config,
    });

    // Build HTTP routes
    let app = Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Chunking jobs
        .route("/chunk/jobs", post(handlers::start_chunk_job))
        .route("/chunk/jobs/:job_id", get(handlers::get_job_status))
        // Profiles
        .route("/chunk/profiles", get(handlers::list_profiles))
        .route("/chunk/profiles/active", get(handlers::get_active_profile))
        .route("/chunk/profiles/active", put(handlers::set_active_profile))
        // State
        .with_state(state)
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Start server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3017);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
