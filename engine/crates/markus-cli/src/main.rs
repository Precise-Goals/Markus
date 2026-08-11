//! markus-engine — Pure Rust Local LLM Manager
//!
//! Replaces the Bash script entirely. Works with GGUF models directly
//! via candle-transformers — no llama.cpp dependency.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt};

use markus_core::{MarkusConfig, ModelScanner, ModelDownloader, DownloadSpec, SystemInfo};

mod tui_runner;

#[derive(Parser, Debug)]
#[command(
    name = "markus-engine",
    version = "3.0.0",
    about = "Markus — Pure Rust Local LLM Engine. Zero llama.cpp dependency.",
    long_about = "Markus v3 is a fully independent Rust-native LLM manager and inference engine.\n\
                  Powered by candle (HuggingFace) — runs GGUF models without llama.cpp.",
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Verbosity level
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch interactive TUI (default)
    #[command(alias = "ui")]
    Tui,

    /// Chat with a model in the terminal
    #[command(alias = "chat")]
    Run {
        /// Model path, index, or name fragment
        #[arg(value_name = "MODEL")]
        model: Option<String>,

        /// Initial prompt (non-interactive single-shot)
        #[arg(short, long)]
        prompt: Option<String>,

        /// Temperature (0.0–2.0)
        #[arg(long, default_value = "0.7")]
        temp: f64,

        /// Max tokens to generate (-1 = unlimited)
        #[arg(short = 'n', long, default_value = "-1")]
        max_tokens: i32,

        /// Context window size
        #[arg(short, long, default_value = "4096")]
        ctx: u32,

        /// System prompt
        #[arg(short = 's', long)]
        system: Option<String>,

        /// GPU layers to offload (0 = CPU)
        #[arg(long, default_value = "0")]
        gpu_layers: u32,
    },

    /// Start OpenAI-compatible HTTP API server
    #[command(alias = "server")]
    Serve {
        /// Model path, index, or name fragment
        #[arg(value_name = "MODEL")]
        model: Option<String>,

        /// Host to listen on
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// GPU layers
        #[arg(long, default_value = "0")]
        gpu_layers: u32,
    },

    /// Download a model from HuggingFace or URL
    #[command(alias = "download", alias = "get")]
    Pull {
        /// Model alias, hf:repo:file, or https:// URL
        model: String,
    },

    /// List all detected models on this system
    #[command(alias = "ls", alias = "models")]
    List,

    /// Scan filesystem for model files
    Scan {
        /// Ignore cache and force rescan
        #[arg(short, long)]
        force: bool,
    },

    /// Show detailed metadata for a model
    #[command(alias = "show")]
    Info {
        model: String,
    },

    /// Remove a model file from disk
    #[command(alias = "rm", alias = "delete")]
    Remove {
        model: String,
    },

    /// Show system hardware and memory information
    Status,

    /// Free system RAM (kill processes, drop kernel caches)
    #[command(alias = "free", alias = "clearmem")]
    Freemem,

    /// Show and edit configuration
    Config {
        #[arg(value_name = "ACTION", default_value = "show")]
        action: String,
        key: Option<String>,
        value: Option<String>,
    },

    /// Show version information
    #[command(alias = "-V")]
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Init logging
    let filter = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .compact()
        .init();

    // Load config and init dirs
    MarkusConfig::init_dirs()?;
    let config = MarkusConfig::load().unwrap_or_default();

    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Tui => {
            tui_runner::run(config).await?;
        }

        Commands::Run { model, prompt, temp, max_tokens, ctx, system, gpu_layers } => {
            let model_path = resolve_model(model, &config)?;
            let mut gen_config = markus_core::GenerationConfig {
                temperature: temp,
                max_tokens,
                ..Default::default()
            };

            let mut cfg = config.clone();
            cfg.ctx_size = ctx;
            cfg.gpu_layers = gpu_layers;
            if let Some(sp) = system { cfg.system_prompt = sp; }

            if let Some(p) = prompt {
                // Single-shot mode
                run_single_shot(&model_path, &p, &gen_config, &cfg).await?;
            } else {
                // Interactive CLI chat
                run_interactive_chat(&model_path, &gen_config, &cfg).await?;
            }
        }

        Commands::Serve { model, host, port, gpu_layers } => {
            let model_path = resolve_model(model, &config)?;
            let mut cfg = config.clone();
            cfg.server_host = host;
            cfg.server_port = port;
            cfg.gpu_layers = gpu_layers;

            println!("\n  \x1b[91m\x1b[1m▸ MARKUS SERVER\x1b[0m  \x1b[96m{}\x1b[0m", model_path.display());
            println!("  \x1b[2mEndpoint:\x1b[0m  \x1b[96mhttp://{}:{}\x1b[0m", cfg.server_host, cfg.server_port);
            println!("  \x1b[2mPress Ctrl+C to stop\x1b[0m\n");

            let server = markus_server::Server::new(model_path, cfg);
            server.run().await?;
        }

        Commands::Pull { model } => {
            cmd_pull(&model).await?;
        }

        Commands::List => cmd_list(&config),

        Commands::Scan { force } => cmd_scan(force, &config),

        Commands::Info { model } => cmd_info(&model, &config)?,

        Commands::Remove { model } => cmd_remove(&model, &config)?,

        Commands::Status => cmd_status(),

        Commands::Freemem => cmd_freemem(),

        Commands::Config { action, key, value } => {
            cmd_config(&action, key.as_deref(), value.as_deref(), &config)?;
        }

        Commands::Version => {
            println!("\n  \x1b[91m\x1b[1mmarkus-engine\x1b[0m  v3.0.0  —  Pure Rust · No llama.cpp");
            println!("  \x1b[96mBackend:\x1b[0m   candle (HuggingFace Rust ML framework)");
            println!("  \x1b[96mRuntime:\x1b[0m   Tokio async · Ratatui TUI");
            println!("  \x1b[96mFormats:\x1b[0m   GGUF (native) · SafeTensors (planned)");
            println!();
        }
    }

    Ok(())
}

