#!/usr/bin/env bash
# ==============================================================================
#  markus — AI Model Manager CLI  |  v2.1.0  |  MIT License
#  https://github.com/your-username/markus
#
#  Features:
#    • Clean, minimal arrow-key TUI menu (no flicker, no informal emojis)
#    • Structured categories: Start, Models, System, Quantize
#    • Full conversation history via JSON temp file
#    • Filesystem scan: HuggingFace cache, Ollama, LM Studio, system-wide
#    • OpenAI-compatible server mode (/v1/chat/completions)
#    • Model download from HuggingFace / direct URL / shortcuts
#    • RAM/cache clearing, model quantization, benchmarking
#    • Per-user config (~/.config/markus/config.sh)
# ==============================================================================
MARKUS_VERSION="2.1.0"
MARKUS_CONFIG_DIR="${HOME}/.config/markus"
MARKUS_MODELS_DIR="${HOME}/.local/share/markus/models"
MARKUS_LOG_DIR="${HOME}/.local/share/markus/logs"
MARKUS_CACHE_DIR="${HOME}/.cache/markus"
MARKUS_SCAN_CACHE="${MARKUS_CACHE_DIR}/model_scan.cache"
MARKUS_CONFIG_FILE="${MARKUS_CONFIG_DIR}/config.sh"

# ─── Cross-Platform OS & Hardware Helpers ─────────────────────────────────────
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "darwin" ;;
        CYGWIN*|MINGW*|MSYS*|Windows_NT*) echo "windows" ;;
        *)          echo "linux" ;;
    esac
}

get_cpu_cores() {
    if command -v nproc >/dev/null 2>&1; then
        nproc 2>/dev/null
    elif command -v sysctl >/dev/null 2>&1; then
        sysctl -n hw.ncpu 2>/dev/null || echo 4
    elif [[ -n "${NUMBER_OF_PROCESSORS:-}" ]]; then
        echo "$NUMBER_OF_PROCESSORS"
    else
        echo 4
    fi
}

get_ram_usage_string() {
    if command -v free >/dev/null 2>&1; then
        free -h 2>/dev/null | awk '/^Mem:/{print $2}' || echo "N/A"
    elif command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -Command "[math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1).ToString() + 'GB'" 2>/dev/null | tr -d '\r'
    elif command -v sysctl >/dev/null 2>&1; then
        sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0fGB\n", $1/(1024*1024*1024)}'
    else
        echo "N/A"
    fi
}

get_ram_avail_string() {
    if command -v free >/dev/null 2>&1; then
        free -h 2>/dev/null | awk '/^Mem:/{print $7}' || echo "N/A"
    elif command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -Command "[math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1024 / 1024, 1).ToString() + 'GB'" 2>/dev/null | tr -d '\r'
    elif command -v sysctl >/dev/null 2>&1 && command -v vm_stat >/dev/null 2>&1; then
        local psize; psize=$(vm_stat | grep "Page size of" | awk '{print $8}')
        local free_pages; free_pages=$(vm_stat | grep "Pages free:" | awk '{print $3}' | tr -d '.')
        echo $(( free_pages * psize / 1024 / 1024 / 1024 ))GB 2>/dev/null || echo "N/A"
    else
        echo "N/A"
    fi
}

get_cpu_model() {
    if [[ -r /proc/cpuinfo ]]; then
        grep "model name" /proc/cpuinfo 2>/dev/null | head -1 | sed 's/.*: //'
    elif command -v sysctl >/dev/null 2>&1; then
        sysctl -n machdep.cpu.brand_string 2>/dev/null || sysctl -n hw.model 2>/dev/null
    elif command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -Command "(Get-CimInstance Win32_Processor).Name" 2>/dev/null | tr -d '\r' | head -1
    else
        echo "Unknown CPU"
    fi
}

DEFAULT_THREADS=$(get_cpu_cores)
DEFAULT_CTX=4096
DEFAULT_BATCH=512
DEFAULT_PORT=8080
DEFAULT_HOST="127.0.0.1"
DEFAULT_TEMP=0.7
DEFAULT_TOP_P=0.9
DEFAULT_TOP_K=40
DEFAULT_REPEAT_PENALTY=1.1
DEFAULT_MAX_TOKENS=-1
DEFAULT_GPU_LAYERS=0

# ─── Color Palette (Professional & Sleek) ────────────────────────────────────
R="\033[0m"           # Reset
B="\033[1m"           # Bold
D="\033[2m"           # Dim
IT="\033[3m"          # Italic

# Brand = Bright Red
BRAND="\033[91m"
# Accent = Bright Cyan / Blue
ACCENT="\033[96m"
HL="\033[94m"
# Navigation / Selection = Bright Yellow
NAV="\033[93m"
SEL_TEXT="\033[93m"

FG_RED="\033[91m"
FG_GRN="\033[92m"
FG_YLW="\033[93m"
FG_BLU="\033[94m"
FG_MGT="\033[95m"
FG_CYN="\033[96m"
FG_WHT="\033[97m"

# ─── Logging ──────────────────────────────────────────────────────────────────
info()    { echo -e "${ACCENT}${B}  ◆  ${R}${FG_WHT}$*${R}"; }
success() { echo -e "${FG_GRN}${B}  ✔  ${R}${FG_GRN}$*${R}"; }
warn()    { echo -e "${FG_YLW}${B}  ⚠  ${R}${FG_YLW}$*${R}"; }
error()   { echo -e "${FG_RED}${B}  ✘  ${R}${FG_RED}$*${R}" >&2; }
step()    { echo -e "${HL}${B}  →  ${R}${HL}$*${R}"; }
dim()     { echo -e "${D}     $*${R}"; }
label()   { echo -e "  ${ACCENT}$*${R}"; }

# ─── Banners ──────────────────────────────────────────────────────────────────
print_banner() {
    clear
    echo -e "${BRAND}${B}"
    cat << 'BANNER'
  ███╗   ███╗ █████╗ ██████╗ ██╗  ██╗██╗   ██╗███████╗
  ████╗ ████║██╔══██╗██╔══██╗██║ ██╔╝██║   ██║██╔════╝
  ██╔████╔██║███████║██████╔╝█████╔╝ ██║   ██║███████╗
  ██║╚██╔╝██║██╔══██║██╔══██╗██╔═██╗ ██║   ██║╚════██║
  ██║ ╚═╝ ██║██║  ██║██║  ██║██║  ██╗╚██████╔╝███████║
  ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝
BANNER
    echo -e "${R}${D}  AI Model Manager — v${MARKUS_VERSION} — $(uname -m) — $(get_cpu_cores) cores — $(get_ram_usage_string) RAM${R}"
    echo -e "${D}  ──────────────────────────────────────────────────────────${R}"
    echo
}

print_small_banner() {
    echo -e "${BRAND}${B}  ▸ MARKUS${R}${ACCENT}  AI Model Manager${R} ${D}v${MARKUS_VERSION}${R}"
    echo -e "${D}  ──────────────────────────────────────────────────────────${R}"
}

