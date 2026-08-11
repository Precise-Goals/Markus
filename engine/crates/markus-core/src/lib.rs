//! markus-core — Pure-Rust GGUF inference engine
//!
//! Zero dependency on llama.cpp. Uses HuggingFace Candle for tensor math.
//!
//! # Architecture
//! ```
//! GgufLoader  ──►  ModelConfig  ──►  Transformer
//!      │                                   │
//!      └──►  Tokenizer  ◄──────────────────┘
//!                 │
//!                 ▼
//!           GenerationPipeline  ──►  token stream
//! ```

pub mod config;
pub mod error;
pub mod gguf;
pub mod model;
pub mod models;
pub mod pipeline;
pub mod scanner;
pub mod system;
pub mod tokenizer;
pub mod download;

pub use config::MarkusConfig;
pub use error::MarkusError;
pub use gguf::{GgufLoader, GgufMeta};
pub use model::{ModelInfo, ModelFormat};
pub use pipeline::{GenerationConfig, GenerationPipeline, TokenEvent};
pub use scanner::ModelScanner;
pub use system::SystemInfo;
pub use download::{DownloadSpec, ModelDownloader};

pub type Result<T> = std::result::Result<T, MarkusError>;
