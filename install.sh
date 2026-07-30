#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
#  MARKUS  —  Universal One-Line Installer
#  Usage:
#    curl -fsSL https://raw.githubusercontent.com/<YOUR_USERNAME>/markus/main/install.sh | bash
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# Colors
R='\033[0m'
B='\033[1m'
BRAND='\033[91m'       # Bright Red
ACCENT='\033[96m'      # Cyan
FG_GRN='\033[92m'      # Green
FG_YLW='\033[93m'      # Yellow
D='\033[2m'            # Dim

# Default repository location (replace with your GitHub username/repo)
REPO_URL="${MARKUS_REPO_URL:-https://raw.githubusercontent.com/USER/markus/main}"

echo -e "${BRAND}${B}"
cat << "EOF"
  ███╗   ███╗ █████╗ ██████╗ ██╗  ██╗██╗   ██╗███████╗
  ████╗ ████║██╔══██╗██╔══██╗██║ ██╔╝██║   ██║██╔════╝
  ██╔████╔██║███████║██████╔╝█████╔╝ ██║   ██║███████╗
  ██║╚██╔╝██║██╔══██║██╔══██╗██╔═██╗ ██║   ██║╚════██║
  ██║ ╚═╝ ██║██║  ██║██║  ██║██║  ██╗╚██████╔╝███████║
  ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝
EOF
echo -e "${R}"
echo -e "  ${B}MARKUS${R} — Universal AI Model Manager & Chatbot CLI Installer\n"

# 1. Determine installation directory
INSTALL_DIR="/usr/local/bin"
USE_SUDO=""

if [[ $EUID -ne 0 ]]; then
    if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
        USE_SUDO="sudo"
    elif [[ ! -w "$INSTALL_DIR" ]]; then
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "$INSTALL_DIR"
        echo -e "  ${D}ℹ No sudo privileges; installing to user directory: ${ACCENT}${INSTALL_DIR}${R}"
    fi
fi

TARGET_PATH="${INSTALL_DIR}/markus"
echo -e "  ◆ Installing Markus to ${ACCENT}${TARGET_PATH}${R}..."

# 2. Download the latest markus binary/script
TMP_FILE="$(mktemp /tmp/markus_install.XXXXXX)"
trap 'rm -f "$TMP_FILE"' EXIT

if ! curl -fsSL "${REPO_URL}/markus" -o "$TMP_FILE"; then
    echo -e "  ${FG_YLW}⚠ Could not download from ${REPO_URL}/markus.${R}"
    echo -e "  ${D}ℹ If testing locally, copying from local project directory...${R}"
    if [[ -f "./markus" ]]; then
        cp "./markus" "$TMP_FILE"
    else
        echo -e "  \033[91m✖ Error: Could not find 'markus' script to install.\033[0m" >&2
        exit 1
    fi
fi

# 3. Install binary
chmod +x "$TMP_FILE"
${USE_SUDO} mv "$TMP_FILE" "$TARGET_PATH"
${USE_SUDO} chmod 755 "$TARGET_PATH"

# 4. Initialize default directories
CONFIG_DIR="${HOME}/.config/markus"
MODELS_DIR="${HOME}/.local/share/markus/models"
mkdir -p "$CONFIG_DIR" "$MODELS_DIR"

echo -e "  ${FG_GRN}✔${R} Successfully installed ${B}markus${R} to ${ACCENT}${TARGET_PATH}${R}"

# 5. Check PATH for non-standard install dir
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo -e "\n  ${FG_YLW}⚠ Note: ${INSTALL_DIR} is not currently in your PATH.${R}"
    echo -e "  Add it by appending this line to your ~/.bashrc or ~/.zshrc:"
    echo -e "    ${ACCENT}export PATH=\"\$PATH:${INSTALL_DIR}\"${R}"
fi

# 6. Check for llama.cpp backend
echo -e "\n  ${D}─── Checking backend dependency ───────────────────────────────────${R}"
if command -v llama-cli >/dev/null 2>&1 || command -v llama-server >/dev/null 2>&1 || command -v llama-cpp-bin >/dev/null 2>&1 || [[ -x "/usr/local/lib/ollama/llama-server" ]]; then
    echo -e "  ${FG_GRN}✔${R} llama.cpp backend detected on system."
else
    echo -e "  ${FG_YLW}ℹ${R} No llama.cpp backend found yet."
    echo -e "    When you first run ${ACCENT}markus${R}, it can automatically download or build"
    echo -e "    the llama.cpp backend for you."
fi

echo -e "\n${BRAND}${B}  ┌─ ALL DONE! ────────────────────────────────────────────────────────┐${R}"
echo -e "  ${D}│${R}  Launch the interactive menu:   ${ACCENT}markus${R}                            ${D}│${R}"
echo -e "  ${D}│${R}  Download a model:              ${ACCENT}markus pull llama3${R}                ${D}│${R}"
echo -e "  ${D}│${R}  Start interactive chat:        ${ACCENT}markus run <model>${R}                ${D}│${R}"
echo -e "  ${D}└────────────────────────────────────────────────────────────────────┘${R}\n"