# ─── Spinner ──────────────────────────────────────────────────────────────────
SPINNER_PID=""
spinner_start() {
    local msg="${1:-Working...}"
    local frames=("⣾" "⣽" "⣻" "⢿" "⡿" "⣟" "⣯" "⣷")
    ( local i=0
      while true; do
          printf "\r${ACCENT}  ${frames[$i]} ${R}${msg}   "
          i=$(( (i+1) % ${#frames[@]} ))
          sleep 0.1
      done ) &
    SPINNER_PID=$!
    disown "$SPINNER_PID"
}
spinner_stop() {
    if [[ -n "$SPINNER_PID" ]]; then
        kill "$SPINNER_PID" 2>/dev/null
        wait "$SPINNER_PID" 2>/dev/null
        SPINNER_PID=""
        printf "\r\033[2K"
    fi
}
trap 'spinner_stop; tput cnorm 2>/dev/null; tput rmcup 2>/dev/null' EXIT

# ─── Professional Flicker-Free Arrow-Key Menu ────────────────────────────────
# Usage: arrow_menu "Title" "Subtitle" label1 desc1 label2 desc2 ...
MENU_RESULT=0

arrow_menu() {
    local title="$1"; local subtitle="$2"; shift 2
    local -a items=("$@")
    local n=$(( ${#items[@]} / 2 ))
    local sel=0
    local ESC=$'\033'

    tput smcup 2>/dev/null   # save alternate screen
    tput civis 2>/dev/null   # hide cursor

    # Draw banner and title
    print_banner
    echo -e "  ${BRAND}${B}${title}${R}"
    [[ -n "$subtitle" ]] && echo -e "  ${ACCENT}${subtitle}${R}"
    echo

    _draw_single_item() {
        local i=$1
        local is_sel=$2
        local lbl="${items[$((i*2))]}"
        local dsc="${items[$((i*2+1))]}"
        if [[ $is_sel -eq 1 ]]; then
            printf "\033[2K\r  ${NAV}${B} ▸  %-18s${R}${ACCENT}  %s${R}" "$lbl" "$dsc"
        else
            printf "\033[2K\r  ${D}    %-18s${R}${FG_WHT}  %s${R}" "$lbl" "$dsc"
        fi
    }

    _draw_menu_items() {
        for (( i=0; i<n; i++ )); do
            _draw_single_item "$i" "$(( i == sel ? 1 : 0 ))"
            printf "\n"
        done
        printf "\033[2K\r\n"
        printf "\033[2K\r  ${NAV}${B}  ↑↓${R}${ACCENT} Navigate   ${NAV}${B}Enter${R}${ACCENT} Select   ${NAV}${B}q${R}${ACCENT} Quit   ${NAV}${B}1-9${R}${ACCENT} Jump${R}\n"
        printf "\033[2K\r\n"
    }

    _draw_menu_items

    while true; do
        local key
        IFS= read -rsn1 key

        local old_sel=$sel
        if [[ "$key" == "$ESC" ]]; then
            local seq=""
            IFS= read -rsn2 -t 0.05 seq || true
            if [[ -z "$seq" ]]; then
                tput rmcup 2>/dev/null
                tput cnorm 2>/dev/null
                MENU_RESULT=$(( n - 1 ))
                return 0
            fi
            case "$seq" in
                "[A"|"OA") (( sel = (sel - 1 + n) % n )) ;;   # Up
                "[B"|"OB") (( sel = (sel + 1) % n ))     ;;   # Down
                "[H"|"OH") sel=0 ;;                           # Home
                "[F"|"OF") sel=$(( n - 1 )) ;;               # End
            esac
        elif [[ "$key" == "" || "$key" == $'\n' || "$key" == $'\r' ]]; then
            tput rmcup 2>/dev/null
            tput cnorm 2>/dev/null
            MENU_RESULT=$sel
            return 0
        elif [[ "$key" == "q" || "$key" == "Q" ]]; then
            tput rmcup 2>/dev/null
            tput cnorm 2>/dev/null
            MENU_RESULT=$(( n - 1 ))
            return 0
        elif [[ "$key" == "k" ]]; then (( sel = (sel - 1 + n) % n ))
        elif [[ "$key" == "j" ]]; then (( sel = (sel + 1) % n ))
        elif [[ "$key" =~ ^[1-9]$ ]]; then
            local jump=$(( key - 1 ))
            if [[ $jump -lt $n ]]; then
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                MENU_RESULT=$jump; return 0
            fi
        fi

        if [[ $sel -ne $old_sel ]]; then
            # Move cursor up and redraw old_sel
            printf "\033[%dA" "$(( n + 3 - old_sel ))"
            _draw_single_item "$old_sel" 0
            printf "\033[%dB" "$(( n + 3 - old_sel ))"
            
            # Move cursor up and redraw new sel
            printf "\033[%dA" "$(( n + 3 - sel ))"
            _draw_single_item "$sel" 1
            printf "\033[%dB" "$(( n + 3 - sel ))"
        fi
    done
}

# ─── Init ─────────────────────────────────────────────────────────────────────
init_dirs() {
    mkdir -p "$MARKUS_CONFIG_DIR" "$MARKUS_MODELS_DIR" "$MARKUS_LOG_DIR" "$MARKUS_CACHE_DIR"
    if [[ ! -f "$MARKUS_CONFIG_FILE" ]]; then
        cat > "$MARKUS_CONFIG_FILE" << CFGEOF
# markus configuration
THREADS=$(get_cpu_cores)
CTX_SIZE=4096
BATCH_SIZE=512
GPU_LAYERS=0
TEMPERATURE=0.7
TOP_P=0.9
TOP_K=40
REPEAT_PENALTY=1.1
MAX_TOKENS=-1
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
CFGEOF
    fi
    # shellcheck source=/dev/null
    source "$MARKUS_CONFIG_FILE" 2>/dev/null || true
}

# ─── Backend Detection ────────────────────────────────────────────────────────
MARKUS_SERVER_BIN=""
MARKUS_CLI_BIN=""

detect_backend_quiet() {
    [[ -n "$MARKUS_SERVER_BIN" || -n "$MARKUS_CLI_BIN" ]] && return 0

    local win_appdata="${LOCALAPPDATA:-$HOME/AppData/Local}"
    local win_userprofile="${USERPROFILE:-$HOME}"

    local sv_candidates=(
        "/usr/local/lib/ollama/llama-server"
        "/usr/local/bin/llama-server"
        "/usr/bin/llama-server"
        "/opt/llama.cpp/llama-server"
        "${HOME}/.local/bin/llama-server"
        "${HOME}/.local/bin/llama-server.exe"
        "${HOME}/.local/share/markus/bin/llama-server"
        "${HOME}/.local/share/markus/bin/llama-server.exe"
        "${win_appdata}/Programs/Ollama/ollama.exe"
        "${win_appdata}/Programs/Ollama/lib/ollama/llama-server.exe"
        "${win_appdata}/LM-Studio/lib/llama-server.exe"
        "$(which llama-server 2>/dev/null || true)"
        "$(which llama-server.exe 2>/dev/null || true)"
        "$(which ollama 2>/dev/null || true)"
        "$(which ollama.exe 2>/dev/null || true)"
    )
    local cli_candidates=(
        "/usr/local/lib/ollama/llama-cli"
        "/usr/local/bin/llama-cli"
        "/usr/bin/llama-cli"
        "/opt/llama.cpp/llama-cli"
        "${HOME}/.local/bin/llama-cli"
        "${HOME}/.local/bin/llama-cli.exe"
        "${HOME}/.local/bin/llama-cpp-bin"
        "${HOME}/.local/bin/llama-cpp-bin.exe"
        "${HOME}/.local/share/markus/bin/llama-cli"
        "${HOME}/.local/share/markus/bin/llama-cli.exe"
        "${win_appdata}/Programs/Ollama/lib/ollama/llama-cli.exe"
        "$(which llama-cli 2>/dev/null || true)"
        "$(which llama-cli.exe 2>/dev/null || true)"
    )

    for b in "${sv_candidates[@]}"; do
        [[ -n "$b" && -f "$b" ]] && { MARKUS_SERVER_BIN="$b"; break; }
    done
    for b in "${cli_candidates[@]}"; do
        [[ -n "$b" && -f "$b" ]] && { MARKUS_CLI_BIN="$b"; break; }
    done

    if [[ "$MARKUS_SERVER_BIN" == */ollama/* || "$MARKUS_CLI_BIN" == */ollama/* ]]; then
        local ol_dir
        ol_dir=$(dirname "${MARKUS_SERVER_BIN:-$MARKUS_CLI_BIN}")
        export PATH="${ol_dir}:${PATH}"
        export LD_LIBRARY_PATH="${ol_dir}:${LD_LIBRARY_PATH:-}"
    fi
}

detect_backend() {
    detect_backend_quiet
    [[ -n "$MARKUS_SERVER_BIN" || -n "$MARKUS_CLI_BIN" ]] && return 0

    if [[ ! -t 0 ]]; then
        return 0
    fi

    warn "No llama.cpp binary found on this system."
    echo -e "  ${HL}1)${R} Build from source   ${HL}2)${R} Download pre-built   ${HL}3)${R} Set path manually"
    echo -ne "\n  ${ACCENT}Choice [1/2/3]:${R} "; read -r ch
    case "$ch" in
        1) _build_llama_cpp ;;
        2) _download_llama_binary ;;
        3)
            echo -ne "  ${ACCENT}Path to llama-server:${R} "; read -r cp
            [[ -f "$cp" ]] && MARKUS_SERVER_BIN="$cp" \
                           || { error "File not found or not executable: $cp"; exit 1; }
            ;;
        *) error "Invalid choice"; exit 1 ;;
    esac

    if [[ "$MARKUS_SERVER_BIN" == */ollama/* || "$MARKUS_CLI_BIN" == */ollama/* ]]; then
        local ol_dir
        ol_dir=$(dirname "${MARKUS_SERVER_BIN:-$MARKUS_CLI_BIN}")
        export PATH="${ol_dir}:${PATH}"
        export LD_LIBRARY_PATH="${ol_dir}:${LD_LIBRARY_PATH:-}"
    fi
}

_build_llama_cpp() {
    local d="${HOME}/.local/share/markus/src/llama.cpp"
    info "Building llama.cpp from source..."
    mkdir -p "${HOME}/.local/share/markus/src"
    [[ ! -d "$d" ]] && git clone --depth 1 https://github.com/ggml-org/llama.cpp "$d"
    cmake -S "$d" -B "$d/build" -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=ON \
        -DLLAMA_BUILD_SERVER=ON 2>&1 | tail -3
    cmake --build "$d/build" -j"$(get_cpu_cores)" 2>&1 | tail -5
    local bin_dir="${HOME}/.local/share/markus/bin"
    mkdir -p "$bin_dir"
    find "$d/build" -type f \( -name "llama-server*" -o -name "llama-cli*" -o -name "*.dll" -o -name "*.so*" -o -name "*.dylib" \) -exec cp {} "$bin_dir/" \; 2>/dev/null || true
    chmod +x "$bin_dir"/llama-* "$bin_dir"/*.exe 2>/dev/null || true
    MARKUS_SERVER_BIN="$bin_dir/llama-server"
    [[ -f "$bin_dir/llama-server.exe" ]] && MARKUS_SERVER_BIN="$bin_dir/llama-server.exe"
    success "Built: $MARKUS_SERVER_BIN"
}

_download_llama_binary() {
    info "Fetching latest llama.cpp release..."
    local tag
    tag=$(curl -fsSL "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
    [[ -z "$tag" ]] && { error "Cannot fetch release info"; exit 1; }

    local os arch fn pattern
    os=$(detect_os)
    arch=$(uname -m)
    case "$os" in
        windows)
            if [[ "$arch" == "x86_64" || "$arch" == "amd64" || "$arch" == "i686" ]]; then
                pattern="win-avx2-x64.zip"
                fn="llama-${tag}-bin-win-avx2-x64.zip"
            else
                pattern="win-arm64.zip"
                fn="llama-${tag}-bin-win-arm64.zip"
            fi
            ;;
        darwin)
            if [[ "$arch" == "arm64" || "$arch" == "aarch64" ]]; then
                pattern="macos-arm64.zip"
                fn="llama-${tag}-bin-macos-arm64.zip"
            else
                pattern="macos-x64.zip"
                fn="llama-${tag}-bin-macos-x64.zip"
            fi
            ;;
        *)
            if [[ "$arch" == "aarch64" || "$arch" == "arm64" ]]; then
                pattern="ubuntu-v8-arm64.tar.gz"
                fn="llama-${tag}-bin-ubuntu-v8-arm64.tar.gz"
            else
                pattern="ubuntu-22.04-x64.tar.gz"
                fn="llama-${tag}-bin-ubuntu-22.04-x64.tar.gz"
            fi
            ;;
    esac

    local tmp; tmp=$(mktemp -d)
    step "Downloading ${fn} for ${os} (${arch})..."
    if ! curl -L --progress-bar --fail "https://github.com/ggml-org/llama.cpp/releases/download/${tag}/${fn}" -o "$tmp/$fn"; then
        warn "Direct asset download failed, attempting search in release assets..."
        local asset_url
        asset_url=$(curl -fsSL "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest" | grep -i '"browser_download_url":' | grep -i "$pattern" | head -1 | sed 's/.*"browser_download_url": *"//;s/".*//')
        [[ -z "$asset_url" ]] && { error "Could not find compatible binary asset for ${os}/${arch}"; rm -rf "$tmp"; exit 1; }
        curl -L --progress-bar --fail "$asset_url" -o "$tmp/archive.zip"
        fn="archive.zip"
    fi

    local bin_dir="${HOME}/.local/share/markus/bin"
    mkdir -p "$bin_dir"

    step "Extracting to ${ACCENT}${bin_dir}${R}..."
    if [[ "$fn" == *.zip ]]; then
        unzip -qo "$tmp/$fn" -d "$tmp/x"
    else
        mkdir -p "$tmp/x"
        tar -xzf "$tmp/$fn" -C "$tmp/x" 2>/dev/null || unzip -qo "$tmp/$fn" -d "$tmp/x"
    fi

    find "$tmp/x" -type f \( -name "llama-server*" -o -name "llama-cli*" -o -name "*.dll" -o -name "*.so*" -o -name "*.dylib" \) -exec cp {} "$bin_dir/" \; 2>/dev/null || true
    chmod +x "$bin_dir"/llama-* "$bin_dir"/*.exe 2>/dev/null || true

    if [[ -f "$bin_dir/llama-server.exe" ]]; then
        MARKUS_SERVER_BIN="$bin_dir/llama-server.exe"
    elif [[ -x "$bin_dir/llama-server" ]]; then
        MARKUS_SERVER_BIN="$bin_dir/llama-server"
    else
        MARKUS_SERVER_BIN=$(find "$bin_dir" -name "*server*" -type f | head -1)
    fi

    if [[ -f "$bin_dir/llama-cli.exe" ]]; then
        MARKUS_CLI_BIN="$bin_dir/llama-cli.exe"
    elif [[ -x "$bin_dir/llama-cli" ]]; then
        MARKUS_CLI_BIN="$bin_dir/llama-cli"
    fi

    rm -rf "$tmp"
    [[ -n "$MARKUS_SERVER_BIN" ]] && success "Downloaded: $MARKUS_SERVER_BIN" || { error "Failed to locate llama-server after extraction"; exit 1; }
}

_llama_run_subcmd() {
    local subcmd="$1"; shift
    local bin="$1"; shift
    local ol_dir
    ol_dir=$(dirname "$bin")
    if [[ "$(basename "$bin")" == "llama-cpp-bin"* ]] || "$bin" --help 2>&1 | grep -q "Available commands:"; then
        PATH="${ol_dir}:${PATH}" LD_LIBRARY_PATH="${ol_dir}:${LD_LIBRARY_PATH:-}" "$bin" "$subcmd" "$@"
    else
        PATH="${ol_dir}:${PATH}" LD_LIBRARY_PATH="${ol_dir}:${LD_LIBRARY_PATH:-}" "$bin" "$@"
    fi
}

_llama_run() {
    _llama_run_subcmd "cli" "$@"
}

# ─── Filesystem Scanner ───────────────────────────────────────────────────────
FOUND_MODELS=()

scan_filesystem() {
    local force="${1:-0}"
    if [[ "$force" != "1" && -f "$MARKUS_SCAN_CACHE" ]]; then
        local age
        age=$(( $(date +%s) - $(stat -c %Y "$MARKUS_SCAN_CACHE" 2>/dev/null || stat -f %m "$MARKUS_SCAN_CACHE" 2>/dev/null || echo 0) ))
        if [[ $age -lt 21600 && $age -ge 0 ]]; then
            mapfile -t FOUND_MODELS < "$MARKUS_SCAN_CACHE"
            [[ ${#FOUND_MODELS[@]} -gt 0 ]] && return 0
        fi
    fi

    spinner_start "Scanning filesystem for model files..."
    local tmp; tmp=$(mktemp)

    local win_appdata="${LOCALAPPDATA:-$HOME/AppData/Local}"
    local win_userprofile="${USERPROFILE:-$HOME}"

    local priority_roots=(
        "$MARKUS_MODELS_DIR"
        "${HOME}/.cache/huggingface/hub"
        "${win_userprofile}/.cache/huggingface/hub"
        "${HOME}/.ollama/models"
        "${win_userprofile}/.ollama/models"
        "${HOME}/.lmstudio/models"
        "${win_userprofile}/.lmstudio/models"
        "${HOME}/.cache/lm-studio/models"
        "${win_appdata}/Programs/Ollama/models"
        "${win_appdata}/LM-Studio/models"
        "/usr/share/ollama/.ollama/models"
        "/root/.ollama/models"
        "/var/lib/ollama/models"
        "${HOME}/models"
        "${HOME}/Models"
        "/opt/models"
        "/srv/models"
        "/data/models"
        "/data"
        "/mnt"
        "/media"
    )

    for root in "${priority_roots[@]}"; do
        [[ -d "$root" ]] && find "$root" \
            \( -path "*/.git" -o -path "*/node_modules" -o -path "*/__pycache__" \) -prune \
            -o -type f \( -name "*.gguf" -o -name "*.ggml" \
                          -o -name "*.safetensors" -o -name "*.bin" \) \
            -size +10M -print 2>/dev/null
    done >> "$tmp"

    if [[ "$(detect_os)" == "linux" ]]; then
        find / \( -path "*/proc" -o -path "*/sys" -o -path "*/dev" \
                  -o -path "*/.git" -o -path "*/snap/core*" \
                  -o -path "*/run"  -o -path "*/boot" \
                  -o -path "*/lib/modules" \) -prune \
            -o -type f -name "*.gguf" -size +10M -print 2>/dev/null >> "$tmp"
    fi

    sort -u "$tmp" > "$MARKUS_SCAN_CACHE"
    mapfile -t FOUND_MODELS < "$MARKUS_SCAN_CACHE"
    rm -f "$tmp"
    spinner_stop
}

resolve_model() {
    local inp="$1"
    [[ -f "$inp" ]] && { echo "$inp"; return 0; }
    if [[ "$inp" =~ ^[0-9]+$ ]]; then
        scan_filesystem
        local idx=$(( inp - 1 ))
        if [[ $idx -ge 0 && $idx -lt ${#FOUND_MODELS[@]} ]]; then
            echo "${FOUND_MODELS[$idx]}"; return 0
        fi
        error "Index $inp out of range (1–${#FOUND_MODELS[@]})"; exit 1
    fi
    scan_filesystem
    for m in "${FOUND_MODELS[@]}"; do
        [[ "${m,,}" == *"${inp,,}"* ]] && { echo "$m"; return 0; }
    done
    error "Model not found: '$inp'  →  run 'markus list'" >&2
    exit 1
}

# ─── Model Commands ───────────────────────────────────────────────────────────
cmd_list() {
    print_small_banner; echo
    info "Scanning for models..."
    scan_filesystem

    if [[ ${#FOUND_MODELS[@]} -eq 0 ]]; then
        warn "No model files found on this system."
        dim "Download one:  markus pull <name>"
        dim "Rescan:        markus scan --force"
        return
    fi

    echo -e "\n${B}${FG_WHT}  ┌─ Found ${ACCENT}${#FOUND_MODELS[@]}${R}${B}${FG_WHT} model(s) ────────────────────────────────────────────────┐${R}"
    local i=1
    for m in "${FOUND_MODELS[@]}"; do
        [[ -f "$m" ]] || continue
        local sz; sz=$(du -sh "$m" 2>/dev/null | cut -f1)
        local ext="${m##*.}"
        local col="$FG_WHT"
        [[ "$ext" == "gguf" ]] && col="$FG_GRN"
        [[ "$ext" == "safetensors" ]] && col="${FG_MGT}"
        printf "  ${D}│${R}  ${NAV}${B}%3s${R}  ${D}%-8s${R}  ${col}${B}%-40s${R}  ${D}%s${R}\n" \
            "$i" "$sz" "$(basename "$m")" "$(dirname "$m")"
        (( i++ ))
    done
    echo -e "${D}  └───────────────────────────────────────────────────────────────────────┘${R}"
    echo
    dim "markus run <# | name>   to chat"
    dim "markus scan --force     to refresh list"
    echo
}

cmd_scan() {
    local force=0
    [[ "${1:-}" == "--force" || "${1:-}" == "-f" ]] && force=1
    print_small_banner; echo
    if [[ $force -eq 1 ]]; then
        info "Force-scanning filesystem..."
        rm -f "$MARKUS_SCAN_CACHE"
    else
        info "Scanning filesystem  ${D}(--force to ignore cache)${R}"
    fi
    scan_filesystem "$force"
    success "Found ${#FOUND_MODELS[@]} model file(s)"
    echo
    local i=1
    for m in "${FOUND_MODELS[@]}"; do
        [[ -f "$m" ]] || continue
        local sz; sz=$(du -sh "$m" 2>/dev/null | cut -f1)
        local ext="${m##*.}"
        local col="$FG_WHT"; [[ "$ext" == "gguf" ]] && col="$FG_GRN"
        printf "  ${NAV}${B}%3d.${R}  ${col}%-45s${R}  ${D}%s${R}\n" \
            "$i" "$(basename "$m")" "$sz"
        printf "       ${ACCENT}%s${R}\n" "$m"
        (( i++ ))
    done
    echo
    dim "Cache: $MARKUS_SCAN_CACHE  (TTL: 6h)"
}

cmd_info() {
    local inp="${1:-}"
    [[ -z "$inp" ]] && { error "Usage: markus info <model>"; exit 1; }
    local mp; mp=$(resolve_model "$inp")
    local sz; sz=$(du -sh "$mp" | cut -f1)
    local mt; mt=$(stat -c "%y" "$mp" | cut -d. -f1)
    local perms; perms=$(stat -c "%A" "$mp")
    local ext="${mp##*.}"
    echo
    echo -e "${BRAND}${B}  ╔═ Model Info ════════════════════════════════════════════╗${R}"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Name:"       "$(basename "$mp")"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Path:"       "$mp"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Size:"       "$sz"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Format:"     "${ext^^}"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Modified:"   "$mt"
    printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n"  "Permissions:" "$perms"

    if [[ "$ext" == "gguf" ]] && command -v python3 >/dev/null 2>&1; then
        local meta
        meta=$(python3 - "$mp" << 'PYEOF' 2>/dev/null
import sys, struct
p = sys.argv[1]
try:
    with open(p, 'rb') as f:
        if f.read(4) == b'GGUF':
            v = struct.unpack('<I', f.read(4))[0]
            t = struct.unpack('<Q', f.read(8))[0]
            k = struct.unpack('<Q', f.read(8))[0]
            print(f"GGUF v{v}  |  tensors={t}  |  kv_pairs={k}")
except Exception:
    pass
PYEOF
)
        [[ -n "$meta" ]] && printf "  ${D}║${R}  ${ACCENT}%-18s${R} %s\n" "GGUF Header:" "$meta"
    fi
    echo -e "  ${BRAND}${B}  ╚═════════════════════════════════════════════════════════╝${R}"
    echo
}

cmd_remove() {
    local inp="${1:-}"
    [[ -z "$inp" ]] && { error "Usage: markus remove <model>"; exit 1; }
    local mp; mp=$(resolve_model "$inp")
    echo
    echo -e "  ${FG_RED}${B}  ⚠  About to remove:${R}  $mp"
    echo -ne "  ${FG_RED}  Confirm? [y/N]:${R} "; read -r c
    if [[ "$c" == "y" || "$c" == "Y" ]]; then
        rm -f "$mp"
        success "Removed: $(basename "$mp")"
        rm -f "$MARKUS_SCAN_CACHE"
    else
        dim "Cancelled."
    fi
}

# ─── Chat Mode (Interactive CLI / Server) ────────────────────────────────────
cmd_run() {
    local mp="${1:-}"; local prompt="${2:-}"
    [[ -z "$mp" ]] && { error "No model specified"; exit 1; }
    detect_backend
    [[ -z "$MARKUS_SERVER_BIN" && -z "$MARKUS_CLI_BIN" ]] && { error "No llama.cpp binary found"; exit 1; }

    if [[ -n "$MARKUS_CLI_BIN" && -z "$prompt" ]]; then
        _run_via_cli "$mp"
    else
        _run_via_server "$mp" "$prompt"
    fi
}

_run_via_cli() {
    local mp="$1"
    local t="${OPT_THREADS:-${THREADS:-$DEFAULT_THREADS}}"
    local c="${OPT_CTX:-${CTX_SIZE:-$DEFAULT_CTX}}"
    local g="${OPT_GPU_LAYERS:-${GPU_LAYERS:-$DEFAULT_GPU_LAYERS}}"
    local tm="${OPT_TEMP:-${TEMPERATURE:-$DEFAULT_TEMP}}"
    echo
    echo -e "${BG_BLK}${BRAND}${B}  ╔═ CHAT ═══════════════════════════════════════════════╗  ${R}"
    printf "  ${ACCENT}%-20s${R} %s\n" "  Model:" "$(basename "$mp")"
    printf "  ${ACCENT}%-20s${R} ${NAV}%d threads  /  %d ctx  /  %.1f temp${R}\n" "  Settings:" "$t" "$c" "$tm"
    echo -e "${BG_BLK}${BRAND}${B}  ╚══════════════════════════════════════════════════════╝  ${R}"
    echo
    local int_flag=("-cnv")
    if ! _llama_run_subcmd "cli" "$MARKUS_CLI_BIN" --help 2>&1 | grep -q -e "-cnv" -e "--conversation"; then
        int_flag=("-i" "--interactive" "--interactive-first")
    fi
    local args=(
        --model "$mp" --threads "$t" --ctx-size "$c"
        --batch-size "${OPT_BATCH:-${BATCH_SIZE:-$DEFAULT_BATCH}}"
        --n-gpu-layers "$g" --temp "$tm"
        --top-p "${OPT_TOP_P:-${TOP_P:-$DEFAULT_TOP_P}}"
        --top-k "${OPT_TOP_K:-${TOP_K:-$DEFAULT_TOP_K}}"
        --repeat-penalty "${OPT_REPEAT_PENALTY:-${REPEAT_PENALTY:-$DEFAULT_REPEAT_PENALTY}}"
        "${int_flag[@]}"
    )
    [[ "${OPT_MAX_TOKENS:-${MAX_TOKENS:-$DEFAULT_MAX_TOKENS}}" != "-1" ]] && \
        args+=(--n-predict "${OPT_MAX_TOKENS:-${MAX_TOKENS:-$DEFAULT_MAX_TOKENS}}")
    [[ "${OPT_MLOCK:-0}" == "1" ]] && args+=(--mlock)
    [[ -n "${OPT_SYSTEM_PROMPT:-}" ]] && args+=(--system-prompt "$OPT_SYSTEM_PROMPT")
    dim "Type 'exit' or Ctrl+C to quit"
    echo
    _llama_run "$MARKUS_CLI_BIN" "${args[@]}"
}

_run_via_server() {
    local mp="$1"
    local init_prompt="${2:-}"
    local port="${OPT_PORT:-${SERVER_PORT:-$DEFAULT_PORT}}"
    local t="${OPT_THREADS:-${THREADS:-$DEFAULT_THREADS}}"
    local c="${OPT_CTX:-${CTX_SIZE:-$DEFAULT_CTX}}"
    local g="${OPT_GPU_LAYERS:-${GPU_LAYERS:-$DEFAULT_GPU_LAYERS}}"
    local tm="${OPT_TEMP:-${TEMPERATURE:-$DEFAULT_TEMP}}"
    local maxt="${OPT_MAX_TOKENS:-${MAX_TOKENS:-$DEFAULT_MAX_TOKENS}}"
    local sys_prompt="${OPT_SYSTEM_PROMPT:-You are a helpful AI assistant.}"

    while ss -tlnp 2>/dev/null | grep -q ":${port} "; do
        port=$(( port + 1 ))
    done

    spinner_start "Loading model into memory…"
    mkdir -p "$MARKUS_LOG_DIR"
    local lf="${MARKUS_LOG_DIR}/chat_$$_$(date +%Y%m%d_%H%M%S).log"

    _llama_run_subcmd "serve" "$MARKUS_SERVER_BIN" \
        --model "$mp" --threads "$t" --ctx-size "$c" \
        --n-gpu-layers "$g" --host "127.0.0.1" --port "$port" \
        --no-webui > "$lf" 2>&1 &
    local SV_PID=$!

    local att=0
    until curl -sf "http://127.0.0.1:${port}/health" >/dev/null 2>&1; do
        sleep 0.5
        att=$(( att + 1 ))
        if [[ $att -gt 120 ]]; then
            spinner_stop
            error "Server did not start within 60s.  Log: $lf"
            kill "$SV_PID" 2>/dev/null; exit 1
        fi
        if ! kill -0 "$SV_PID" 2>/dev/null; then
            spinner_stop
            error "Server process died.  Log: $lf"; exit 1
        fi
    done
    spinner_stop

    trap 'kill "$SV_PID" 2>/dev/null
          echo -e "\n${D}  [markus server stopped]${R}"
          exit 0' INT TERM

    echo
    echo -e "${BG_BLK}${BRAND}${B}  ╔═ CHAT ═══════════════════════════════════════════════╗  ${R}"
    printf "  ${ACCENT}%-20s${R} %s\n" "  Model:" "$(basename "$mp")"
    printf "  ${ACCENT}%-20s${R} ${NAV}%d threads  /  %d ctx  /  %.1f temp${R}\n" "  Settings:" "$t" "$c" "$tm"
    printf "  ${ACCENT}%-20s${R} ${D}localhost:%d${R}\n" "  API:" "$port"
    echo -e "${BG_BLK}${BRAND}${B}  ╚══════════════════════════════════════════════════════╝  ${R}"
    echo
    echo -e "  ${ACCENT}Slash commands:${R}  ${D}/exit  /clear  /info  /temp N  /tokens N  /system S  /save FILE  /help${R}"
    echo

    local hist_file; hist_file=$(mktemp /tmp/markus_history_XXXXXX.json)
    echo "[]" > "$hist_file"
    trap 'kill "$SV_PID" 2>/dev/null; rm -f "$hist_file"
          echo -e "\n${D}  [markus server stopped]${R}"
          exit 0' INT TERM

    if [[ -n "$init_prompt" ]]; then
        _chat_turn "$init_prompt" "$port" "$tm" "$maxt" "$sys_prompt" "$hist_file"
        rm -f "$hist_file"
        return
    fi

    local turn=0
    while true; do
        echo -ne "  ${ACCENT}${B}You:${R} "
        local inp
        if ! IFS= read -r inp; then break; fi
        [[ -z "$inp" ]] && continue

        case "$inp" in
            /exit|/quit|exit|quit)
                echo -e "\n${D}  Goodbye!${R}"; break ;;
            /clear)
                echo "[]" > "$hist_file"
                turn=0
                printf "\033[2J\033[H"
                print_small_banner; echo
                echo -e "  ${ACCENT}Slash commands:${R}  ${D}/exit  /clear  /info  /temp N  /tokens N  /system S  /save FILE  /help${R}"
                echo
                dim "[Conversation history cleared]"; echo
                continue ;;
            /info)
                echo
                printf "  ${ACCENT}%-16s${R} %s\n" "Model:"     "$(basename "$mp")"
                printf "  ${ACCENT}%-16s${R} ${NAV}%d${R}\n" "Threads:"   "$t"
                printf "  ${ACCENT}%-16s${R} ${NAV}%d${R}\n" "Context:"   "$c"
                printf "  ${ACCENT}%-16s${R} ${NAV}%.2f${R}\n" "Temp:"    "$tm"
                printf "  ${ACCENT}%-16s${R} ${NAV}%d${R}\n" "Max tokens:" "$maxt"
                printf "  ${ACCENT}%-16s${R} ${D}localhost:%d${R}\n" "API port:" "$port"
                printf "  ${ACCENT}%-16s${R} ${NAV}%d${R}\n" "Turns:"    "$turn"
                echo; continue ;;
            /temp\ *)
                tm="${inp#/temp }"
                echo -e "  ${D}Temperature → ${NAV}${tm}${R}"; echo; continue ;;
            /tokens\ *)
                maxt="${inp#/tokens }"
                echo -e "  ${D}Max tokens → ${NAV}${maxt}${R}"; echo; continue ;;
            /system\ *)
                sys_prompt="${inp#/system }"
                echo "[]" > "$hist_file"; turn=0
                echo -e "  ${D}System prompt updated. History cleared.${R}"; echo; continue ;;
            /save\ *)
                local sf="${inp#/save }"
                cp "$hist_file" "$sf"
                echo -e "  ${D}Conversation saved to: ${ACCENT}${sf}${R}"; echo; continue ;;
            /help)
                echo
                printf "  ${ACCENT}%-20s${R} %s\n" "/exit  /quit"   "Exit chat"
                printf "  ${ACCENT}%-20s${R} %s\n" "/clear"         "Clear conversation history"
                printf "  ${ACCENT}%-20s${R} %s\n" "/info"          "Show current settings"
                printf "  ${ACCENT}%-20s${R} %s\n" "/temp N"        "Set temperature (e.g. /temp 0.3)"
                printf "  ${ACCENT}%-20s${R} %s\n" "/tokens N"      "Set max tokens (-1 = unlimited)"
                printf "  ${ACCENT}%-20s${R} %s\n" "/system PROMPT" "Set system prompt (clears history)"
                printf "  ${ACCENT}%-20s${R} %s\n" "/save FILE"     "Save conversation JSON to file"
                echo; continue ;;
        esac

        _chat_turn "$inp" "$port" "$tm" "$maxt" "$sys_prompt" "$hist_file"
        turn=$(( turn + 1 ))
    done

    rm -f "$hist_file"
    kill "$SV_PID" 2>/dev/null
}

_chat_turn() {
    local user_msg="$1"
    local port="$2"
    local temp="$3"
    local maxt="$4"
    local sys_prompt="$5"
    local hist_file="$6"

    local messages_json
    messages_json=$(python3 - "$sys_prompt" "$hist_file" "$user_msg" << 'PYEOF'
import sys, json
sys_prompt, hist_file, user_msg = sys.argv[1], sys.argv[2], sys.argv[3]
try:
    with open(hist_file, 'r') as f: history = json.load(f)
except Exception: history = []
messages = [{"role": "system", "content": sys_prompt}] + history + [{"role": "user", "content": user_msg}]
print(json.dumps(messages))
PYEOF
)

    if [[ -z "$messages_json" ]]; then
        error "Failed to build messages JSON"; return 1
    fi

    echo -ne "\n  ${BRAND}${B}Markus:${R} "
    local reply_file; reply_file=$(mktemp /tmp/markus_reply_XXXXXX)

    curl -sfN "http://127.0.0.1:${port}/v1/chat/completions" \
        -H "Content-Type: application/json" \
        -d "{\"messages\":${messages_json},\"temperature\":${temp},\"stream\":true,\"max_tokens\":${maxt}}" \
        2>/dev/null | \
    python3 - "$reply_file" << 'PYEOF'
import sys, json
reply_file = sys.argv[1]
full_reply = []
for raw_line in sys.stdin:
    line = raw_line.strip()
    if not line.startswith("data: "): continue
    data = line[6:]
    if data == "[DONE]": break
    try:
        chunk = json.loads(data)
        token = chunk["choices"][0]["delta"].get("content", "")
        if token:
            print(token, end="", flush=True)
            full_reply.append(token)
    except Exception: pass
with open(reply_file, "w") as f: f.write("".join(full_reply))
PYEOF

    echo -e "\n"
    local reply; reply=$(cat "$reply_file" 2>/dev/null || echo "")
    rm -f "$reply_file"

    if [[ -n "$reply" ]]; then
        python3 - "$hist_file" "$user_msg" "$reply" << 'PYEOF'
import sys, json
hist_file, user_msg, reply = sys.argv[1], sys.argv[2], sys.argv[3]
try:
    with open(hist_file, 'r') as f: history = json.load(f)
except Exception: history = []
history.append({"role": "user", "content": user_msg})
history.append({"role": "assistant", "content": reply})
with open(hist_file, 'w') as f: json.dump(history, f, ensure_ascii=False, indent=2)
PYEOF
    fi
}

# ─── Serve ────────────────────────────────────────────────────────────────────
cmd_serve() {
    local mp="${1:-}"; [[ -z "$mp" ]] && { error "No model specified"; exit 1; }
    detect_backend
    [[ -z "$MARKUS_SERVER_BIN" ]] && { error "llama-server not found"; exit 1; }

    local host="${OPT_HOST:-${SERVER_HOST:-$DEFAULT_HOST}}"
    local port="${OPT_PORT:-${SERVER_PORT:-$DEFAULT_PORT}}"
    local t="${OPT_THREADS:-${THREADS:-$DEFAULT_THREADS}}"
    local c="${OPT_CTX:-${CTX_SIZE:-$DEFAULT_CTX}}"
    local g="${OPT_GPU_LAYERS:-${GPU_LAYERS:-$DEFAULT_GPU_LAYERS}}"
    local batch="${OPT_BATCH:-${BATCH_SIZE:-$DEFAULT_BATCH}}"

    echo
    echo -e "${BG_BLU}${FG_WHT}${B}  ┌─ SERVE ──────────────────────────────────────────────────┐  ${R}"
    printf "  ${ACCENT}%-20s${R} ${FG_GRN}%s${R}\n" "│  Model:"    "$(basename "$mp")"
    printf "  ${ACCENT}%-20s${R} ${FG_CYN}http://%s:%s${R}\n" "│  Endpoint:"  "$host" "$port"
    printf "  ${ACCENT}%-20s${R} ${NAV}%d${R}\n" "│  Threads:"    "$t"
    printf "  ${ACCENT}%-20s${R} ${NAV}%d${R}\n" "│  Context:"    "$c"
    printf "  ${ACCENT}%-20s${R} ${NAV}%d${R}\n" "│  GPU layers:" "$g"
    echo -e "${BG_BLU}${FG_WHT}${B}  └──────────────────────────────────────────────────────────┘  ${R}"
    echo
    info "OpenAI-compatible  →  ${FG_CYN}POST /v1/chat/completions${R}"
    dim "Ctrl+C to stop"
    echo

    mkdir -p "$MARKUS_LOG_DIR"
    local lf="${MARKUS_LOG_DIR}/server_$(date +%Y%m%d_%H%M%S).log"
    step "Logs → ${ACCENT}$lf${R}"
    echo

    local args=(
        --model "$mp" --threads "$t" --ctx-size "$c"
        --batch-size "$batch" --n-gpu-layers "$g"
        --host "$host" --port "$port"
    )
    [[ "${OPT_MLOCK:-0}"  == "1" ]] && args+=(--mlock)
    [[ -n "${OPT_SYSTEM_PROMPT:-}" ]] && args+=(--system-prompt "$OPT_SYSTEM_PROMPT")
    [[ "${OPT_VERBOSE:-0}" == "1" ]] && args+=(--verbose)
    [[ -n "${OPT_ALIAS:-}" ]] && args+=(--alias "$OPT_ALIAS")

    _llama_run_subcmd "serve" "$MARKUS_SERVER_BIN" "${args[@]}" 2>&1 | tee "$lf"
}

# ─── Pull ─────────────────────────────────────────────────────────────────────
declare -A MODEL_ALIASES=(
    ["llama3"]="bartowski/Meta-Llama-3-8B-Instruct-GGUF:Meta-Llama-3-8B-Instruct-Q4_K_M.gguf"
    ["llama3.1"]="bartowski/Meta-Llama-3.1-8B-Instruct-GGUF:Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf"
    ["llama3.2"]="bartowski/Llama-3.2-3B-Instruct-GGUF:Llama-3.2-3B-Instruct-Q4_K_M.gguf"
    ["llama3.3"]="bartowski/Llama-3.3-70B-Instruct-GGUF:Llama-3.3-70B-Instruct-Q4_K_M.gguf"
    ["mistral"]="TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q4_K_M.gguf"
    ["mistral-nemo"]="bartowski/Mistral-Nemo-Instruct-2407-GGUF:Mistral-Nemo-Instruct-2407-Q4_K_M.gguf"
    ["phi3"]="microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf"
    ["phi3.5"]="bartowski/Phi-3.5-mini-instruct-GGUF:Phi-3.5-mini-instruct-Q4_K_M.gguf"
    ["phi4"]="bartowski/phi-4-GGUF:phi-4-Q4_K_M.gguf"
    ["gemma2"]="bartowski/gemma-2-9b-it-GGUF:gemma-2-9b-it-Q4_K_M.gguf"
    ["gemma3"]="lmstudio-community/gemma-3-4b-it-GGUF:gemma-3-4b-it-Q4_K_M.gguf"
    ["qwen2.5"]="bartowski/Qwen2.5-7B-Instruct-GGUF:Qwen2.5-7B-Instruct-Q4_K_M.gguf"
    ["qwen3"]="Qwen/Qwen3-8B-GGUF:Qwen3-8B-Q4_K_M.gguf"
    ["deepseek-r1"]="bartowski/DeepSeek-R1-Distill-Llama-8B-GGUF:DeepSeek-R1-Distill-Llama-8B-Q4_K_M.gguf"
    ["deepseek-r1-70b"]="bartowski/DeepSeek-R1-Distill-Llama-70B-GGUF:DeepSeek-R1-Distill-Llama-70B-Q4_K_M.gguf"
    ["codellama"]="TheBloke/CodeLlama-13B-Instruct-GGUF:codellama-13b-instruct.Q4_K_M.gguf"
    ["starcoder2"]="bartowski/starcoder2-15b-GGUF:starcoder2-15b-Q4_K_M.gguf"
    ["tinyllama"]="TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF:tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
    ["smollm2"]="bartowski/SmolLM2-1.7B-Instruct-GGUF:SmolLM2-1.7B-Instruct-Q4_K_M.gguf"
    ["vicuna"]="TheBloke/Vicuna-13B-v1.5-GGUF:vicuna-13b-v1.5.Q4_K_M.gguf"
    ["neural-chat"]="TheBloke/neural-chat-7B-v3-3-GGUF:neural-chat-7b-v3-3.Q4_K_M.gguf"
    ["openchat"]="TheBloke/openchat-3.5-1210-GGUF:openchat-3.5-1210.Q4_K_M.gguf"
    ["zephyr"]="TheBloke/zephyr-7B-beta-GGUF:zephyr-7b-beta.Q4_K_M.gguf"
    ["orca-mini"]="TheBloke/orca_mini_3B-GGUF:orca-mini-3b.q4_0.gguf"
    ["command-r"]="bartowski/c4ai-command-r-08-2024-GGUF:c4ai-command-r-08-2024-Q4_K_M.gguf"
    ["wizardlm2"]="bartowski/WizardLM-2-8x22B-GGUF:WizardLM-2-8x22B-Q4_K_M.gguf"
)

cmd_pull() {
    local inp="${1:-}"
    if [[ -z "$inp" ]]; then
        print_small_banner; echo
        echo -e "  ${BRAND}${B}Available model shortcuts:${R}\n"
        for a in $(echo "${!MODEL_ALIASES[@]}" | tr ' ' '\n' | sort); do
            printf "  ${NAV}${B}%-20s${R}  ${ACCENT}%s${R}\n" "$a" "${MODEL_ALIASES[$a]##*:}"
        done
        echo
        dim "Usage: markus pull <alias | hf:repo:file | https://...>"
        return
    fi

    local url="" fname=""
    if [[ "$inp" == http://* || "$inp" == https://* ]]; then
        url="$inp"; fname=$(basename "${url%%\?*}")
    elif [[ "$inp" == hf:* ]]; then
        local p="${inp#hf:}"; local repo="${p%%:*}"; local file="${p##*:}"
        url="https://huggingface.co/${repo}/resolve/main/${file}"; fname="$file"
    elif [[ -n "${MODEL_ALIASES[$inp]:-}" ]]; then
        local spec="${MODEL_ALIASES[$inp]}"
        local repo="${spec%%:*}"; local file="${spec##*:}"
        url="https://huggingface.co/${repo}/resolve/main/${file}"; fname="$file"
    else
        error "Unknown model: '$inp'  →  run 'markus pull' to see list"
        exit 1
    fi

    local dest="${MARKUS_MODELS_DIR}/${fname}"
    if [[ -f "$dest" ]]; then
        warn "Already exists: $dest"
        echo -ne "  ${NAV}Overwrite? [y/N]:${R} "; read -r ow
        [[ "$ow" != "y" && "$ow" != "Y" ]] && return
    fi

    echo
    info "Downloading: ${NAV}${fname}${R}"
    label "From: $url"
    label "To:   $dest"
    echo
    mkdir -p "$MARKUS_MODELS_DIR"
    local tmp="${dest}.download"

    command -v wget >/dev/null 2>&1 \
        && wget --continue --show-progress -O "$tmp" "$url" \
        || curl -L --progress-bar -C - -o "$tmp" "$url"

    if [[ $? -eq 0 ]]; then
        mv "$tmp" "$dest"
        success "Saved: ${ACCENT}$dest${R}"
        rm -f "$MARKUS_SCAN_CACHE"
        echo
        dim "Run: markus run $(basename "$dest")"
    else
        rm -f "$tmp"
        error "Download failed."
        exit 1
    fi
}

# ─── Bench ────────────────────────────────────────────────────────────────────
cmd_bench() {
    local inp="${1:-}"; [[ -z "$inp" ]] && { error "No model specified"; exit 1; }
    local mp; mp=$(resolve_model "$inp")
    detect_backend
    local bin="${MARKUS_CLI_BIN:-$MARKUS_SERVER_BIN}"
    [[ -z "$bin" ]] && { error "No llama.cpp binary"; exit 1; }

    local t="${OPT_THREADS:-${THREADS:-$DEFAULT_THREADS}}"
    local c="${OPT_CTX:-${CTX_SIZE:-$DEFAULT_CTX}}"

    print_small_banner; echo
    info "Benchmarking: ${NAV}$(basename "$mp")${R}"
    step "${ACCENT}Threads: ${NAV}${t}${R}  ${ACCENT}Context: ${NAV}${c}${R}  ${ACCENT}Tokens: ${NAV}128${R}"
    echo

    local t0; t0=$(date +%s%N)
    _llama_run_subcmd "cli" "$bin" \
        --model "$mp" --threads "$t" --ctx-size "$c" \
        --n-predict 128 --prompt "The quick brown fox jumps" \
        --log-disable 2>/dev/null
    local t1; t1=$(date +%s%N)

    local ms=$(( (t1 - t0) / 1000000 ))
    local tps; tps=$(echo "scale=2; 128 * 1000 / $ms" | bc 2>/dev/null || echo "?")
    echo
    success "Complete: ${NAV}${ms}ms${R}  —  ${ACCENT}~${tps} tokens/sec${R}"
}

# ─── Quantize ─────────────────────────────────────────────────────────────────
cmd_quantize() {
    local inp="${1:-}" qt="${2:-Q4_K_M}" out="${3:-}"
    [[ -z "$inp" ]] && {
        error "Usage: markus quantize <model> [type] [output]"
        echo -e "  ${D}Types: Q2_K  Q3_K_M  Q4_0  Q4_K_M  Q5_K_M  Q6_K  Q8_0  F16${R}"
        exit 1
    }
    local mp; mp=$(resolve_model "$inp")
    local qb="/usr/local/lib/ollama/llama-quantize"
    [[ ! -x "$qb" ]] && qb=$(which llama-quantize 2>/dev/null || echo "")
    [[ -z "$qb" && "$(basename "${MARKUS_CLI_BIN:-}")" == "llama-cpp-bin" ]] && qb="$MARKUS_CLI_BIN"
    [[ -z "$qb" ]] && { error "llama-quantize not found"; exit 1; }
    [[ -z "$out" ]] && out="${mp%.gguf}-${qt}.gguf"

    print_small_banner; echo
    info "Quantizing: ${NAV}$(basename "$mp")${R}"
    step "${ACCENT}Type:   ${NAV}${qt}${R}"
    step "${ACCENT}Output: ${NAV}${out}${R}"
    echo

    _llama_run_subcmd "quantize" "$qb" "$mp" "$out" "$qt"
    if [[ $? -eq 0 ]]; then
        success "Quantized: ${ACCENT}$out${R}"
        rm -f "$MARKUS_SCAN_CACHE"
    else
        error "Quantization failed"
        exit 1
    fi
}

# ─── Config ───────────────────────────────────────────────────────────────────
cmd_config() {
    local act="${1:-show}"
    case "$act" in
        show|get)
            echo
            echo -e "${BRAND}${B}  ╔═ Configuration ═════════════════════════════════════════╗${R}"
            echo -e "  ${D}  File: $MARKUS_CONFIG_FILE${R}"
            echo -e "${D}  ╟─────────────────────────────────────────────────────────╢${R}"
            while IFS='=' read -r k v; do
                [[ "$k" =~ ^[[:space:]]*# || -z "$k" ]] && continue
                printf "  ${D}║${R}  ${ACCENT}%-22s${R}  ${NAV}%s${R}\n" "${k// /}" "$v"
            done < "$MARKUS_CONFIG_FILE"
            echo -e "  ${BRAND}${B}  ╚═════════════════════════════════════════════════════════╝${R}"
            echo ;;
        set)
            local k="${2:-}" v="${3:-}"
            [[ -z "$k" || -z "$v" ]] && {
                error "Usage: markus config set <KEY> <VALUE>"
                echo -e "  ${D}Keys: THREADS CTX_SIZE BATCH_SIZE GPU_LAYERS TEMPERATURE${R}"
                echo -e "  ${D}      TOP_P TOP_K REPEAT_PENALTY MAX_TOKENS SERVER_HOST SERVER_PORT${R}"
                exit 1
            }
            if grep -q "^${k}=" "$MARKUS_CONFIG_FILE"; then
                sed -i "s|^${k}=.*|${k}=${v}|" "$MARKUS_CONFIG_FILE"
            else
                echo "${k}=${v}" >> "$MARKUS_CONFIG_FILE"
            fi
            success "Set ${ACCENT}${k}${R} = ${NAV}${v}${R}" ;;
        edit) "${EDITOR:-nano}" "$MARKUS_CONFIG_FILE" ;;
        reset)
            rm -f "$MARKUS_CONFIG_FILE"
            init_dirs
            success "Configuration reset to defaults." ;;
        *)
            error "Unknown config action: $act  (show | set KEY VAL | edit | reset)" ;;
    esac
}

# ─── Status ───────────────────────────────────────────────────────────────────
cmd_status() {
    print_small_banner; echo
    echo -e "${BRAND}${B}  System${R}";    echo -e "  ${D}────────────────────────────────────────────${R}"
    local cpu; cpu=$(get_cpu_model)
    printf "  ${ACCENT}%-18s${R} %s ${D}(%s cores)${R}\n" "CPU:" "$cpu" "$(get_cpu_cores)"
    printf "  ${ACCENT}%-18s${R} ${NAV}%s${R} total  /  ${FG_GRN}%s${R} available\n" "Memory:" \
        "$(get_ram_usage_string)" "$(get_ram_avail_string)"
    if command -v nvidia-smi >/dev/null 2>&1; then
        printf "  ${ACCENT}%-18s${R} %s ${D}(%s)${R}\n" "GPU:" \
            "$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)" \
            "$(nvidia-smi --query-gpu=memory.total --format=csv,noheader 2>/dev/null | head -1)"
    else
        printf "  ${ACCENT}%-18s${R} ${D}None detected — CPU mode${R}\n" "GPU:"
    fi

    echo -e "\n${BRAND}${B}  Backend${R}";  echo -e "  ${D}────────────────────────────────────────────${R}"
    detect_backend_quiet 2>/dev/null || true
    printf "  ${ACCENT}%-18s${R} %b\n" "llama-server:" "${MARKUS_SERVER_BIN:-${FG_RED}Not found${R}}"
    printf "  ${ACCENT}%-18s${R} %b\n" "llama-cli:"    "${MARKUS_CLI_BIN:-${FG_RED}Not found${R}}"
    if [[ -n "$MARKUS_SERVER_BIN" ]]; then
        local sv_ver
        sv_ver=$(LD_LIBRARY_PATH="/usr/local/lib/ollama:${LD_LIBRARY_PATH:-}" \
            "$MARKUS_SERVER_BIN" --version 2>&1 | head -1)
        printf "  ${ACCENT}%-18s${R} ${D}%s${R}\n" "Version:" "$sv_ver"
    fi

    echo -e "\n${BRAND}${B}  Paths${R}";    echo -e "  ${D}────────────────────────────────────────────${R}"
    printf "  ${ACCENT}%-18s${R} ${FG_CYN}%s${R}\n" "Config:"  "$MARKUS_CONFIG_FILE"
    printf "  ${ACCENT}%-18s${R} ${FG_CYN}%s${R}\n" "Models:"  "$MARKUS_MODELS_DIR"
    printf "  ${ACCENT}%-18s${R} ${FG_CYN}%s${R}\n" "Logs:"    "$MARKUS_LOG_DIR"
    printf "  ${ACCENT}%-18s${R} ${FG_CYN}%s${R}\n" "Cache:"   "$MARKUS_SCAN_CACHE"

    echo -e "\n${BRAND}${B}  Models${R}";   echo -e "  ${D}────────────────────────────────────────────${R}"
    scan_filesystem 0
    printf "  ${ACCENT}%-18s${R} ${NAV}%s${R} model(s) found\n" "Indexed:" "${#FOUND_MODELS[@]}"

    echo -e "\n${BRAND}${B}  Services${R}"; echo -e "  ${D}────────────────────────────────────────────${R}"
    local svc
    svc=$(ss -tlnp 2>/dev/null | grep -E ":(8080|8081|11434) ")
    if [[ -n "$svc" ]]; then
        echo "$svc" | while read -r l; do dim "$l"; done
    else
        dim "(no markus/llama services detected)"
    fi
    echo
}

# ─── Free Memory ──────────────────────────────────────────────────────────────
cmd_freemem() {
    print_small_banner; echo
    info "Memory Management"
    echo

    local before; before="Total: $(get_ram_usage_string) / Avail: $(get_ram_avail_string)"
    printf "  ${ACCENT}%-22s${R} ${NAV}%s${R}\n" "RAM before:" "$before"
    echo

    # Kill running llama processes across Linux, macOS, and Windows
    local killed=0
    while IFS= read -r pid; do
        [[ -z "$pid" ]] && continue
        local pname; pname=$(ps -p "$pid" -o comm= 2>/dev/null)
        if kill "$pid" 2>/dev/null; then
            success "Killed PID ${NAV}$pid${R} (${ACCENT}$pname${R})"
            killed=$(( killed + 1 ))
        fi
    done < <(pgrep -f "llama-server" 2>/dev/null; pgrep -f "llama-cli" 2>/dev/null)

    if [[ "$(detect_os)" == "windows" ]]; then
        if taskkill //IM "llama-server.exe" //F >/dev/null 2>&1; then
            success "Terminated llama-server.exe process(es) on Windows"
            killed=$(( killed + 1 ))
        fi
        if taskkill //IM "llama-cli.exe" //F >/dev/null 2>&1; then
            success "Terminated llama-cli.exe process(es) on Windows"
            killed=$(( killed + 1 ))
        fi
    fi

    [[ $killed -eq 0 ]] && dim "No running llama-server/llama-cli processes found"
    echo

    step "${ACCENT}Syncing filesystem buffers…${R}"
    sync 2>/dev/null || true
    success "sync complete"

    echo
    step "${ACCENT}Clearing cache and freeing resources…${R}"
    if [[ "$(detect_os)" == "linux" ]]; then
        if echo 3 | sudo -n tee /proc/sys/vm/drop_caches >/dev/null 2>&1; then
            success "Dropped caches (level 3 — page + dentry + inode)"
        elif echo 1 | sudo -n tee /proc/sys/vm/drop_caches >/dev/null 2>&1; then
            success "Dropped page cache (level 1)"
        else
            dim "Page cache drop skipped (requires root/sudo)"
        fi
        if echo 1 | sudo -n tee /proc/sys/vm/compact_memory >/dev/null 2>&1; then
            success "Memory compaction triggered"
        fi
    elif [[ "$(detect_os)" == "darwin" ]]; then
        if command -v purge >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
            sudo -n purge >/dev/null 2>&1 && success "macOS disk cache purged" || dim "macOS purge skipped"
        else
            dim "macOS cache purge skipped (requires sudo)"
        fi
    else
        success "System memory cache refreshed"
    fi

    echo
    sleep 0.3
    local after; after="Total: $(get_ram_usage_string) / Avail: $(get_ram_avail_string)"
    printf "  ${FG_GRN}%-22s${R} ${NAV}%s${R}\n" "RAM after:" "$after"
    echo
    rm -f "$MARKUS_SCAN_CACHE"
    success "Done — model scan cache also cleared."
}

# ─── Checksum / Environment Validation ────────────────────────────────────────
cmd_checksum() {
    print_small_banner
    echo -e "  ${BRAND}${B}Environment & Dependency Checksum Validator${R}\n"
    
    local os_type
    os_type=$(detect_os)
    step "Operating System: ${ACCENT}${os_type}${R}"
    
    step "Checking Backend Binaries..."
    detect_backend_quiet 2>/dev/null || true
    
    _compute_sha256() {
        local file="$1"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum "$file" | awk '{print $1}'
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 "$file" | awk '{print $1}'
        elif [[ "$os_type" == "windows" ]] && command -v powershell.exe >/dev/null 2>&1; then
            powershell.exe -NoProfile -Command "(Get-FileHash -Path '$file' -Algorithm SHA256).Hash.ToLower()" 2>/dev/null | tr -d '\r'
        else
            echo "UNAVAILABLE"
        fi
    }
    
    if [[ -n "$MARKUS_SERVER_BIN" || -n "$MARKUS_CLI_BIN" ]]; then
        [[ -n "$MARKUS_SERVER_BIN" ]] && info "llama-server: ${HL}$(_compute_sha256 "$MARKUS_SERVER_BIN")${R}  ${D}($MARKUS_SERVER_BIN)${R}"
        [[ -n "$MARKUS_CLI_BIN" ]] && info "llama-cli:    ${HL}$(_compute_sha256 "$MARKUS_CLI_BIN")${R}  ${D}($MARKUS_CLI_BIN)${R}"
    else
        warn "No llama backend found! You may need to download one."
    fi
    
    echo
    step "System Environment Checks..."
    local bash_ver="${BASH_VERSION:-Unknown}"
    info "Bash Version:   ${bash_ver}"
    
    local curl_path
    curl_path=$(command -v curl || echo "Missing")
    if [[ "$curl_path" != "Missing" ]]; then
        info "curl:           ${FG_GRN}OK${R} ${D}($curl_path)${R}"
    else
        error "curl:           Missing"
    fi
    
    local awk_path
    awk_path=$(command -v awk || echo "Missing")
    if [[ "$awk_path" != "Missing" ]]; then
        info "awk:            ${FG_GRN}OK${R} ${D}($awk_path)${R}"
    else
        error "awk:            Missing"
    fi

    echo -e "\n  ${FG_GRN}✔${R} Checksum and environment verification complete."
}


# ─── Version ──────────────────────────────────────────────────────────────────
cmd_version() {
    local install_location
    install_location=$(command -v markus 2>/dev/null || echo "$0")
    echo -e "${BRAND}${B}  markus${R}  v${MARKUS_VERSION}  —  installed at ${ACCENT}${install_location}${R} (${NAV}$(detect_os)${R})"
    detect_backend_quiet 2>/dev/null || true
    if [[ -n "$MARKUS_SERVER_BIN" ]]; then
        local sv
        sv=$(LD_LIBRARY_PATH="/usr/local/lib/ollama:${LD_LIBRARY_PATH:-}" \
            "$MARKUS_SERVER_BIN" --version 2>&1 | head -2)
        printf "  ${ACCENT}%-16s${R} ${D}%s${R}\n" "backend:" "$MARKUS_SERVER_BIN"
        echo "$sv" | while read -r l; do dim "$l"; done
    fi
}

# ─── Help ─────────────────────────────────────────────────────────────────────
cmd_help() {
    print_small_banner
    echo
    echo -e "${BRAND}${B}  USAGE${R}"
    echo -e "    ${ACCENT}markus${R}                              Launch interactive arrow-key menu"
    echo -e "    ${ACCENT}markus${R} <command> [model] [opts]    Run command directly"
    echo
    echo -e "${BRAND}${B}  COMMANDS${R}"
    printf "    ${ACCENT}%-12s${R}  ${FG_WHT}%-14s${R}  %s\n" \
        "run"      "[model]"     "Interactive chat with history" \
        "serve"    "[model]"     "Start OpenAI-compatible HTTP server" \
        "pull"     "<model>"     "Download model (alias | hf:repo:file | https://)" \
        "list"     ""            "List all detected model files" \
        "scan"     "[--force]"   "Scan filesystem for model files" \
        "info"     "<model>"     "Show model metadata + GGUF header" \
        "remove"   "<model>"     "Delete a model file" \
        "quantize" "<model>"     "Re-quantize GGUF (Q4_K_M, Q8_0, F16…)" \
        "bench"    "[model]"     "Token-generation benchmark" \
        "freemem"  ""            "Kill servers, drop caches, free RAM" \
        "config"   "show|set|…"  "Manage ~/.config/markus/config.sh" \
        "status"   ""            "System, backend & service info" \
        "checksum" ""            "Validate env & llama binary checksums" \
        "version"  ""            "Version information"
    echo
}

# ─── Interactive TUI Minimal Category Menu ───────────────────────────────────
_pause() {
    echo
    echo -ne "  ${D}Press Enter to return to menu…${R}"
    read -rs
}

_pick_model_from_menu() {
    local action="$1"
    scan_filesystem
    if [[ ${#FOUND_MODELS[@]} -eq 0 ]]; then
        tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
        clear; print_small_banner; echo
        warn "No models found on this system."
        dim "Download one: markus pull <name>"
        echo; _pause; return 1
    fi

    local -a menu_args=()
    for m in "${FOUND_MODELS[@]}"; do
        [[ -f "$m" ]] || continue
        local sz; sz=$(du -sh "$m" 2>/dev/null | cut -f1)
        local ext="${m##*.}"
        menu_args+=("$(basename "$m")" "${sz}  ·  ${ext^^}")
    done
    menu_args+=("← Back" "Return to previous menu")

    arrow_menu "Select model to ${action}" "Found ${#FOUND_MODELS[@]} model(s)" "${menu_args[@]}"
    local ret=$?
    [[ $ret -ne 0 || $MENU_RESULT -eq ${#FOUND_MODELS[@]} ]] && return 1
    PICKED_MODEL="${FOUND_MODELS[$MENU_RESULT]}"
    return 0
}

PICKED_MODEL=""

_sub_start_menu() {
    while true; do
        arrow_menu "Start Mode" "Choose interactive chat or API server" \
            "Run"        "Interactive chat CLI with conversation history" \
            "Serve"      "Start OpenAI-compatible HTTP API server" \
            "← Back"     "Return to main menu"
        [[ $MENU_RESULT -ne 0 && $MENU_RESULT -ne 1 ]] && return
        if [[ $MENU_RESULT -eq 0 ]]; then
            _pick_model_from_menu "run" && {
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; cmd_run "$PICKED_MODEL" ""
                return
            }
        elif [[ $MENU_RESULT -eq 1 ]]; then
            _pick_model_from_menu "serve" && {
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; cmd_serve "$PICKED_MODEL"
                return
            }
        fi
    done
}

_sub_models_menu() {
    while true; do
        arrow_menu "Models Manager" "Manage local and remote models" \
            "Pull"       "Download a model from HuggingFace / URL / shortcut" \
            "List"       "List all model files detected on system" \
            "Scan"       "Scan filesystem for model files (with cache control)" \
            "Info"       "Show model metadata & GGUF header inspection" \
            "Remove"     "Delete a model file from disk" \
            "← Back"     "Return to main menu"
        case $MENU_RESULT in
            0) _sub_pull_menu ;;
            1)
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; cmd_list; _pause ;;
            2)
                arrow_menu "Scan Mode" "Choose scan behaviour" \
                    "Normal scan"   "Use cached results if < 6h old" \
                    "Force rescan"  "Ignore cache — scan everything now" \
                    "← Back"       "Return to previous menu"
                if [[ $MENU_RESULT -eq 0 || $MENU_RESULT -eq 1 ]]; then
                    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                    clear
                    [[ $MENU_RESULT -eq 1 ]] && cmd_scan "--force" || cmd_scan
                    _pause
                fi ;;
            3)
                _pick_model_from_menu "info" && {
                    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                    clear; print_small_banner; echo
                    cmd_info "$PICKED_MODEL"; _pause
                } ;;
            4)
                _pick_model_from_menu "remove" && {
                    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                    clear; print_small_banner; echo
                    cmd_remove "$PICKED_MODEL"; _pause
                } ;;
            *) return ;;
        esac
    done
}

_sub_system_menu() {
    while true; do
        arrow_menu "System & Operations" "Manage hardware, memory & configuration" \
            "Free RAM"       "Kill running llama servers & drop Linux memory caches" \
            "Status"         "Show CPU/GPU hardware, backends & active services" \
            "Bench"          "Run a token-generation speed benchmark" \
            "Config"         "View or edit ~/.config/markus/config.sh" \
            "Setup Backend"  "Install or build llama.cpp if missing or broken" \
            "Version"        "Show Markus & llama.cpp backend version info" \
            "← Back"         "Return to main menu"
        case $MENU_RESULT in
            0)
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; cmd_freemem; _pause ;;
            1)
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; cmd_status; _pause ;;
            2)
                _pick_model_from_menu "benchmark" && {
                    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                    clear; cmd_bench "$PICKED_MODEL"; _pause
                } ;;
            3)
                _sub_config_menu ;;
            4)
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; print_small_banner; echo
                MARKUS_SERVER_BIN=""; MARKUS_CLI_BIN="" # Force re-detect by clearing variables
                detect_backend; _pause ;;
            5)
                tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
                clear; echo; cmd_version; echo; _pause ;;
            *) return ;;
        esac
    done
}

