//! OpenAI-compatible HTTP server for Markus
//!
//! Exposes:
//!   POST /v1/chat/completions   — streaming + non-streaming
//!   GET  /v1/models             — model list
//!   GET  /health                — health check
//!   GET  /metrics               — basic token stats

pub mod routes;
pub mod state;
pub mod types;

use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;

use axum::{Router, routing::{get, post}};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing::info;

use markus_core::{GenerationPipeline, MarkusConfig};
use crate::state::AppState;

pub struct Server {
    config: MarkusConfig,
    model_path: PathBuf,
    bind_addr: SocketAddr,
}

impl Server {
    pub fn new(model_path: PathBuf, config: MarkusConfig) -> Self {
        let addr = format!("{}:{}", config.server_host, config.server_port)
            .parse()
            .expect("Invalid server address");
        Self { config, model_path, bind_addr: addr }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        info!("Loading model for server: {}", self.model_path.display());

        let pipeline = GenerationPipeline::load(&self.model_path, &self.config).await?;
        let model_name = self.model_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        let state = Arc::new(AppState::new(pipeline, model_name, self.config));

        let cors = CorsLayer::new()
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_origin(Any);

        let app = Router::new()
            .route("/health",                     get(routes::health))
            .route("/v1/models",                  get(routes::list_models))
            .route("/v1/chat/completions",         post(routes::chat_completions))
            .route("/v1/completions",              post(routes::completions))
            .route("/metrics",                    get(routes::metrics))
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        info!("Markus server running on http://{}", self.bind_addr);
        info!("OpenAI-compatible: POST /v1/chat/completions");

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
