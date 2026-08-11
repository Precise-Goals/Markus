//! Filesystem model scanner — multi-root discovery with caching
//!
//! Scans all known model directories (HuggingFace hub, Ollama, LM Studio,
//! markus models dir, user home, and system-wide paths) and caches results.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use walkdir::WalkDir;
use tracing::{debug, info};

use crate::config::MarkusConfig;
use crate::model::{ModelFormat, ModelInfo};
use crate::gguf::GgufLoader;

const CACHE_TTL_SECS: u64 = 6 * 60 * 60; // 6 hours
const MIN_MODEL_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

/// Discovers and catalogs local model files
pub struct ModelScanner {
    scan_cache_path: PathBuf,
}

impl ModelScanner {
    pub fn new() -> Self {
        Self {
            scan_cache_path: MarkusConfig::cache_dir().join("model_scan.json"),
        }
    }

    /// Get cached results or run a fresh scan
    pub fn scan(&self, force: bool) -> Vec<ModelInfo> {
        if !force {
            if let Some(cached) = self.load_cache() {
                info!("Using cached model scan ({} models)", cached.len());
                return cached;
            }
        }
        let models = self.run_scan();
        self.save_cache(&models);
        models
    }

    fn load_cache(&self) -> Option<Vec<ModelInfo>> {
        let path = &self.scan_cache_path;
        if !path.exists() { return None; }

        let age = std::fs::metadata(path).ok()?
            .modified().ok()?
            .elapsed().ok()?;

        if age > Duration::from_secs(CACHE_TTL_SECS) {
            return None;
        }

        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn save_cache(&self, models: &[ModelInfo]) {
        if let Some(dir) = self.scan_cache_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(models) {
            let _ = std::fs::write(&self.scan_cache_path, json);
        }
    }

    pub fn invalidate_cache(&self) {
        let _ = std::fs::remove_file(&self.scan_cache_path);
    }

    fn run_scan(&self) -> Vec<ModelInfo> {
        let roots = self.get_scan_roots();
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in &roots {
            if !root.exists() { continue; }
            debug!("Scanning: {}", root.display());

            for entry in WalkDir::new(root)
                .follow_links(true)
                .max_depth(8)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let p = entry.path().to_path_buf();

                // Filter by extension and min size
                let ext = p.extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if !matches!(ext.as_str(), "gguf" | "safetensors" | "bin") {
                    continue;
                }

                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if size < MIN_MODEL_SIZE { continue; }

                // Deduplicate via canonical path
                if let Ok(canonical) = p.canonicalize() {
                    if seen.insert(canonical) {
                        paths.push(p);
                    }
                } else {
                    paths.push(p);
                }
            }
        }

        // Sort: GGUF first, then by size descending
        paths.sort_by(|a, b| {
            let a_gguf = a.extension().map(|e| e == "gguf").unwrap_or(false);
            let b_gguf = b.extension().map(|e| e == "gguf").unwrap_or(false);
            b_gguf.cmp(&a_gguf)
        });

        // Build ModelInfo (with GGUF metadata enrichment where possible)
        let models: Vec<ModelInfo> = paths.into_iter().map(|path| {
            let mut info = ModelInfo::from_path(path.clone());
            if info.format == ModelFormat::Gguf {
                if let Ok(loader) = GgufLoader::load(&path) {
                    info = info.with_gguf_meta(&loader.meta);
                }
            }
            info
        }).collect();

        info!("Scan complete: {} models found", models.len());
        models
    }

    fn get_scan_roots(&self) -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
        let data = dirs::data_dir().unwrap_or_else(|| home.join(".local/share"));
        let cache = dirs::cache_dir().unwrap_or_else(|| home.join(".cache"));

        vec![
            // Markus own storage
            MarkusConfig::models_dir(),
            // HuggingFace Hub
            cache.join("huggingface/hub"),
            home.join(".cache/huggingface/hub"),
            // Ollama
            home.join(".ollama/models"),
            // LM Studio
            home.join(".lmstudio/models"),
            home.join(".cache/lm-studio/models"),
            data.join("lm-studio/models"),
            // User models directories
            home.join("models"),
            home.join("Models"),
            home.join("AI/models"),
            // System-wide
            PathBuf::from("/opt/models"),
            PathBuf::from("/srv/models"),
            PathBuf::from("/data/models"),
        ]
    }
}

impl Default for ModelScanner {
    fn default() -> Self {
        Self::new()
    }
}
