//! Architecture dispatcher — loads the right model runner for each GGUF architecture
//! Uses candle-transformers quantized models (native GGUF loading via candle's gguf_file)

use std::path::Path;
use candle_core::{Device, Tensor};
use candle_transformers::models::quantized_llama::ModelWeights as QuantizedLlama;
use candle_transformers::models::quantized_phi3::ModelWeights as QuantizedPhi3;

use crate::config::MarkusConfig;
use crate::gguf::GgufMeta;
use crate::{MarkusError, Result};

/// Trait that all architecture-specific runners must implement
pub trait ModelRunner {
    /// Run a forward pass and return logits tensor [1, vocab_size]
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle_core::Result<Tensor>;
}

/// Load the appropriate model runner based on GGUF architecture string
pub fn load_model(
    path: &Path,
    meta: &GgufMeta,
    device: &Device,
    cfg: &MarkusConfig,
) -> Result<Box<dyn ModelRunner + Send + Sync>> {
    use std::fs::File;
    use std::io::BufReader;
    use candle_core::quantized::gguf_file;

    let arch = meta.architecture().unwrap_or("llama").to_string();

    let file = File::open(path).map_err(MarkusError::Io)?;
    let mut reader = BufReader::new(file);

    // Load GGUF content via candle's own reader
    let gguf_content = gguf_file::Content::read(&mut reader)
        .map_err(|e| MarkusError::Inference(format!("candle GGUF read error: {}", e)))?;

    match arch.as_str() {
        // LLaMA family — covers most modern models
        "llama" | "llama2" | "llama3" | "llama4" | "codellama" | "smollm"
        | "tinyllama" | "qwen2" | "qwen2_5" | "gemma" | "gemma2" | "gemma3"
        | "deepseek" | "deepseek2" | "starcoder" | "starcoder2"
        | "orion" | "baichuan" | "internlm2" | "mamba" => {
            let model = QuantizedLlama::from_gguf(gguf_content, &mut reader, device)
                .map_err(|e| MarkusError::Inference(format!("LLaMA load error: {}", e)))?;
            Ok(Box::new(LlamaRunner { model }))
        }

        // Phi-3/3.5/4 family
        "phi3" | "phi2" | "phi" => {
            let model = QuantizedPhi3::from_gguf(false, gguf_content, &mut reader, device)
                .map_err(|e| MarkusError::Inference(format!("Phi3 load error: {}", e)))?;
            Ok(Box::new(Phi3Runner { model }))
        }

        // Mistral — uses llama weights (same transformer block structure)
        "mistral" | "mistral_nemo" | "mixtral" => {
            // Mistral's GGUF is llama-compatible; use the LLaMA runner
            let model = QuantizedLlama::from_gguf(gguf_content, &mut reader, device)
                .map_err(|e| MarkusError::Inference(format!("Mistral load error: {}", e)))?;
            Ok(Box::new(LlamaRunner { model }))
        }

        unsupported => {
            Err(MarkusError::UnsupportedArch {
                arch: unsupported.to_string()
            })
        }
    }
}

// ── LLaMA runner (covers LLaMA 1/2/3/4, Qwen2, Gemma, DeepSeek, Mistral, etc.) ─

struct LlamaRunner {
    model: QuantizedLlama,
}

impl ModelRunner for LlamaRunner {
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle_core::Result<Tensor> {
        self.model.forward(input, pos)
    }
}

// ── Phi-3 runner ──────────────────────────────────────────────────────────────

struct Phi3Runner {
    model: QuantizedPhi3,
}

impl ModelRunner for Phi3Runner {
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle_core::Result<Tensor> {
        self.model.forward(input, pos)
    }
}