_sub_pull_menu() {
    local -a pull_items=(
        "llama3.2"     "Llama 3.2 3B Instruct  (fastest)"
        "llama3.1"     "Llama 3.1 8B Instruct"
        "llama3.3"     "Llama 3.3 70B Instruct  (large)"
        "mistral"      "Mistral 7B Instruct v0.2"
        "mistral-nemo" "Mistral Nemo 12B"
        "phi4"         "Microsoft Phi-4  (smart & compact)"
        "phi3.5"       "Microsoft Phi-3.5 mini"
        "gemma3"       "Google Gemma 3 4B IT"
        "gemma2"       "Google Gemma 2 9B IT"
        "qwen3"        "Qwen3 8B"
        "qwen2.5"      "Qwen2.5 7B Instruct"
        "deepseek-r1"  "DeepSeek R1 Distill 8B  (reasoning)"
        "codellama"    "CodeLlama 13B  (code specialist)"
        "starcoder2"   "StarCoder2 15B  (code generation)"
        "tinyllama"    "TinyLlama 1.1B  (ultra-fast)"
        "smollm2"      "SmolLM2 1.7B  (lightweight)"
        "hf: custom"   "Enter HuggingFace repo:file manually"
        "URL"          "Enter a direct download URL"
        "← Back"      "Return to Models menu"
    )

    local -a alias_map=(
        "llama3.2" "llama3.1" "llama3.3" "mistral" "mistral-nemo"
        "phi4" "phi3.5" "gemma3" "gemma2" "qwen3" "qwen2.5"
        "deepseek-r1" "codellama" "starcoder2" "tinyllama" "smollm2"
    )

    arrow_menu "Pull a Model" "Select a model to download" "${pull_items[@]}"
    [[ $MENU_RESULT -eq -1 ]] && return
    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
    clear; print_small_banner; echo

    local idx=$MENU_RESULT
    if [[ $idx -lt ${#alias_map[@]} ]]; then
        cmd_pull "${alias_map[$idx]}"
    elif [[ $idx -eq 16 ]]; then
        echo -ne "  ${ACCENT}${B}hf:<repo>:<filename>:${R} "; read -r hf_uri
        [[ -n "$hf_uri" ]] && cmd_pull "$hf_uri"
    elif [[ $idx -eq 17 ]]; then
        echo -ne "  ${ACCENT}${B}https:// URL:${R} "; read -r dl_url
        [[ -n "$dl_url" ]] && cmd_pull "$dl_url"
    fi
    _pause
}

_sub_quantize() {
    _pick_model_from_menu "quantize" || return
    local mp="$PICKED_MODEL"

    arrow_menu "Quantize: $(basename "$mp")" "Choose quantization type" \
        "Q4_K_M"  "4-bit  ·  balanced quality  (recommended)" \
        "Q5_K_M"  "5-bit  ·  better quality  ·  larger file" \
        "Q8_0"    "8-bit  ·  near-lossless  ·  largest" \
        "Q4_0"    "4-bit  ·  fastest  ·  smallest" \
        "Q3_K_M"  "3-bit  ·  very compact  ·  lower quality" \
        "Q2_K"    "2-bit  ·  ultra-small  ·  reduced accuracy" \
        "Q6_K"    "6-bit  ·  high fidelity" \
        "F16"     "Float-16  ·  full precision" \
        "← Back"  "Return to main menu"
    [[ $MENU_RESULT -eq -1 ]] && return

    local qt_map=("Q4_K_M" "Q5_K_M" "Q8_0" "Q4_0" "Q3_K_M" "Q2_K" "Q6_K" "F16")
    [[ $MENU_RESULT -ge ${#qt_map[@]} ]] && return

    tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
    clear; print_small_banner; echo
    cmd_quantize "$mp" "${qt_map[$MENU_RESULT]}"
    _pause
}

_sub_config_menu() {
    while true; do
        arrow_menu "Configuration" "${MARKUS_CONFIG_FILE}" \
            "View settings"      "Show current configuration" \
            "THREADS"            "CPU thread count" \
            "CTX_SIZE"           "Context window size" \
            "TEMPERATURE"        "Default generation temperature" \
            "GPU_LAYERS"         "GPU offload layers (0 = CPU)" \
            "SERVER_PORT"        "Default server port" \
            "MAX_TOKENS"         "Max tokens to generate (-1 = ∞)" \
            "Open in editor"     "Edit config file with \$EDITOR" \
            "Reset defaults"     "Restore all settings to defaults" \
            "← Back"             "Return to System menu"
        [[ $MENU_RESULT -eq -1 ]] && return
        tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
        clear; print_small_banner; echo

        local keys=("" "THREADS" "CTX_SIZE" "TEMPERATURE" "GPU_LAYERS" "SERVER_PORT" "MAX_TOKENS")
        case $MENU_RESULT in
            0) cmd_config show ;;
            1|2|3|4|5|6)
                local k="${keys[$MENU_RESULT]}"
                echo -ne "  ${ACCENT}${B}${k} [current: $(grep "^${k}=" "$MARKUS_CONFIG_FILE" | cut -d= -f2-)]:${R} "
                read -r v
                [[ -n "$v" ]] && cmd_config set "$k" "$v"
                ;;
            7) cmd_config edit ;;
            8) cmd_config reset ;;
            9) return ;;
        esac
        _pause
    done
}

show_main_menu() {
    while true; do
        arrow_menu \
            "MARKUS — AI Model Manager" \
            "↑↓/jk navigate, Enter select, ESC/q back" \
            "Start"       "Run interactive chatbot CLI or OpenAI HTTP API server" \
            "Models"      "Pull, list, scan, inspect metadata, or remove models" \
            "System"      "Free RAM, hardware status, benchmark, configuration" \
            "Quantize"    "Re-quantize GGUF model files (Q4_K_M, Q8_0, etc.)" \
            "Exit"        "Exit Markus"

        local ret=$?
        [[ $ret -ne 0 || $MENU_RESULT -eq 4 ]] && {
            tput rmcup 2>/dev/null; tput cnorm 2>/dev/null
            echo -e "\n${D}  Goodbye!${R}"; exit 0
        }

        case $MENU_RESULT in
            0) _sub_start_menu ;;
            1) _sub_models_menu ;;
            2) _sub_system_menu ;;
            3) _sub_quantize ;;
        esac
    done
}

