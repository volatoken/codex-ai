# ============================================================
# Codex AI — Cai dat tu dong tren Windows Server VPS
# Chay PowerShell voi quyen Administrator:
#   powershell -ExecutionPolicy Bypass -File install-vps.ps1
# ============================================================
$ErrorActionPreference = 'Stop'

Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  Codex AI - Cai dat VPS Windows Server"     -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# -- [1/6] Chocolatey --
Write-Host "[1/6] Cai Chocolatey..." -ForegroundColor Yellow
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (!(Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
    Write-Host "  Chocolatey da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Chocolatey da co san." -ForegroundColor Green
}

# -- [2/6] Git --
Write-Host "[2/6] Cai Git..." -ForegroundColor Yellow
if (!(Get-Command git -ErrorAction SilentlyContinue)) {
    choco install -y git
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
    Write-Host "  Git da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Git da co san: $(git --version)" -ForegroundColor Green
}

# -- [3/6] Docker Desktop --
Write-Host "[3/6] Cai Docker Desktop..." -ForegroundColor Yellow
if (!(Get-Command docker -ErrorAction SilentlyContinue)) {
    choco install -y docker-desktop
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
    Write-Host "  Docker Desktop da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Docker da co san: $(docker --version)" -ForegroundColor Green
}

# -- [4/6] Bat Hyper-V + Containers --
Write-Host "[4/6] Bat Hyper-V va Containers..." -ForegroundColor Yellow
try {
    $hyperv = Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V
    if ($hyperv.State -ne 'Enabled') {
        Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All -NoRestart | Out-Null
        Write-Host "  Hyper-V da bat." -ForegroundColor Green
    } else {
        Write-Host "  Hyper-V da bat san." -ForegroundColor Green
    }
} catch {
    Write-Host "  Khong bat duoc Hyper-V (VPS co the khong ho tro). Tiep tuc..." -ForegroundColor DarkYellow
}
try {
    $containers = Get-WindowsOptionalFeature -Online -FeatureName Containers
    if ($containers.State -ne 'Enabled') {
        Enable-WindowsOptionalFeature -Online -FeatureName Containers -All -NoRestart | Out-Null
        Write-Host "  Containers da bat." -ForegroundColor Green
    } else {
        Write-Host "  Containers da bat san." -ForegroundColor Green
    }
} catch {
    Write-Host "  Khong bat duoc Containers feature. Tiep tuc..." -ForegroundColor DarkYellow
}

# -- [5/6] Clone du an --
Write-Host "[5/6] Clone du an tu GitHub..." -ForegroundColor Yellow
$projectDir = "$HOME\codex-ai"
if (Test-Path $projectDir) {
    Write-Host "  Thu muc da ton tai. Pull moi nhat..." -ForegroundColor DarkYellow
    Push-Location $projectDir
    git pull origin main
    Pop-Location
} else {
    git clone https://github.com/volatoken/codex-ai.git $projectDir
    Write-Host "  Clone xong." -ForegroundColor Green
}

# -- [6/6] Setup --
Write-Host "[6/6] Setup .env va thu muc..." -ForegroundColor Yellow
Push-Location $projectDir
if (!(Test-Path .env)) {
    Copy-Item .env.example .env
    Write-Host "  Tao .env tu .env.example." -ForegroundColor Green
} else {
    Write-Host "  .env da ton tai, giu nguyen." -ForegroundColor Green
}
New-Item -ItemType Directory -Force -Path data, "workspace\projects", logs | Out-Null
Write-Host "  Thu muc data/, workspace/, logs/ da tao." -ForegroundColor Green
Pop-Location

# -- DONE --
Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "  CAI DAT XONG!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host ""
Write-Host "  1. RESTART lai VPS (Docker + Hyper-V can restart)" -ForegroundColor Yellow
Write-Host "  2. Sau khi restart, mo PowerShell chay:" -ForegroundColor Yellow
Write-Host "     cd $projectDir" -ForegroundColor White
Write-Host "     notepad .env" -ForegroundColor White
Write-Host "  3. Dien TELEGRAM_BOT_TOKEN, LLM_API_KEY vao .env" -ForegroundColor Yellow
Write-Host "  4. Save roi chay:" -ForegroundColor Yellow
Write-Host "     docker compose up -d --build" -ForegroundColor White
Write-Host ""
