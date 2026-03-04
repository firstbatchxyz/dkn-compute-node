#!/usr/bin/env bash
# Dria Node installer for macOS and Linux
# Usage: curl -sSL https://raw.githubusercontent.com/firstbatchxyz/dkn-compute-node/v2/scripts/install.sh | bash
set -euo pipefail

REPO="firstbatchxyz/dkn-compute-node"
BINARY="dria-node"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

# Detect OS
case "$(uname -s)" in
    Darwin)  OS="macOS" ;;
    Linux)   OS="linux" ;;
    *)       error "Unsupported OS: $(uname -s). Use Windows installer for Windows." ;;
esac

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64)   ARCH="amd64" ;;
    aarch64|arm64)  ARCH="arm64" ;;
    *)              error "Unsupported architecture: $(uname -m)" ;;
esac

# On Linux x86_64, check for AVX2 support and fall back to noavx if missing
if [ "$OS" = "linux" ] && [ "$ARCH" = "amd64" ]; then
    if ! grep -q avx2 /proc/cpuinfo 2>/dev/null; then
        ARCH="amd64-noavx"
        info "CPU does not support AVX2, using baseline binary."
    fi
fi

info "Detected: ${OS} ${ARCH}"

# Fetch latest release tag
info "Fetching latest release..."
LATEST=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4) \
    || error "Failed to fetch latest release. Check your internet connection."

[ -z "$LATEST" ] && error "Could not determine latest release tag."
info "Latest release: ${LATEST}"

# Download binary
ASSET="${BINARY}-${OS}-${ARCH}"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET}"

info "Downloading ${ASSET}..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -sSfL -o "${TMPDIR}/${BINARY}" "$URL" \
    || error "Download failed. Asset may not exist for your platform: ${URL}"

chmod +x "${TMPDIR}/${BINARY}"

# Install
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ "$(id -u)" = "0" ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
info "Installed to ${INSTALL_DIR}/${BINARY}"

# Check if install dir is in PATH
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        info ""
        info "WARNING: ${INSTALL_DIR} is not in your PATH."
        info "Add it by running:"
        info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        info "Or add that line to your ~/.bashrc / ~/.zshrc"
        ;;
esac

# Verify
if command -v "$BINARY" &>/dev/null; then
    info ""
    info "Successfully installed $(${BINARY} --version)"
    info "Run '${BINARY} start --help' to get started."
else
    info ""
    info "Installation complete. Run '${INSTALL_DIR}/${BINARY} --version' to verify."
fi