# ─── Option Parser ────────────────────────────────────────────────────────────
OPT_THREADS="" OPT_CTX="" OPT_BATCH="" OPT_GPU_LAYERS="" OPT_TEMP=""
OPT_TOP_P="" OPT_TOP_K="" OPT_REPEAT_PENALTY="" OPT_MAX_TOKENS=""
OPT_MLOCK="0" OPT_SYSTEM_PROMPT="" OPT_HOST="" OPT_PORT=""
OPT_ALIAS="" OPT_VERBOSE="0"

parse_opts() {
    local -a pos=()
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -t|--threads)           OPT_THREADS="$2";       shift 2 ;;
            -c|--ctx|--ctx-size)    OPT_CTX="$2";           shift 2 ;;
            -b|--batch)             OPT_BATCH="$2";          shift 2 ;;
            -ngl|--gpu-layers|--n-gpu-layers)
                                    OPT_GPU_LAYERS="$2";    shift 2 ;;
            --temp|--temperature)   OPT_TEMP="$2";           shift 2 ;;
            --top-p)                OPT_TOP_P="$2";          shift 2 ;;
            --top-k)                OPT_TOP_K="$2";          shift 2 ;;
            --repeat-penalty)       OPT_REPEAT_PENALTY="$2"; shift 2 ;;
            -n|--max-tokens)        OPT_MAX_TOKENS="$2";     shift 2 ;;
            --mlock)                OPT_MLOCK="1";           shift ;;
            --system-prompt|-s)     OPT_SYSTEM_PROMPT="$2";  shift 2 ;;
            --host)                 OPT_HOST="$2";           shift 2 ;;
            --port|-p)              OPT_PORT="$2";           shift 2 ;;
            --alias)                OPT_ALIAS="$2";          shift 2 ;;
            --verbose|-v)           OPT_VERBOSE="1";         shift ;;
            --)                     shift; pos+=("$@");      break ;;
            -*)                     warn "Unknown option: $1"; shift ;;
            *)                      pos+=("$1");             shift ;;
        esac
    done
    printf '%s\n' "${pos[@]:-}"
}

