#!/bin/sh
set -e

REPO="firstbatchxyz/dkn-compute-node"
BINARY="dria-node"
INSTALL_DIR="/usr/local/bin"

# Detect OS
OS=$(uname -s)
case "$OS" in
  Linux*)  OS_NAME="linux" ;;
  Darwin*) OS_NAME="macOS" ;;
  *)       echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)  ARCH_NAME="amd64" ;;
  aarch64|arm64)  ARCH_NAME="arm64" ;;
  *)              echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest release tag (includes pre-releases)
TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases" | grep '"tag_name"' | head -1 | cut -d'"' -f4)
if [ -z "$TAG" ]; then
  echo "Failed to fetch latest release"
  exit 1
fi

ASSET="${BINARY}-${OS_NAME}-${ARCH_NAME}"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

echo "Installing ${BINARY} ${TAG} (${OS_NAME}/${ARCH_NAME})..."

TMPFILE=$(mktemp)
curl -fsSL "$URL" -o "$TMPFILE"
chmod +x "$TMPFILE"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPFILE" "${INSTALL_DIR}/${BINARY}"
else
  sudo mv "$TMPFILE" "${INSTALL_DIR}/${BINARY}"
fi

echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"
"${INSTALL_DIR}/${BINARY}" --version
