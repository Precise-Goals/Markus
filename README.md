# MARKUS — AI Model Manager CLI

**MARKUS** is an all-in-one, globally available, sleek terminal UI and CLI tool for managing local LLMs, running conversational chat sessions, and hosting OpenAI-compatible REST APIs powered by `llama.cpp`.

---

## ⚡ One-Line Installation

Install Markus universally on any Linux/macOS machine with one command:

```bash
curl -fsSL https://raw.githubusercontent.com/<YOUR_USERNAME>/markus/main/install.sh | bash
```
*(Replace `<YOUR_USERNAME>` with your GitHub username once published)*

You can also install manually by cloning this repository:
```bash
git clone https://github.com/<YOUR_USERNAME>/markus.git
cd markus
sudo ./install.sh
```

---

## ✨ Features

- **Sleek Minimal Category TUI**: Flicker-free arrow-key terminal interface with categorized menus (`Start`, `Models`, `System`, `Quantize`, `Exit`).
- **Instant ESC / 'q' Back Key**: Press **`ESC`** or **`q`** at any time to instantly go back to the previous menu or exit cleanly.
- **Universal Model Scanning**: Automatically detects existing `.gguf` and `.safetensors` models from HuggingFace cache (`~/.cache/huggingface/hub`), Ollama (`~/.ollama/models`), LM Studio, and system-wide paths.
- **Interactive Chat with History**: Full multi-turn chat sessions (`-cnv`) with custom temperature, max tokens, and system prompts.
- **OpenAI-Compatible Server**: Instantly spin up a local REST API endpoint (`/v1/chat/completions`) compatible with OpenAI SDKs and tools.
- **Model Downloads & Shortcuts**: Pull GGUF models directly via HuggingFace shortcuts (`markus pull qwen2.5`, `markus pull llama3.2`, etc.) or custom URLs.
- **Memory & Cache Cleaner**: One-click `freemem` command to kill running inference servers and drop Linux kernel page/dentry/inode memory caches.
- **Model Quantization & Benchmarking**: Re-quantize models (`Q4_K_M`, `Q8_0`, `F16`, etc.) and test generation tokens/sec performance.
- **Universal Llama Backend Support**: Automatically supports standard standalone `llama-cli`/`llama-server` binaries as well as all-in-one multi-call `llama-cpp-bin` wrappers.

---

## 🚀 Quick Start

### Interactive Menu
Run without arguments to launch the sleek arrow-key menu:
```bash
markus
```

### Direct Commands
```bash
# Pull a model
markus pull qwen2.5
markus pull llama3.2
markus pull hf:bartowski/Qwen2.5-7B-Instruct-GGUF:Qwen2.5-7B-Instruct-Q4_K_M.gguf

# Interactive chat
markus run qwen2.5

# Start OpenAI-compatible HTTP API server
markus serve qwen2.5 --host 0.0.0.0 --port 8080

# List detected models on disk
markus list

# Free memory / clear cache
markus freemem

# Benchmark generation speed
markus bench qwen2.5
```

---

## 🛠 Configuration

Configuration is stored at `~/.config/markus/config.sh`. You can manage it from the interactive TUI or directly:
```bash
markus config show
markus config set THREADS 32
markus config set CTX_SIZE 8192
markus config set TEMPERATURE 0.7
```

---

## 📄 License

MIT License
