#!/usr/bin/env bash
# Codex AI — Linux/Mac Setup Script (Rust Gateway + DeerFlow)
set -e

echo "=== Codex AI Setup (Rust + DeerFlow) ==="

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
echo "Rust: $(rustc --version)"

# Check Docker
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker not found. Please install Docker."
    exit 1
fi
echo "Docker: $(docker --version)"

# Check Docker Compose
if ! docker compose version &> /dev/null; then
    echo "ERROR: Docker Compose not found. Please install docker-compose-plugin."
    exit 1
fi
echo "Docker Compose: $(docker compose version)"

# Build Rust gateway
echo "Building Rust gateway..."
cd rust
cargo build --release
cd ..
echo "Rust gateway built."

# Create directories
mkdir -p data workspace/projects logs

# Copy env if needed
if [ ! -f .env ]; then
    cp .env.example .env
    echo "Created .env from .env.example — please fill in your credentials."
fi

echo ""
echo "=== Setup Complete ==="
echo "1. Edit .env with your Telegram bot token, LLM API key, etc."
echo "2. Start full stack: make up"
echo "   Or manually: docker compose up deerflow -d  &&  make run"
