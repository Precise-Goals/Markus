//! Model downloader — HuggingFace Hub, direct URLs, and shortcuts

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use tracing::info;

use crate::config::MarkusConfig;
use crate::{MarkusError, Result};

/// Known model shortcut aliases → HuggingFace repo:file
pub fn model_aliases() -> HashMap<&'static str, (&'static str, &'static str)> {
    let mut m = HashMap::new();
    // alias → (repo, filename)
    m.insert("llama3.2",      ("bartowski/Llama-3.2-3B-Instruct-GGUF",              "Llama-3.2-3B-Instruct-Q4_K_M.gguf"));
    m.insert("llama3.1",      ("bartowski/Meta-Llama-3.1-8B-Instruct-GGUF",         "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf"));
    m.insert("llama3.3",      ("bartowski/Llama-3.3-70B-Instruct-GGUF",             "Llama-3.3-70B-Instruct-Q4_K_M.gguf"));
    m.insert("mistral",       ("TheBloke/Mistral-7B-Instruct-v0.2-GGUF",            "mistral-7b-instruct-v0.2.Q4_K_M.gguf"));
    m.insert("mistral-nemo",  ("bartowski/Mistral-Nemo-Instruct-2407-GGUF",         "Mistral-Nemo-Instruct-2407-Q4_K_M.gguf"));
    m.insert("phi3",          ("microsoft/Phi-3-mini-4k-instruct-gguf",             "Phi-3-mini-4k-instruct-q4.gguf"));
    m.insert("phi3.5",        ("bartowski/Phi-3.5-mini-instruct-GGUF",              "Phi-3.5-mini-instruct-Q4_K_M.gguf"));
    m.insert("phi4",          ("bartowski/phi-4-GGUF",                              "phi-4-Q4_K_M.gguf"));
    m.insert("gemma2",        ("bartowski/gemma-2-9b-it-GGUF",                      "gemma-2-9b-it-Q4_K_M.gguf"));
    m.insert("gemma3",        ("lmstudio-community/gemma-3-4b-it-GGUF",             "gemma-3-4b-it-Q4_K_M.gguf"));
    m.insert("qwen2.5",       ("bartowski/Qwen2.5-7B-Instruct-GGUF",               "Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
    m.insert("qwen3",         ("Qwen/Qwen3-8B-GGUF",                               "Qwen3-8B-Q4_K_M.gguf"));
    m.insert("deepseek-r1",   ("bartowski/DeepSeek-R1-Distill-Llama-8B-GGUF",      "DeepSeek-R1-Distill-Llama-8B-Q4_K_M.gguf"));
    m.insert("deepseek-r1-70b",("bartowski/DeepSeek-R1-Distill-Llama-70B-GGUF",   "DeepSeek-R1-Distill-Llama-70B-Q4_K_M.gguf"));
    m.insert("codellama",     ("TheBloke/CodeLlama-13B-Instruct-GGUF",             "codellama-13b-instruct.Q4_K_M.gguf"));
    m.insert("starcoder2",    ("bartowski/starcoder2-15b-GGUF",                    "starcoder2-15b-Q4_K_M.gguf"));
    m.insert("tinyllama",     ("TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF",          "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"));
    m.insert("smollm2",       ("bartowski/SmolLM2-1.7B-Instruct-GGUF",            "SmolLM2-1.7B-Instruct-Q4_K_M.gguf"));
    m.insert("vicuna",        ("TheBloke/Vicuna-13B-v1.5-GGUF",                   "vicuna-13b-v1.5.Q4_K_M.gguf"));
    m.insert("openchat",      ("TheBloke/openchat-3.5-1210-GGUF",                 "openchat-3.5-1210.Q4_K_M.gguf"));
    m.insert("zephyr",        ("TheBloke/zephyr-7B-beta-GGUF",                    "zephyr-7b-beta.Q4_K_M.gguf"));
    m
}

/// Download specification — can come from alias, hf: shorthand, or direct URL
#[derive(Debug, Clone)]
pub struct DownloadSpec {
    pub url: String,
    pub filename: String,
}

impl DownloadSpec {
    pub fn from_input(input: &str) -> Result<Self> {
        let aliases = model_aliases();

        if input.starts_with("http://") || input.starts_with("https://") {
            let filename = input.split('/').last()
                .and_then(|s| s.split('?').next())
                .unwrap_or("model.gguf")
                .to_string();
            Ok(Self { url: input.to_string(), filename })
        } else if input.starts_with("hf:") {
            // Format: hf:repo/name:filename
            let rest = &input[3..];
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(MarkusError::Download(
                    "Invalid hf: format. Use hf:owner/repo:filename.gguf".into()
                ));
            }
            let repo = parts[0];
            let file = parts[1];
            Ok(Self {
                url: format!("https://huggingface.co/{}/resolve/main/{}", repo, file),
                filename: file.to_string(),
            })
        } else if let Some((repo, file)) = aliases.get(input) {
            Ok(Self {
                url: format!("https://huggingface.co/{}/resolve/main/{}", repo, file),
                filename: file.to_string(),
            })
        } else {
            Err(MarkusError::Download(format!(
                "Unknown model '{}'. Use an alias, hf:repo:file, or a direct URL.", input
            )))
        }
    }
}

pub struct ModelDownloader {
    client: Client,
    dest_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("markus-engine/3.0.0")
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .map_err(|e| MarkusError::Download(e.to_string()))?;

        Ok(Self {
            client,
            dest_dir: MarkusConfig::models_dir(),
        })
    }

    pub async fn download(
        &self,
        spec: &DownloadSpec,
        on_progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<PathBuf> {
        let dest = self.dest_dir.join(&spec.filename);
        let tmp = dest.with_extension("download");

        std::fs::create_dir_all(&self.dest_dir)?;

        if dest.exists() {
            return Ok(dest);
        }

        info!("Downloading {} → {}", spec.url, dest.display());

        let mut response = self.client.get(&spec.url)
            .send()
            .await
            .map_err(|e| MarkusError::Download(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MarkusError::Download(
                format!("HTTP {} for {}", response.status(), spec.url)
            ));
        }

        let total = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        let mut file = tokio::fs::File::create(&tmp).await?;

        while let Some(chunk) = response.chunk().await
            .map_err(|e| MarkusError::Download(e.to_string()))?
        {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            on_progress(downloaded, total);
        }

        file.flush().await?;
        drop(file);

        tokio::fs::rename(&tmp, &dest).await?;
        info!("Download complete: {}", dest.display());
        Ok(dest)
    }
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create downloader")
    }
}
