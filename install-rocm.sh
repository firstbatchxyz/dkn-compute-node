#!/usr/bin/env bash
set -euo pipefail

REPO="firstbatchxyz/dkn-compute-node"
BINARY="dria-node"
ASSET="dria-node-linux-amd64-rocm.tar.gz"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

if [ "$(uname -s)" != "Linux" ]; then
  error "This installer only supports Linux."
fi

case "$(uname -m)" in
  x86_64|amd64) ;;
  *) error "This installer only supports x86_64 / amd64 systems." ;;
esac

command -v curl >/dev/null 2>&1 || error "curl is required."
command -v tar >/dev/null 2>&1 || error "tar is required."

HAS_ROCM_RUNTIME=0
if [ -d "${ROCM_PATH:-}" ] || [ -d "${HIP_PATH:-}" ] || [ -d /opt/rocm ] || ldconfig -p 2>/dev/null | grep -q 'libamdhip64'; then
  HAS_ROCM_RUNTIME=1
else
  warn "ROCm runtime was not detected in a standard location."
  warn "Install ROCm 6.x first if you have not already."
fi

TAG="${DRIA_NODE_TAG:-}"
if [ -z "${TAG}" ]; then
  info "Fetching latest release..."
  TAG=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4) \
    || error "Failed to fetch the latest release tag."
fi

[ -n "${TAG}" ] || error "Could not determine which release to install."
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

if [ -w "/usr/local/bin" ] && [ -w "/usr/local/lib" ]; then
  PREFIX="/usr/local"
elif [ "$(id -u)" = "0" ]; then
  PREFIX="/usr/local"
else
  PREFIX="${HOME}/.local"
fi

INSTALL_DIR="${PREFIX}/lib/dria-node-rocm"
LINK_PATH="${PREFIX}/bin/${BINARY}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT

mkdir -p "${PREFIX}/bin" "${PREFIX}/lib"

info "Downloading ${ASSET} from ${TAG}..."
curl -sSfL -o "${TMPDIR}/${ASSET}" "${URL}" \
  || error "Download failed. The ROCm bundle may not exist yet for ${TAG}."

info "Installing to ${INSTALL_DIR}..."
tar -xzf "${TMPDIR}/${ASSET}" -C "${TMPDIR}"

BUNDLE_SOURCE=$(find "${TMPDIR}" -mindepth 1 -maxdepth 1 -type d -name 'dria-node-linux-amd64-rocm*' | head -1)
[ -n "${BUNDLE_SOURCE}" ] || error "Downloaded archive did not contain the expected ROCm bundle."

rm -rf "${INSTALL_DIR}.tmp"
mv "${BUNDLE_SOURCE}" "${INSTALL_DIR}.tmp"
rm -rf "${INSTALL_DIR}"
mv "${INSTALL_DIR}.tmp" "${INSTALL_DIR}"

ln -sfn "${INSTALL_DIR}/dria-node" "${LINK_PATH}"

case ":${PATH}:" in
  *":${PREFIX}/bin:"*) ;;
  *)
    warn "${PREFIX}/bin is not in your PATH."
    warn "Add it with: export PATH=\"${PREFIX}/bin:\$PATH\""
    ;;
esac

if [ "${HAS_ROCM_RUNTIME}" -eq 1 ] && "${LINK_PATH}" --version >/dev/null 2>&1; then
  info "Successfully installed $(${LINK_PATH} --version)"
else
  info "Installation complete."
  info "Run '${LINK_PATH} --version' after ROCm is installed to verify."
fi
