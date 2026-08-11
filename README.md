# Markus Engine — Pure Rust AI Model Manager (v3.0.0)

Markus is a fully independent, Rust-native Large Language Model (LLM) manager and inference engine. It replaces complex Python environments and C/C++ dependencies (like `llama.cpp`) with a blazingly fast, memory-safe, pure Rust stack powered by HuggingFace's `candle` framework.

## 🚀 Features

- **Pure Rust Inference**: Zero C/C++ dependencies (`llama.cpp` is no longer required).
- **Interactive TUI**: A beautiful Ratatui-based terminal UI for chatting, browsing models, and managing system resources.
- **Anywhere Access**: Run `markus` from any directory once installed.
- **OpenAI-Compatible Server**: Drop-in replacement for OpenAI endpoints (`POST /v1/chat/completions`).
- **GGUF Support**: Direct, zero-copy parsing and execution of `.gguf` model files.
- **Multi-Model Support**: Out-of-the-box support for LLaMA 1/2/3, Mistral, Phi-3, Qwen, Gemma, DeepSeek, and more.
- **HuggingFace Downloader**: Built-in async downloader to easily pull models via aliases or direct URLs.

---

## 📦 Installation

To get started, you will need [Rust](https://rustup.rs/) installed on your machine. The install scripts will build the engine from source and place the `markus` executable globally in your PATH (`~/.local/bin` for Linux/macOS or `~\.local\bin` for Windows).

### Linux & macOS
Open a terminal in the project directory and run:
```bash
chmod +x install.sh
./install.sh
```

### Windows
Open PowerShell in the project directory and run:
```powershell
.\install.ps1
```

*(Note: Depending on your execution policies, you may need to run `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass` first).*

---

## 🕹️ Usage

Once installed, you can simply type `markus` from **any directory** on your machine. 

### Launch the Interactive TUI (Default)
```bash
markus
```
*Navigating the TUI:* Use arrow keys (or `j`/`k`) to move, `Enter` to select, and `q` to quit.

### Command Line Mode (CLI)

You can also use `markus` for fast terminal commands:

- **List local models:**
  ```bash
  markus list
  ```
- **Chat with a model (by index or name):**
  ```bash
  markus run 1
  markus run qwen3
  ```
- **Run a single-shot prompt:**
  ```bash
  markus run 1 --prompt "Explain quantum computing in simple terms."
  ```
- **Download a new model from HuggingFace:**
  ```bash
  markus pull llama3.2
  ```
- **Start the OpenAI-compatible API Server:**
  ```bash
  markus serve 1 --port 8080
  ```
- **Show system RAM/GPU status:**
  ```bash
  markus status
  ```
- **Clear system RAM (drop caches/kill zombie processes):**
  ```bash
  markus freemem
  ```

---

## 🏗️ Architecture

Markus v3 is built as a highly modular Cargo workspace containing:
1. `markus-core`: The heart of the engine (GGUF parsing, Tokenizer, async Candle inference pipeline, Model Scanner).
2. `markus-server`: An Axum-based web server that provides streaming SSE and blocking JSON responses using standard OpenAI schemas.
3. `markus-tui`: A rich terminal application using `ratatui` and `crossterm`.
4. `markus-cli`: The unified binary entrypoint tying everything together with `clap`.

All ML mathematical operations and quantized tensor executions are fully handled natively by [Candle](https://github.com/huggingface/candle).

---

## ⚙️ Configuration

Configuration is automatically stored in `~/.config/markus/config.toml`. You can view or edit these settings by running:
```bash
markus config show
markus config edit
```
Settings include default context lengths, CPU threads, max tokens, repetition penalties, and more.

## License

MIT License. See `LICENSE` for details.
