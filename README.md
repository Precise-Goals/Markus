# Markus Engine — Pure Rust AI Model Manager (v3.0.0)

Markus is a fully independent, Rust-native Large Language Model (LLM) manager and inference engine. It replaces complex Python environments and C/C++ dependencies (like `llama.cpp`) with a blazingly fast, memory-safe, pure Rust stack powered by HuggingFace's `candle` framework.

## 🚀 Quick Install

To install Markus globally so you can run it from any directory, just copy and paste the one-liner for your OS:

### Linux & macOS
```bash
curl -sSL https://raw.githubusercontent.com/Precise-Goals/markus/main/install.sh | bash
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/Precise-Goals/markus/main/install.ps1 | iex
```

*(Note: You will need [Rust](https://rustup.rs/) and `git` installed. The installer will automatically clone, build, and add `markus` to your PATH).*

---

## 🕹️ Usage

Once installed, you can simply type `markus` from **any directory** on your machine. 

### Launch the Interactive TUI (Default)
```bash
markus
```
*Navigating the TUI:* Use arrow keys (or `j`/`k`) to move, `Enter` to select, and `q` to quit.

### Command Line Mode (CLI)
You can bypass the TUI and execute commands directly:

- **List local models:** `markus list`
- **Chat with a model:** `markus run 1` or `markus run qwen3`
- **Single-shot prompt:** `markus run 1 --prompt "Explain quantum computing"`
- **Download a model:** `markus pull llama3.2`
- **Start OpenAI API Server:** `markus serve 1 --port 8080`
- **Show system hardware status:** `markus status`
- **Clear system RAM:** `markus freemem`

---

## 📖 In-Depth Project Explanation

### Why Pure Rust?
Historically, LLM execution pipelines have relied heavily on `llama.cpp` (a large C/C++ library) wrapped in Python bindings or messy bash scripts. While performant, this creates massive dependency headaches—users need specific GCC versions, CMake, build-essential, and gigabytes of compiled binaries just to chat with a model.

**Markus v3 completely eliminates this.** By leveraging HuggingFace's [Candle](https://github.com/huggingface/candle) (a minimalist ML framework for Rust), Markus parses GGUF files natively and executes quantized tensor math natively in Rust. This results in:
- **Memory Safety:** Rust’s borrow checker guarantees no segfaults or memory leaks.
- **Microscopic Footprint:** The entire compiled binary is around ~15MB. No giant shared libraries.
- **Portability:** If it compiles, it runs. No missing `.so` or `.dll` files.

### 🏗️ Architecture

Markus v3 is divided into a highly modular Cargo workspace:

#### 1. `markus-core` (The Heart)
- **GGUF Parser:** We built a custom zero-copy parser that reads `.gguf` binary files to extract layer metadata, tensor shapes, and quantization types directly.
- **Tokenizer:** Loads HuggingFace `tokenizer.json` logic for fast encoding/decoding.
- **Inference Pipeline:** A multithreaded generation loop that runs the model's forward pass. It spawns the generation on a dedicated Tokio blocking thread and streams raw tokens back through a `tokio::sync::mpsc` channel.
- **Model Dispatcher:** Automatically detects the architecture inside the GGUF (e.g., LLaMA, Phi-3, Qwen2) and loads the correct Transformer block definitions.

#### 2. `markus-server` (The API)
- Built on `Axum`, this crate provides an HTTP server mimicking the OpenAI API (`POST /v1/chat/completions`). 
- Because tokens stream over the `mpsc` channel, the server can easily chunk them into Server-Sent Events (SSE) for real-time web clients, or buffer them for blocking responses.

#### 3. `markus-tui` (The Interface)
- Replaces the original bash-based arrow menu with a beautiful, flicker-free terminal application built with `ratatui` and `crossterm`.
- Features isolated widgets: a Chat pane with word-wrapping, a Model Browser, and a System Dashboard that monitors RAM/CPU in real-time.

#### 4. `markus-cli` (The Glue)
- Powered by `clap`, this acts as the entrypoint. It parses your command line arguments, handles `stdout` printing for quick tasks (like `markus list`), and launches the TUI or Server when requested.

### Model Discovery & Caching
Markus scans standard directories (`~/.local/share/markus/models`, `~/.cache/huggingface`, Ollama folders, LM Studio folders) to find all `.gguf` models on your machine without requiring duplication.

---

## License

MIT License. See `LICENSE` for details.
