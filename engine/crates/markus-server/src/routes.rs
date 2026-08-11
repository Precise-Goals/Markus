//! HTTP route handlers

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse},
    Json,
};
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;
use uuid::Uuid;

use markus_core::pipeline::{ChatMessage, GenerationConfig, TokenEvent};
use crate::state::AppState;
use crate::types::*;

fn unix_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn gen_id() -> String {
    format!("chatcmpl-{}", Uuid::new_v4().to_string().replace('-', "")[..16].to_string())
}

/// GET /health
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "model": state.model_name,
        "uptime_secs": state.uptime_secs().await,
    }))
}

/// GET /metrics
pub async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tokens = *state.tokens_generated.lock().await;
    let requests = *state.requests_handled.lock().await;
    Json(json!({
        "tokens_generated": tokens,
        "requests_handled": requests,
        "uptime_secs": state.uptime_secs().await,
        "model": state.model_name,
    }))
}

/// GET /v1/models
pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(ModelListResponse {
        object: "list",
        data: vec![ModelObject {
            id: state.model_name.clone(),
            object: "model",
            created: unix_ts(),
            owned_by: "markus",
        }],
    })
}

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    let gen_config = GenerationConfig {
        temperature: req.temperature,
        top_p: req.top_p,
        max_tokens: req.max_tokens.unwrap_or(state.config.max_tokens),
        top_k: state.config.top_k as u64,
        repeat_penalty: state.config.repeat_penalty,
        seed: 42,
    };

    *state.requests_handled.lock().await += 1;

    if req.stream {
        stream_chat_response(state, req.messages, gen_config).await
    } else {
        blocking_chat_response(state, req.messages, gen_config).await
    }
}

/// POST /v1/completions (raw prompt, non-chat)
pub async fn completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletionRequest>,
) -> impl IntoResponse {
    let messages = vec![
        ChatMessage { role: "user".into(), content: req.prompt },
    ];
    let gen_config = GenerationConfig {
        temperature: req.temperature,
        max_tokens: req.max_tokens.unwrap_or(state.config.max_tokens),
        ..Default::default()
    };
    blocking_chat_response(state, messages, gen_config).await
}

async fn blocking_chat_response(
    state: Arc<AppState>,
    messages: Vec<ChatMessage>,
    gen_config: GenerationConfig,
) -> Response {
    let (tx, mut rx) = mpsc::channel::<TokenEvent>(256);
    let mut pipeline = state.pipeline.lock().await;

    let messages_clone = messages.clone();
    let gen_clone = gen_config.clone();

    pipeline.chat_stream(&messages_clone, &gen_clone, tx).await;
    drop(pipeline);

    let mut content = String::new();
    let mut total_tokens = 0u32;

    while let Some(event) = rx.recv().await {
        match event {
            TokenEvent::Token(t) => content.push_str(&t),
            TokenEvent::Done { tokens_generated, .. } => {
                total_tokens = tokens_generated;
                *state.tokens_generated.lock().await += tokens_generated as u64;
            }
            TokenEvent::Error(e) => {
                return Json(ErrorResponse {
                    error: ErrorDetail {
                        message: e,
                        r#type: "inference_error".into(),
                        code: None,
                    }
                }).into_response();
            }
        }
    }

    Json(ChatCompletionResponse {
        id: gen_id(),
        object: "chat.completion",
        created: unix_ts(),
        model: state.model_name.clone(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage { role: "assistant".into(), content },
            finish_reason: "stop".into(),
        }],
        usage: Usage {
            prompt_tokens: 0, // TODO: track prompt tokens
            completion_tokens: total_tokens,
            total_tokens,
        },
    }).into_response()
}

async fn stream_chat_response(
    state: Arc<AppState>,
    messages: Vec<ChatMessage>,
    gen_config: GenerationConfig,
) -> Response {
    let (tx, rx) = mpsc::channel::<TokenEvent>(256);
    let model_name = state.model_name.clone();
    let state_clone = state.clone();

    // Spawn generation in background
    tokio::spawn(async move {
        let mut pipeline = state_clone.pipeline.lock().await;
        pipeline.chat_stream(&messages, &gen_config, tx).await;
    });

    let id = gen_id();
    let stream = ReceiverStream::new(rx).map(move |event| {
        let id = id.clone();
        let model = model_name.clone();
        let chunk = match event {
            TokenEvent::Token(text) => ChatChunk {
                id,
                object: "chat.completion.chunk",
                created: unix_ts(),
                model,
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: Delta { role: None, content: Some(text) },
                    finish_reason: None,
                }],
            },
            TokenEvent::Done { .. } => ChatChunk {
                id,
                object: "chat.completion.chunk",
                created: unix_ts(),
                model,
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: Delta { role: None, content: None },
                    finish_reason: Some("stop".into()),
                }],
            },
            TokenEvent::Error(e) => ChatChunk {
                id,
                object: "chat.completion.chunk",
                created: unix_ts(),
                model,
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: Delta { role: None, content: Some(format!("[ERROR: {}]", e)) },
                    finish_reason: Some("error".into()),
                }],
            },
        };

        let json = serde_json::to_string(&chunk).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(format!("data: {}\n\n", json))
    });

    let body = Body::from_stream(stream);
    Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap()
}
