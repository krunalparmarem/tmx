#!/usr/bin/env bash
set -e

echo "🚀 Installing TMX 10x Agentic Workspace..."

if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust/Cargo is not installed."
    echo "Please install Rust first by running:"
    echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if ! command -v tmux &> /dev/null; then
    echo "❌ Error: tmux is not installed."
    echo "Please install tmux first (e.g., 'brew install tmux' or 'apt install tmux')"
    exit 1
fi

echo "📦 Compiling and installing from source..."
cargo install --git https://github.com/krunalparmarem/tmx.git --force

echo "✅ Installation complete!"
echo "Run 'tmx' in your terminal to begin the setup!"
