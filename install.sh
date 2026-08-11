#!/usr/bin/env bash
# markus installer — builds and installs from source for Linux & macOS
# Usage via curl: curl -sSL https://raw.githubusercontent.com/Precise-Goals/markus/main/install.sh | bash

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="markus"
REPO_URL="https://github.com/Precise-Goals/markus.git"

# Colors
GRN="\033[92m"; RED="\033[91m"; CYN="\033[96m"; DIM="\033[2m"; RST="\033[0m"; B="\033[1m"
info()    { echo -e "  ${CYN}${B}◆${RST}  $*"; }
success() { echo -e "  ${GRN}${B}✔${RST}  $*"; }
error()   { echo -e "  ${RED}${B}✘${RST}  $*" >&2; }

echo -e "\n  ${RED}${B}▸ MARKUS INSTALLER${RST}  ${DIM}v3.0.0 — Pure Rust (Linux/macOS)${RST}\n"

# 1. Determine execution context (local dir vs piped via curl)
TEMP_CLONE_DIR=""
if [[ -d "engine/crates/markus-core" ]]; then
    # Running locally in the cloned repo
    ENGINE_DIR="${PWD}/engine"
else
    # Piped from curl or run outside the repo
    info "Cloning repository to temporary directory..."
    if ! command -v git >/dev/null; then
        error "git is required but not installed."
        exit 1
    fi
    TEMP_CLONE_DIR=$(mktemp -d)
    git clone --depth 1 "$REPO_URL" "$TEMP_CLONE_DIR"
    ENGINE_DIR="${TEMP_CLONE_DIR}/engine"
fi

# 2. Check Rust
if ! command -v cargo >/dev/null 2>&1; then
    if [[ -f "$HOME/.cargo/env" ]]; then
        source "$HOME/.cargo/env"
    else
        info "Installing Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source "$HOME/.cargo/env"
    fi
fi
success "Rust $(rustc --version | awk '{print $2}') ready"

# 3. Build
info "Building markus (release)... this may take a few minutes."
cd "$ENGINE_DIR"
cargo build --release 2>&1 | grep -E "Compiling|Finished|error" | while read -r line; do
    echo -e "     ${DIM}${line}${RST}"
done

if [[ ! -f "$ENGINE_DIR/target/release/markus-engine" ]]; then
    error "Build failed — check output above"
    [[ -n "$TEMP_CLONE_DIR" ]] && rm -rf "$TEMP_CLONE_DIR"
    exit 1
fi
success "Build complete"

# 4. Install
mkdir -p "$INSTALL_DIR"
cp "$ENGINE_DIR/target/release/markus-engine" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
success "Installed to ${CYN}$INSTALL_DIR/$BINARY_NAME${RST}"

# 5. Cleanup
if [[ -n "$TEMP_CLONE_DIR" ]]; then
    rm -rf "$TEMP_CLONE_DIR"
fi

# 6. PATH check
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo -e "\n  ${DIM}Add this to your shell profile (~/.bashrc or ~/.zshrc):${RST}"
    echo -e "  ${CYN}export PATH=\"\$HOME/.local/bin:\$PATH\"${RST}\n"
fi

echo -e "\n  ${GRN}${B}✔  markus is ready!${RST}\n"
echo -e "  ${DIM}Type${RST} ${CYN}markus${RST} ${DIM}from any directory to start.${RST}\n"
