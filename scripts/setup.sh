#!/usr/bin/env bash
# Codex AI — Linux/Mac Setup Script
set -e

echo "=== Codex AI Setup ==="

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
echo "Rust: $(rustc --version)"

# Check Python
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 not found. Please install Python 3.11+"
    exit 1
fi
echo "Python: $(python3 --version)"

# Setup Python venv
echo "Creating Python virtual environment..."
python3 -m venv python/.venv
source python/.venv/bin/activate
pip install -r python/requirements.txt
echo "Python dependencies installed."

# Build Rust
echo "Building Rust core..."
cd rust
cargo build --release
cd ..

# Create directories
mkdir -p data workspace/projects logs

# Copy env if needed
if [ ! -f .env ]; then
    cp .env.example .env
    echo "Created .env from .env.example — please fill in your credentials."
fi

echo ""
echo "=== Setup Complete ==="
echo "1. Edit .env with your Telegram bot token and LLM API key"
echo "2. Run: make run"
