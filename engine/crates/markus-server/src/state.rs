//! Shared application state for the HTTP server

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;
use markus_core::{GenerationPipeline, MarkusConfig};

pub struct AppState {
    pub pipeline: Mutex<GenerationPipeline>,
    pub model_name: String,
    pub config: MarkusConfig,
    pub started_at: Instant,
    pub tokens_generated: Mutex<u64>,
    pub requests_handled: Mutex<u64>,
}

impl AppState {
    pub fn new(pipeline: GenerationPipeline, model_name: String, config: MarkusConfig) -> Self {
        Self {
            pipeline: Mutex::new(pipeline),
            model_name,
            config,
            started_at: Instant::now(),
            tokens_generated: Mutex::new(0),
            requests_handled: Mutex::new(0),
        }
    }

    pub async fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
