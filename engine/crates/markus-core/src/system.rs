//! System information — CPU, RAM, GPU detection

use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub gpu_info: Vec<GpuInfo>,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_total_mb: Option<u64>,
    pub vram_free_mb: Option<u64>,
    pub backend: GpuBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuBackend {
    Cuda,
    Metal,
    Vulkan,
    None,
}

impl SystemInfo {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_model = sys.cpus().first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        let cpu_threads = sys.cpus().len() as u32;
        let cpu_cores = num_cpus::get_physical() as u32;
        let total_ram_mb = sys.total_memory() / 1024 / 1024;
        let available_ram_mb = sys.available_memory() / 1024 / 1024;

        let arch = std::env::consts::ARCH.to_string();
        let os = format!("{} {}", std::env::consts::OS, System::os_version().unwrap_or_default());

        // GPU detection via nvidia-smi (optional — non-fatal if absent)
        let gpu_info = detect_gpus();

        Self {
            cpu_model,
            cpu_cores,
            cpu_threads,
            total_ram_mb,
            available_ram_mb,
            gpu_info,
            os,
            arch,
        }
    }

    pub fn ram_display(&self) -> String {
        if self.total_ram_mb >= 1024 {
            format!("{:.1}GB", self.total_ram_mb as f64 / 1024.0)
        } else {
            format!("{}MB", self.total_ram_mb)
        }
    }

    pub fn available_ram_display(&self) -> String {
        if self.available_ram_mb >= 1024 {
            format!("{:.1}GB", self.available_ram_mb as f64 / 1024.0)
        } else {
            format!("{}MB", self.available_ram_mb)
        }
    }

    pub fn has_gpu(&self) -> bool {
        !self.gpu_info.is_empty()
    }
}

fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = vec![];

    // Try nvidia-smi
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total,memory.free", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.splitn(3, ',').collect();
                if parts.len() >= 1 {
                    let name = parts[0].trim().to_string();
                    let vram_total = parts.get(1)
                        .and_then(|v| v.trim().parse::<u64>().ok());
                    let vram_free = parts.get(2)
                        .and_then(|v| v.trim().parse::<u64>().ok());
                    gpus.push(GpuInfo {
                        name,
                        vram_total_mb: vram_total,
                        vram_free_mb: vram_free,
                        backend: GpuBackend::Cuda,
                    });
                }
            }
        }
    }

    // On macOS, detect Apple Silicon Metal
    #[cfg(target_os = "macos")]
    if gpus.is_empty() {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()
        {
            // Simplified: just mark Metal as available on macOS
            gpus.push(GpuInfo {
                name: "Apple Silicon GPU (Metal)".to_string(),
                vram_total_mb: None,
                vram_free_mb: None,
                backend: GpuBackend::Metal,
            });
        }
    }

    gpus
}

/// Free system memory by dropping kernel caches (Linux only)
pub fn drop_kernel_caches() -> Vec<String> {
    let mut actions = vec![];

    #[cfg(target_os = "linux")]
    {
        // sync first
        let _ = std::process::Command::new("sync").output();
        actions.push("Filesystem buffers synced".to_string());

        // drop_caches level 3
        if std::fs::write("/proc/sys/vm/drop_caches", "3").is_ok() {
            actions.push("Dropped page cache, dentries, and inodes (level 3)".to_string());
        } else if std::fs::write("/proc/sys/vm/drop_caches", "1").is_ok() {
            actions.push("Dropped page cache (level 1)".to_string());
        } else {
            actions.push("Cache drop skipped (requires root/sudo)".to_string());
        }

        // Memory compaction
        if std::fs::write("/proc/sys/vm/compact_memory", "1").is_ok() {
            actions.push("Memory compaction triggered".to_string());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if std::process::Command::new("purge").output().is_ok() {
            actions.push("macOS disk cache purged".to_string());
        }
    }

    actions
}

/// Kill any running inference processes (from previous sessions)
pub fn kill_inference_processes() -> Vec<(u32, String)> {
    let mut killed = vec![];
    let targets = ["llama-server", "llama-cli", "markus-server", "ollama"];

    let sys = System::new_all();
    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_lowercase();
        if targets.iter().any(|t| name.contains(t)) {
            if proc.kill() {
                killed.push((pid.as_u32(), name));
            }
        }
    }
    killed
}