// ── Helper functions ───────────────────────────────────────────────────────────

fn resolve_model(model: Option<String>, config: &MarkusConfig) -> anyhow::Result<PathBuf> {
    match model {
        Some(m) => {
            let p = PathBuf::from(&m);
            if p.exists() { return Ok(p); }

            // Try as index
            if let Ok(idx) = m.parse::<usize>() {
                let scanner = ModelScanner::new();
                let models = scanner.scan(false);
                if let Some(info) = models.get(idx.saturating_sub(1)) {
                    return Ok(info.path.clone());
                }
            }

            // Try as name fragment
            let scanner = ModelScanner::new();
            let models = scanner.scan(false);
            for info in &models {
                if info.name.to_lowercase().contains(&m.to_lowercase()) {
                    return Ok(info.path.clone());
                }
            }

            anyhow::bail!("Model not found: '{}'. Run 'markus-engine list'", m);
        }
        None => {
            // Interactive picker
            let scanner = ModelScanner::new();
            let models = scanner.scan(false);
            if models.is_empty() {
                anyhow::bail!("No models found. Run 'markus-engine pull <model>'");
            }
            println!("\n  \x1b[91m\x1b[1mSelect a model:\x1b[0m\n");
            for (i, m) in models.iter().enumerate() {
                println!("    \x1b[93m[{:2}]\x1b[0m  \x1b[96m{:<50}\x1b[0m  \x1b[2m{}\x1b[0m",
                    i + 1, m.name, m.size_display());
            }
            print!("\n  \x1b[96mEnter number (1-{}): \x1b[0m", models.len());
            use std::io::{self, BufRead, Write};
            io::stdout().flush()?;
            let stdin = io::stdin();
            let line = stdin.lock().lines().next()
                .ok_or_else(|| anyhow::anyhow!("No input"))?
                .map_err(|e| anyhow::anyhow!("IO error: {}", e))?;
            let idx: usize = line.trim().parse()
                .map_err(|_| anyhow::anyhow!("Invalid number"))?;
            models.get(idx.saturating_sub(1))
                .map(|m| m.path.clone())
                .ok_or_else(|| anyhow::anyhow!("Index out of range"))
        }
    }
}

