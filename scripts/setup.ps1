# Codex AI — Windows Setup Script (Rust Gateway + DeerFlow)
$ErrorActionPreference = "Stop"

Write-Host "=== Codex AI Setup (Rust + DeerFlow) ===" -ForegroundColor Cyan

# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust not found. Please install from https://rustup.rs" -ForegroundColor Red
    exit 1
}
Write-Host "Rust: $(rustc --version)" -ForegroundColor Green

# Check Docker
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Host "Docker not found. Please install Docker Desktop" -ForegroundColor Red
    exit 1
}
Write-Host "Docker: $(docker --version)" -ForegroundColor Green

# Check Docker Compose
try {
    $composeVersion = docker compose version 2>&1
    Write-Host "Docker Compose: $composeVersion" -ForegroundColor Green
} catch {
    Write-Host "Docker Compose not found. Please update Docker Desktop" -ForegroundColor Red
    exit 1
}

# Build Rust gateway
Write-Host "Building Rust gateway..."
Push-Location rust
cargo build --release
Pop-Location
Write-Host "Rust gateway built." -ForegroundColor Green

# Create directories
New-Item -ItemType Directory -Force -Path data, "workspace\projects", logs | Out-Null

# Copy env
if (-not (Test-Path .env)) {
    Copy-Item .env.example .env
    Write-Host "Created .env from .env.example" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Cyan
Write-Host "1. Edit .env with your Telegram bot token, LLM API key, etc."
Write-Host "2. Start full stack: make up"
Write-Host "   Or manually: docker compose up deerflow -d  &&  make run"
