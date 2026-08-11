//! Candle-based GGUF inference pipeline
//!
//! Loads GGUF models using candle-transformers (which has native GGUF support)
//! and streams token output via async channels — no llama.cpp involved.

use std::path::Path;
use std::sync::Arc;

use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::MarkusConfig;
use crate::error::MarkusError;
use crate::gguf::GgufLoader;
use crate::models::ModelRunner;
use crate::tokenizer::MarkusTokenizer;
use crate::Result;

/// Configuration for a single generation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: u64,
    pub repeat_penalty: f32,
    pub max_tokens: i32,
    pub seed: u64,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: -1,
            seed: 42,
        }
    }
}

impl GenerationConfig {
    pub fn from_markus_config(cfg: &MarkusConfig) -> Self {
        Self {
            temperature: cfg.temperature as f64,
            top_p: cfg.top_p as f64,
            top_k: cfg.top_k as u64,
            repeat_penalty: cfg.repeat_penalty,
            max_tokens: cfg.max_tokens,
            seed: 42,
        }
    }
}

/// Events emitted by the generation pipeline
#[derive(Debug, Clone)]
pub enum TokenEvent {
    /// A new token fragment was generated
    Token(String),
    /// Generation is complete — includes total stats
    Done { tokens_generated: u32, elapsed_ms: u64 },
    /// An error occurred
    Error(String),
}

/// Chat message for conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// The main inference pipeline — owns the loaded model and tokenizer
pub struct GenerationPipeline {
    runner: Box<dyn ModelRunner + Send + Sync>,
    tokenizer: MarkusTokenizer,
    device: Device,
    config: MarkusConfig,
}

impl GenerationPipeline {
    /// Load a GGUF model and prepare for inference
    pub async fn load<P: AsRef<Path>>(
        path: P,
        markus_config: &MarkusConfig,
    ) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading model: {}", path.display());

        // Parse GGUF metadata first
        let loader = GgufLoader::load(path)?;
        let arch = loader.meta.architecture()
            .ok_or_else(|| MarkusError::GgufParse("Missing architecture in GGUF".into()))?
            .to_string();

        info!("Architecture: {}, layers: {:?}, ctx: {:?}",
            arch, loader.meta.layer_count(), loader.meta.context_length());

        // Select compute device
        let device = Self::select_device(markus_config.gpu_layers)?;

        // Build tokenizer from GGUF vocabulary
        let tokenizer = MarkusTokenizer::from_gguf_meta(&loader.meta)?;

        // Dispatch to architecture-specific model runner
        let runner = crate::models::load_model(path, &loader.meta, &device, markus_config)?;

        info!("Model loaded successfully on {:?}", device);

        Ok(Self {
            runner,
            tokenizer,
            device,
            config: markus_config.clone(),
        })
    }

    /// Generate a response for a chat conversation (streaming via channel)
    pub async fn chat_stream(
        &mut self,
        messages: &[ChatMessage],
        gen_config: &GenerationConfig,
        tx: mpsc::Sender<TokenEvent>,
    ) {
        let prompt = self.format_chat_prompt(messages);
        self.generate_stream(&prompt, gen_config, tx).await;
    }

    /// Generate a response for a raw prompt string (streaming)
    pub async fn generate_stream(
        &mut self,
        prompt: &str,
        gen_config: &GenerationConfig,
        tx: mpsc::Sender<TokenEvent>,
    ) {
        let start = std::time::Instant::now();

        // Tokenize
        let tokens = match self.tokenizer.encode(prompt) {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(TokenEvent::Error(e.to_string())).await;
                return;
            }
        };

        debug!("Prompt tokens: {}", tokens.len());

        // Run inference via the model runner
        let max_tokens = if gen_config.max_tokens < 0 {
            self.config.ctx_size as usize - tokens.len()
        } else {
            gen_config.max_tokens as usize
        };

        let mut generated = 0u32;
        let mut all_tokens = tokens.clone();

        let mut logits_processor = LogitsProcessor::new(
            gen_config.seed,
            Some(gen_config.temperature),
            Some(gen_config.top_p),
        );

        for step in 0..max_tokens {
            let input = match Tensor::new(
                if step == 0 { all_tokens.as_slice() } else { &all_tokens[all_tokens.len()-1..] },
                &self.device
            ) {
                Ok(t) => t.unsqueeze(0),
                Err(e) => {
                    let _ = tx.send(TokenEvent::Error(format!("Tensor error: {}", e))).await;
                    return;
                }
            };

            let input = match input {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.send(TokenEvent::Error(format!("Unsqueeze error: {}", e))).await;
                    return;
                }
            };

            let logits = match self.runner.forward(&input, step) {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(TokenEvent::Error(format!("Forward pass error: {}", e))).await;
                    return;
                }
            };

            // Get logits for the last token
            let logits = match logits.squeeze(0) {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(TokenEvent::Error(format!("Logits squeeze error: {}", e))).await;
                    return;
                }
            };

            // Apply repeat penalty
            let logits = if gen_config.repeat_penalty != 1.0 {
                apply_repeat_penalty(&logits, gen_config.repeat_penalty as f64, &all_tokens)
                    .unwrap_or(logits)
            } else {
                logits
            };

            // Sample next token
            let next_token = match logits_processor.sample(&logits) {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.send(TokenEvent::Error(format!("Sampling error: {}", e))).await;
                    return;
                }
            };

            all_tokens.push(next_token);

            // Decode token to string
            if let Ok(text) = self.tokenizer.decode_token(next_token) {
                if !text.is_empty() {
                    let _ = tx.send(TokenEvent::Token(text)).await;
                }
            }

            generated += 1;

            // Check for EOS
            if self.tokenizer.is_eos(next_token) {
                break;
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let _ = tx.send(TokenEvent::Done { tokens_generated: generated, elapsed_ms }).await;
    }

    /// Format a chat history into a model-appropriate prompt string
    fn format_chat_prompt(&self, messages: &[ChatMessage]) -> String {
        // ChatML format (works for most modern models)
        let mut out = String::new();
        for msg in messages {
            out.push_str(&format!(
                "<|im_start|>{}\n{}<|im_end|>\n",
                msg.role, msg.content
            ));
        }
        out.push_str("<|im_start|>assistant\n");
        out
    }

    fn select_device(gpu_layers: u32) -> Result<Device> {
        if gpu_layers > 0 {
            // CUDA support: enabled via feature flag in release builds
            #[cfg(feature = "cuda")]
            if let Ok(device) = Device::new_cuda(0) {
                info!("Using CUDA device");
                return Ok(device);
            }
            // Metal support: macOS only
            #[cfg(feature = "metal")]
            if let Ok(device) = Device::new_metal(0) {
                info!("Using Metal device");
                return Ok(device);
            }
            warn!("GPU requested but no GPU device available, falling back to CPU");
        }
        Ok(Device::Cpu)
    }
}

fn apply_repeat_penalty(
    logits: &Tensor,
    penalty: f64,
    tokens: &[u32],
) -> candle_core::Result<Tensor> {
    let mut logits_v: Vec<f32> = logits.to_vec1()?;
    for &tok in tokens {
        let idx = tok as usize;
        if idx < logits_v.len() {
            let v = logits_v[idx];
            logits_v[idx] = if v > 0.0 { v / penalty as f32 } else { v * penalty as f32 };
        }
    }
    Tensor::new(logits_v, logits.device())
}