async fn run_single_shot(
    model: &PathBuf,
    prompt: &str,
    gen_config: &markus_core::GenerationConfig,
    cfg: &MarkusConfig,
) -> anyhow::Result<()> {
    use markus_core::{GenerationPipeline, pipeline::{ChatMessage, TokenEvent}};
    use tokio::sync::mpsc;

    let mut pipeline = GenerationPipeline::load(model, cfg).await
        .map_err(|e| anyhow::anyhow!("Failed to load model: {}", e))?;

    let messages = vec![
        ChatMessage { role: "system".into(), content: cfg.system_prompt.clone() },
        ChatMessage { role: "user".into(), content: prompt.to_string() },
    ];

    let (tx, mut rx) = mpsc::channel(256);
    pipeline.chat_stream(&messages, gen_config, tx).await;

    while let Some(event) = rx.recv().await {
        match event {
            TokenEvent::Token(t) => print!("{}", t),
            TokenEvent::Done { tokens_generated, elapsed_ms } => {
                let tps = tokens_generated as f64 / (elapsed_ms as f64 / 1000.0);
                eprintln!("\n\n  \x1b[2m[{} tokens · {:.1} t/s]\x1b[0m", tokens_generated, tps);
            }
            TokenEvent::Error(e) => {
                eprintln!("\n  \x1b[91mError: {}\x1b[0m", e);
            }
        }
    }
    println!();
    Ok(())
}

async fn run_interactive_chat(
    model: &PathBuf,
    gen_config: &markus_core::GenerationConfig,
    cfg: &MarkusConfig,
) -> anyhow::Result<()> {
    use markus_core::{GenerationPipeline, pipeline::{ChatMessage, TokenEvent}};
    use tokio::sync::mpsc;
    use std::io::{self, BufRead, Write};

    println!("\n  \x1b[91m\x1b[1m╔═ MARKUS CHAT ═══════════════════════════════╗\x1b[0m");
    println!("  \x1b[96m  Model:\x1b[0m {}", model.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
    println!("  \x1b[91m\x1b[1m╚═════════════════════════════════════════════╝\x1b[0m");
    println!("  \x1b[2mType 'exit' or Ctrl+C to quit. '/clear' to reset history.\x1b[0m\n");

    println!("  \x1b[2mLoading model...\x1b[0m");
    let mut pipeline = GenerationPipeline::load(model, cfg).await
        .map_err(|e| anyhow::anyhow!("Failed to load model: {}", e))?;
    println!("  \x1b[92m✔  Model ready!\x1b[0m\n");

    let mut history: Vec<ChatMessage> = vec![];
    let stdin = io::stdin();

    loop {
        print!("  \x1b[96m\x1b[1mYou:\x1b[0m ");
        io::stdout().flush()?;

        let line = match stdin.lock().lines().next() {
            Some(Ok(l)) => l,
            _ => break,
        };

        let input = line.trim().to_string();
        if input.is_empty() { continue; }

        match input.as_str() {
            "exit" | "quit" | "/exit" | "/quit" => {
                println!("\n  \x1b[2mGoodbye!\x1b[0m");
                break;
            }
            "/clear" => {
                history.clear();
                println!("  \x1b[2m[History cleared]\x1b[0m\n");
                continue;
            }
            "/info" => {
                println!("  \x1b[96mModel:\x1b[0m {}", model.display());
                println!("  \x1b[96mHistory turns:\x1b[0m {}", history.len() / 2);
                println!();
                continue;
            }
            _ => {}
        }

        history.push(ChatMessage { role: "user".into(), content: input });

        let mut messages = vec![
            ChatMessage { role: "system".into(), content: cfg.system_prompt.clone() },
        ];
        messages.extend(history.clone());

        let (tx, mut rx) = mpsc::channel(256);
        print!("\n  \x1b[91m\x1b[1mMarkus:\x1b[0m ");
        io::stdout().flush()?;

        pipeline.chat_stream(&messages, gen_config, tx).await;

        let mut reply = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                TokenEvent::Token(t) => {
                    print!("{}", t);
                    io::stdout().flush()?;
                    reply.push_str(&t);
                }
                TokenEvent::Done { tokens_generated, elapsed_ms } => {
                    let tps = tokens_generated as f64 / (elapsed_ms as f64 / 1000.0);
                    println!("\n\n  \x1b[2m[{} tokens · {:.1} t/s]\x1b[0m\n", tokens_generated, tps);
                }
                TokenEvent::Error(e) => {
                    println!("\n  \x1b[91mError: {}\x1b[0m\n", e);
                }
            }
        }

        if !reply.is_empty() {
            history.push(ChatMessage { role: "assistant".into(), content: reply });
        }
    }

    Ok(())
}

