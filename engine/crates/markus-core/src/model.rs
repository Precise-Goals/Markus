//! Model metadata and discovery types

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::gguf::GgufMeta;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelFormat {
    Gguf,
    SafeTensors,
    PyTorchBin,
    Unknown(String),
}

impl ModelFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "gguf" => Self::Gguf,
            "safetensors" => Self::SafeTensors,
            "bin" => Self::PyTorchBin,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn is_native(&self) -> bool {
        matches!(self, ModelFormat::Gguf)
    }

    pub fn label(&self) -> &str {
        match self {
            ModelFormat::Gguf => "GGUF",
            ModelFormat::SafeTensors => "SafeTensors",
            ModelFormat::PyTorchBin => "PyTorch",
            ModelFormat::Unknown(s) => s.as_str(),
        }
    }
}

/// Rich model metadata — populated from GGUF header + filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub path: PathBuf,
    pub name: String,
    pub format: ModelFormat,
    pub size_bytes: u64,
    /// Parsed from GGUF header (None for non-GGUF)
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
    pub embedding_length: Option<u64>,
    pub layer_count: Option<u64>,
    pub head_count: Option<u64>,
    pub vocab_size: Option<u64>,
    pub parameter_count_b: Option<f64>,
    pub quantization: Option<String>,
    pub gguf_version: Option<u32>,
}

impl ModelInfo {
    /// Build a ModelInfo from just a path (filesystem scan mode)
    pub fn from_path(path: PathBuf) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let format = ModelFormat::from_extension(&ext);
        let size_bytes = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        Self {
            path,
            name,
            format,
            size_bytes,
            architecture: None,
            context_length: None,
            embedding_length: None,
            layer_count: None,
            head_count: None,
            vocab_size: None,
            parameter_count_b: None,
            quantization: None,
            gguf_version: None,
        }
    }

    /// Enrich with GGUF metadata
    pub fn with_gguf_meta(mut self, meta: &GgufMeta) -> Self {
        self.architecture = meta.architecture().map(str::to_string);
        self.context_length = meta.context_length();
        self.embedding_length = meta.embedding_length();
        self.layer_count = meta.layer_count();
        self.head_count = meta.head_count();
        self.vocab_size = meta.vocab_size();
        self.gguf_version = Some(meta.version);

        // Estimate parameter count from tensor count and embedding
        if let (Some(emb), Some(layers)) = (self.embedding_length, self.layer_count) {
            // Very rough heuristic: 4 * hidden * layers * 4 (for transformer weights)
            let params = 4 * emb * layers * 4;
            self.parameter_count_b = Some(params as f64 / 1e9);
        }
        self
    }

    pub fn size_display(&self) -> String {
        let gb = self.size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let mb = self.size_bytes as f64 / 1024.0 / 1024.0;
        if gb >= 1.0 {
            format!("{:.1}GB", gb)
        } else {
            format!("{:.0}MB", mb)
        }
    }
}