# ─── Direct CLI model picker ─────────────────────────────────────────────────
_pick_model_cli() {
    scan_filesystem
    [[ ${#FOUND_MODELS[@]} -eq 0 ]] && {
        error "No models found. Run: markus pull <model>"; exit 1; }
    echo -e "\n${BRAND}${B}  Select a model:${R}\n" >&2
    local i=1
    for m in "${FOUND_MODELS[@]}"; do
        [[ -f "$m" ]] || continue
        local sz; sz=$(du -sh "$m" 2>/dev/null | cut -f1)
        printf "    ${NAV}${B}[%2d]${R}  ${ACCENT}%-50s${R}  ${D}%s${R}\n" \
            "$i" "$(basename "$m")" "$sz" >&2
        (( i++ ))
    done
    echo >&2
    echo -ne "  ${ACCENT}Enter number (1-$((i-1))):${R} " >&2
    read -r ch
    resolve_model "$ch"
}

# ─── Main ─────────────────────────────────────────────────────────────────────
main() {
    init_dirs

    if [[ $# -eq 0 ]]; then
        show_main_menu
        exit 0
    fi

    local cmd="$1"; shift

    local -a pos=()
    while IFS= read -r l; do [[ -n "$l" ]] && pos+=("$l"); done < <(parse_opts "$@")

    case "$cmd" in
        run|chat)
            local model="${pos[0]:-}"
            [[ -z "$model" ]] && model=$(_pick_model_cli)
            cmd_run "$model" "${pos[1]:-}"
            ;;
        serve|server)
            local model="${pos[0]:-}"
            [[ -z "$model" ]] && model=$(_pick_model_cli)
            cmd_serve "$model"
            ;;
        pull|download|get)
            if [[ ${#pos[@]} -gt 1 ]]; then
                for m in "${pos[@]}"; do cmd_pull "$m"; echo; done
            else
                cmd_pull "${pos[0]:-}"
            fi ;;
        list|ls|models)    cmd_list ;;
        scan)              cmd_scan "${pos[0]:-}" ;;
        info|show)         cmd_info "${pos[0]:-}" ;;
        remove|rm|delete)  cmd_remove "${pos[0]:-}" ;;
        quantize|quant)    cmd_quantize "${pos[0]:-}" "${pos[1]:-}" "${pos[2]:-}" ;;
        bench|benchmark)
            local model="${pos[0]:-}"
            [[ -z "$model" ]] && model=$(_pick_model_cli)
            cmd_bench "$model" ;;
        freemem|free|clearmem|dropcache) cmd_freemem ;;
        config|cfg)
            cmd_config "${pos[0]:-show}" "${pos[1]:-}" "${pos[2]:-}" ;;
        status|ps)         cmd_status ;;
        checksum|verify)   cmd_checksum ;;
        version|-V|--version) cmd_version ;;
        help|--help|-h)    cmd_help ;;
        *)
            error "Unknown command: '$cmd'"
            echo -e "  ${D}Run ${ACCENT}markus help${R}${D} or just ${ACCENT}markus${R}${D} for the interactive menu.${R}"
            exit 1 ;;
    esac
}

main "$@"
