# Codex AI — Windows Setup Script
$ErrorActionPreference = "Stop"

Write-Host "=== Codex AI Setup ===" -ForegroundColor Cyan

# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust not found. Please install from https://rustup.rs" -ForegroundColor Red
    exit 1
}
Write-Host "Rust: $(rustc --version)" -ForegroundColor Green

# Check Python
$pythonCmd = if (Get-Command python -ErrorAction SilentlyContinue) { "python" }
             elseif (Get-Command python3 -ErrorAction SilentlyContinue) { "python3" }
             else { $null }

if (-not $pythonCmd) {
    Write-Host "Python not found. Please install Python 3.11+" -ForegroundColor Red
    exit 1
}
Write-Host "Python: $(& $pythonCmd --version)" -ForegroundColor Green

# Setup Python venv
Write-Host "Creating Python virtual environment..."
& $pythonCmd -m venv python\.venv
& python\.venv\Scripts\Activate.ps1
pip install -r python\requirements.txt
Write-Host "Python dependencies installed." -ForegroundColor Green

# Build Rust
Write-Host "Building Rust core..."
Push-Location rust
cargo build --release
Pop-Location

# Create directories
New-Item -ItemType Directory -Force -Path data, "workspace\projects", logs | Out-Null

# Copy env
if (-not (Test-Path .env)) {
    Copy-Item .env.example .env
    Write-Host "Created .env from .env.example" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Cyan
Write-Host "1. Edit .env with your Telegram bot token and LLM API key"
Write-Host "2. Run: make run"
