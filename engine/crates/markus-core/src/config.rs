//! Persistent user configuration for Markus

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Context;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkusConfig {
    /// CPU thread count (default: all cores)
    pub threads: u32,
    /// Context window size in tokens
    pub ctx_size: u32,
    /// Batch size for parallel processing
    pub batch_size: u32,
    /// GPU offload layers (0 = CPU only)
    pub gpu_layers: u32,
    /// Default temperature for sampling
    pub temperature: f32,
    /// Top-P nucleus sampling
    pub top_p: f32,
    /// Top-K sampling
    pub top_k: u32,
    /// Repetition penalty
    pub repeat_penalty: f32,
    /// Max tokens to generate (-1 = unlimited)
    pub max_tokens: i32,
    /// HTTP server host
    pub server_host: String,
    /// HTTP server port
    pub server_port: u16,
    /// Flash attention (faster, less VRAM)
    pub flash_attn: bool,
    /// Memory-lock the model to prevent swapping
    pub mlock: bool,
    /// Default system prompt
    pub system_prompt: String,
}

impl Default for MarkusConfig {
    fn default() -> Self {
        Self {
            threads: num_cpus::get() as u32,
            ctx_size: 4096,
            batch_size: 512,
            gpu_layers: 0,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: -1,
            server_host: "127.0.0.1".into(),
            server_port: 8080,
            flash_attn: true,
            mlock: false,
            system_prompt: "You are Markus, a helpful AI assistant running entirely locally. You are fast, private, and independent.".into(),
        }
    }
}

impl MarkusConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("markus")
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn models_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("markus")
            .join("models")
    }

    pub fn logs_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("markus")
            .join("logs")
    }

    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("~/.cache"))
            .join("markus")
    }

    pub fn load() -> crate::Result<Self> {
        let path = Self::config_file();
        if !path.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)
            .map_err(|e| crate::MarkusError::Config(format!("Failed to parse config: {}", e)))
    }

    pub fn save(&self) -> crate::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::MarkusError::Config(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(Self::config_file(), content)?;
        Ok(())
    }

    pub fn init_dirs() -> crate::Result<()> {
        std::fs::create_dir_all(Self::config_dir())?;
        std::fs::create_dir_all(Self::models_dir())?;
        std::fs::create_dir_all(Self::logs_dir())?;
        std::fs::create_dir_all(Self::cache_dir())?;
        Ok(())
    }
}
