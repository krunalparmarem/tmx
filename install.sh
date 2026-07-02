#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# TMX 10x installer
# Downloads a prebuilt binary for your platform (no Rust required). Falls back to
# `cargo install` from source only when a matching prebuilt binary is unavailable.
# ==============================================================================

REPO="krunalparmarem/tmx"
BIN="tmx"

info()  { printf '\033[38;5;117mℹ️  %s\033[0m\n' "$1"; }
ok()    { printf '\033[38;5;114m✅ %s\033[0m\n' "$1"; }
warn()  { printf '\033[38;5;221m⚠️  %s\033[0m\n' "$1"; }
err()   { printf '\033[38;5;203m❌ %s\033[0m\n' "$1" >&2; }

echo "🚀 Installing TMX 10x Agentic Workspace..."

if ! command -v tmux >/dev/null 2>&1; then
    warn "tmux is not installed. Install it first:"
    echo "     macOS:  brew install tmux"
    echo "     Debian: sudo apt install tmux"
fi

# ------------------------------------------------------------------------------
# Detect platform -> release target triple
# ------------------------------------------------------------------------------
detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Darwin)
            case "$arch" in
                arm64|aarch64) echo "aarch64-apple-darwin" ;;
                x86_64)        echo "x86_64-apple-darwin" ;;
                *) return 1 ;;
            esac ;;
        Linux)
            case "$arch" in
                x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
                arm64|aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *) return 1 ;;
            esac ;;
        *) return 1 ;;
    esac
}

# Pick an install dir on PATH that we can write to.
install_dir() {
    for d in "$HOME/.local/bin" "/usr/local/bin"; do
        if [ -d "$d" ] && [ -w "$d" ]; then echo "$d"; return 0; fi
    done
    mkdir -p "$HOME/.local/bin" && echo "$HOME/.local/bin"
}

install_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        err "cargo (Rust) is not installed and no prebuilt binary was available."
        echo "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    info "Building from source with cargo..."
    cargo install --git "https://github.com/${REPO}.git" --force
    ok "Installation complete! Run 'tmx' to launch your command center. 🤖"
    exit 0
}

TARGET="$(detect_target || true)"
if [ -z "${TARGET}" ]; then
    warn "No prebuilt binary for this platform — falling back to source build."
    install_from_source
fi

URL="https://github.com/${REPO}/releases/latest/download/${BIN}-${TARGET}.tar.gz"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

info "Downloading ${BIN} (${TARGET})..."
if ! curl -fsSL "$URL" -o "$TMPDIR/tmx.tar.gz"; then
    warn "Prebuilt download failed — falling back to source build."
    install_from_source
fi

tar -xzf "$TMPDIR/tmx.tar.gz" -C "$TMPDIR"
DEST="$(install_dir)"
mv "$TMPDIR/$BIN" "$DEST/$BIN"
chmod +x "$DEST/$BIN"

ok "Installed to ${DEST}/${BIN}"
case ":$PATH:" in
    *":$DEST:"*) ;;
    *) warn "Add ${DEST} to your PATH:  export PATH=\"${DEST}:\$PATH\"" ;;
esac
echo "Run '${BIN}' — your agent command center awaits! 🤖"