fn cmd_list(config: &MarkusConfig) {
    let scanner = ModelScanner::new();
    let models = scanner.scan(false);

    if models.is_empty() {
        println!("\n  \x1b[93m⚠\x1b[0m  No models found. Run: markus-engine pull <model>\n");
        return;
    }

    println!("\n  \x1b[91m\x1b[1m┌─ Found {} model(s) ──────────────────────────────────────────┐\x1b[0m", models.len());
    for (i, m) in models.iter().enumerate() {
        let col = if m.format.is_native() { "\x1b[92m" } else { "\x1b[97m" };
        println!("  \x1b[2m│\x1b[0m  \x1b[93m\x1b[1m{:3}\x1b[0m  \x1b[2m{:<8}\x1b[0m  {}{}\x1b[0m",
            i + 1, m.size_display(), col, m.name);
        if let Some(arch) = &m.architecture {
            println!("       \x1b[2marchitecture: {}  ctx: {:?}  layers: {:?}\x1b[0m",
                arch, m.context_length, m.layer_count);
        }
    }
    println!("  \x1b[91m\x1b[1m└──────────────────────────────────────────────────────────────┘\x1b[0m\n");
}

fn cmd_scan(force: bool, _config: &MarkusConfig) {
    println!("\n  \x1b[96m◆\x1b[0m  Scanning filesystem for models...");
    let scanner = ModelScanner::new();
    if force { scanner.invalidate_cache(); }
    let models = scanner.scan(force);
    println!("  \x1b[92m✔\x1b[0m  Found {} model(s)\n", models.len());
    cmd_list(_config);
}

fn cmd_info(model_inp: &str, _config: &MarkusConfig) -> anyhow::Result<()> {
    let path = resolve_model(Some(model_inp.to_string()), _config)
        .map_err(|e| anyhow::anyhow!(e))?;

    use markus_core::GgufLoader;
    let loader = GgufLoader::load(&path)
        .map_err(|e| anyhow::anyhow!("GGUF parse error: {}", e))?;
    let meta = &loader.meta;

    println!("\n  \x1b[91m\x1b[1m╔═ Model Info ════════════════════════════════════════════╗\x1b[0m");
    println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Name:", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
    println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Path:", path.display());
    if let Some(arch) = meta.architecture() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Architecture:", arch);
    }
    println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m GGUF v{}", "Format:", meta.version);
    println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Tensors:", meta.tensor_count);
    if let Some(ctx) = meta.context_length() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {} tokens", "Context:", ctx);
    }
    if let Some(layers) = meta.layer_count() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Layers:", layers);
    }
    if let Some(emb) = meta.embedding_length() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Embedding dim:", emb);
    }
    if let Some(heads) = meta.head_count() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Attn heads:", heads);
    }
    if let Some(vocab) = meta.vocab_size() {
        println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m {}", "Vocab size:", vocab);
    }
    println!("  \x1b[2m║\x1b[0m  \x1b[96m{:<18}\x1b[0m ~{}MB (est. w/ KV cache)",
        "Memory needed:", loader.estimated_memory_mb());
    println!("  \x1b[91m\x1b[1m╚═════════════════════════════════════════════════════════╝\x1b[0m\n");

    Ok(())
}

