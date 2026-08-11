//! Error types for markus-core

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MarkusError {
    #[error("GGUF parse error: {0}")]
    GgufParse(String),

    #[error("Model not found: {path}")]
    ModelNotFound { path: String },

    #[error("Unsupported model architecture: {arch}")]
    UnsupportedArch { arch: String },

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Candle error: {0}")]
    Candle(#[from] candle_core::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Download error: {0}")]
    Download(String),

    #[error("Context length exceeded: {used} > {max}")]
    ContextExceeded { used: usize, max: usize },

    #[error("Out of memory: need {need_mb}MB, have {avail_mb}MB")]
    OutOfMemory { need_mb: u64, avail_mb: u64 },
}
