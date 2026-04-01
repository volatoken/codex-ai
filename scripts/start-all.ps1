# Codex AI - Start All Services
# Run this script to start all 3 services: CLIProxyAPI, DeerFlow Adapter, Rust Gateway

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Codex AI - Starting All Services" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Load .env
$envFile = "C:\Users\Administrator\codex-ai\.env"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        if ($_ -match '^\s*([^#][^=]+)=(.*)$') {
            $key = $matches[1].Trim()
            $val = $matches[2].Trim()
            [Environment]::SetEnvironmentVariable($key, $val, "Process")
        }
    }
    Write-Host "[OK] .env loaded" -ForegroundColor Green
}

# Step 1: Kill existing processes
Write-Host "`n--- Stopping existing services ---" -ForegroundColor Yellow
Stop-Process -Name "codex-ai" -Force -ErrorAction SilentlyContinue
Stop-Process -Name "CLIProxyAPI" -Force -ErrorAction SilentlyContinue
# Kill python processes running deerflow_adapter
Get-Process python -ErrorAction SilentlyContinue | Where-Object {
    try { $_.CommandLine -match "deerflow_adapter" } catch { $false }
} | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Write-Host "[OK] Old processes cleaned" -ForegroundColor Green

# Step 2: Start CLIProxyAPI (port 8317)
Write-Host "`n--- Starting CLIProxyAPI (port 8317) ---" -ForegroundColor Yellow
$cliproxy = Start-Process -FilePath "C:\Users\Administrator\cliproxyapi\CLIProxyAPI.exe" `
    -WorkingDirectory "C:\Users\Administrator\cliproxyapi" `
    -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 3

# Check if it's running
$listening8317 = netstat -ano | Select-String ":8317.*LISTENING"
if ($listening8317) {
    Write-Host "[OK] CLIProxyAPI running on port 8317 (PID: $($cliproxy.Id))" -ForegroundColor Green
} else {
    Write-Host "[WARN] CLIProxyAPI may not be ready yet" -ForegroundColor Yellow
}

# Step 3: Start DeerFlow Adapter (port 2024)
Write-Host "`n--- Starting DeerFlow Adapter (port 2024) ---" -ForegroundColor Yellow
$env:DEER_FLOW_CONFIG_PATH = "C:\Users\Administrator\deer-flow\config.yaml"
$env:DEER_FLOW_HOME = "C:\Users\Administrator\.deer-flow"

$pythonExe = "C:\Users\Administrator\deer-flow\backend\.venv\Scripts\python.exe"
if (-not (Test-Path $pythonExe)) {
    $pythonExe = "python"
}

$adapter = Start-Process -FilePath $pythonExe `
    -ArgumentList "C:\Users\Administrator\codex-ai\deerflow_adapter.py" `
    -WorkingDirectory "C:\Users\Administrator\codex-ai" `
    -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput "C:\Users\Administrator\codex-ai\logs\adapter-stdout.log" `
    -RedirectStandardError "C:\Users\Administrator\codex-ai\logs\adapter-stderr.log"
Start-Sleep -Seconds 5

$listening2024 = netstat -ano | Select-String ":2024.*LISTENING"
if ($listening2024) {
    Write-Host "[OK] DeerFlow Adapter running on port 2024 (PID: $($adapter.Id))" -ForegroundColor Green
} else {
    Write-Host "[WARN] DeerFlow Adapter may not be ready yet - check logs\adapter-stderr.log" -ForegroundColor Yellow
}

# Step 4: Build and start Rust Gateway
Write-Host "`n--- Building Rust Gateway ---" -ForegroundColor Yellow
Push-Location "C:\Users\Administrator\codex-ai\rust"
$buildResult = cmd /c "cargo build --release 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] Rust gateway built successfully" -ForegroundColor Green
} else {
    Write-Host "[ERROR] Build failed:" -ForegroundColor Red
    Write-Host $buildResult
    Pop-Location
    exit 1
}
Pop-Location

Write-Host "`n--- Starting Rust Gateway (Telegram Bot) ---" -ForegroundColor Yellow
$gateway = Start-Process -FilePath "C:\Users\Administrator\codex-ai\rust\target\release\codex-ai.exe" `
    -WorkingDirectory "C:\Users\Administrator\codex-ai" `
    -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 3
Write-Host "[OK] Rust Gateway started (PID: $($gateway.Id))" -ForegroundColor Green

# Step 5: Verify all services
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " Service Status Check" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$procs = @(
    @{Name="CLIProxyAPI"; Port=8317},
    @{Name="DeerFlow Adapter"; Port=2024}
)

foreach ($p in $procs) {
    $check = netstat -ano | Select-String ":$($p.Port).*LISTENING"
    if ($check) {
        Write-Host "[RUNNING] $($p.Name) on port $($p.Port)" -ForegroundColor Green
    } else {
        Write-Host "[DOWN]    $($p.Name) on port $($p.Port)" -ForegroundColor Red
    }
}

$codexProc = Get-Process -Name "codex-ai" -ErrorAction SilentlyContinue
if ($codexProc) {
    Write-Host "[RUNNING] Rust Gateway (Telegram Bot) PID: $($codexProc.Id)" -ForegroundColor Green
} else {
    Write-Host "[DOWN]    Rust Gateway (Telegram Bot)" -ForegroundColor Red
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " All services started! Test on Telegram." -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