fn cmd_remove(model_inp: &str, config: &MarkusConfig) -> anyhow::Result<()> {
    let path = resolve_model(Some(model_inp.to_string()), config)
        .map_err(|e| anyhow::anyhow!(e))?;

    println!("\n  \x1b[91m⚠  About to remove:\x1b[0m  {}", path.display());
    print!("  \x1b[91mConfirm? [y/N]: \x1b[0m");
    use std::io::{self, BufRead, Write};
    io::stdout().flush()?;
    let stdin = io::stdin();
    let line = stdin.lock().lines().next()
        .ok_or_else(|| anyhow::anyhow!("No input"))??;

    if line.trim().eq_ignore_ascii_case("y") {
        std::fs::remove_file(&path)?;
        ModelScanner::new().invalidate_cache();
        println!("  \x1b[92m✔\x1b[0m  Removed: {}\n", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
    } else {
        println!("  \x1b[2mCancelled.\x1b[0m\n");
    }
    Ok(())
}

fn cmd_status() {
    println!("\n  \x1b[96m◆\x1b[0m  Collecting system info...");
    let info = SystemInfo::collect();

    println!("\n  \x1b[91m\x1b[1mSystem\x1b[0m  \x1b[2m────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[96m{:<18}\x1b[0m {}  \x1b[2m({} cores / {} threads)\x1b[0m",
        "CPU:", info.cpu_model, info.cpu_cores, info.cpu_threads);
    println!("  \x1b[96m{:<18}\x1b[0m {}  available: {}",
        "Memory:", info.ram_display(), info.available_ram_display());

    if !info.gpu_info.is_empty() {
        println!("\n  \x1b[91m\x1b[1mGPU\x1b[0m  \x1b[2m────────────────────────────────────────────\x1b[0m");
        for gpu in &info.gpu_info {
            print!("  \x1b[96m{:<18}\x1b[0m {}", "GPU:", gpu.name);
            if let Some(vram) = gpu.vram_total_mb {
                print!("  \x1b[2m({}MB VRAM)\x1b[0m", vram);
            }
            println!();
        }
    }

    println!("\n  \x1b[91m\x1b[1mPlatform\x1b[0m  \x1b[2m────────────────────────────────────────\x1b[0m");
    println!("  \x1b[96m{:<18}\x1b[0m {}", "OS:", info.os);
    println!("  \x1b[96m{:<18}\x1b[0m {}", "Architecture:", info.arch);
    println!("  \x1b[96m{:<18}\x1b[0m candle (HuggingFace Rust)", "Inference Engine:");
    println!("  \x1b[96m{:<18}\x1b[0m Zero (no llama.cpp)", "External Deps:");
    println!();
}

fn cmd_freemem() {
    println!("\n  \x1b[96m◆\x1b[0m  Freeing system memory...\n");

    let killed = markus_core::system::kill_inference_processes();
    if killed.is_empty() {
        println!("  \x1b[2mNo inference processes found\x1b[0m");
    } else {
        for (pid, name) in &killed {
            println!("  \x1b[92m✔\x1b[0m  Killed PID {} ({})", pid, name);
        }
    }

    println!();
    let actions = markus_core::system::drop_kernel_caches();
    for action in &actions {
        println!("  \x1b[92m✔\x1b[0m  {}", action);
    }

    ModelScanner::new().invalidate_cache();
    println!("\n  \x1b[92m✔\x1b[0m  Done\n");
}

async fn cmd_pull(model: &str) -> anyhow::Result<()> {
    let spec = DownloadSpec::from_input(model)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("\n  \x1b[96m◆\x1b[0m  Downloading: \x1b[93m{}\x1b[0m", spec.filename);
    println!("  \x1b[2mFrom:\x1b[0m  {}\n", spec.url);

    let downloader = ModelDownloader::new()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let dest = downloader.download(&spec, |dl, total| {
        let pct = if total > 0 { dl * 100 / total } else { 0 };
        let mb = dl as f64 / 1024.0 / 1024.0;
        let total_mb = total as f64 / 1024.0 / 1024.0;
        eprint!("\r  \x1b[96m  {:.0}/{:.0}MB  {}%\x1b[0m      ", mb, total_mb, pct);
    }).await.map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("\n\n  \x1b[92m✔\x1b[0m  Saved to: {}", dest.display());
    println!("  \x1b[2mRun:\x1b[0m  markus-engine run {}\n", dest.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
    Ok(())
}

fn cmd_config(action: &str, key: Option<&str>, value: Option<&str>, config: &MarkusConfig) -> anyhow::Result<()> {
    match action {
        "show" | "get" => {
            let toml = toml::to_string_pretty(config)
                .map_err(|e| anyhow::anyhow!("Config serialize error: {}", e))?;
            println!("\n  \x1b[91m\x1b[1m╔═ Configuration ═══════════════════════════════════════╗\x1b[0m");
            println!("  \x1b[2m  File: {}\x1b[0m", MarkusConfig::config_file().display());
            println!();
            for line in toml.lines() {
                println!("  {}", line);
            }
            println!("  \x1b[91m\x1b[1m╚═══════════════════════════════════════════════════════╝\x1b[0m\n");
        }
        "edit" => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".into());
            std::process::Command::new(editor)
                .arg(MarkusConfig::config_file())
                .status()?;
        }
        "reset" => {
            let default = MarkusConfig::default();
            default.save().map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("  \x1b[92m✔\x1b[0m  Configuration reset to defaults\n");
        }
        _ => {
            eprintln!("  Unknown config action: '{}'. Use: show | edit | reset", action);
        }
    }
    Ok(())
}
