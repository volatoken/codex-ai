# ============================================================
# Codex AI — Cai dat tu dong tren Windows Server VPS
# Chay PowerShell voi quyen Administrator:
#   powershell -ExecutionPolicy Bypass -File install-vps.ps1
#
# Cai dat: Chocolatey, Git, Rust, Python 3.12, Docker Engine
# KHONG can Docker Desktop, KHONG can Hyper-V
# ============================================================
$ErrorActionPreference = 'Stop'

function Refresh-Path {
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
}

Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  Codex AI - Cai dat VPS Windows Server"     -ForegroundColor Cyan
Write-Host "  (Rust + Python + Docker Engine)"            -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# -- [1/8] Chocolatey --
Write-Host "[1/8] Cai Chocolatey..." -ForegroundColor Yellow
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (!(Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    Refresh-Path
    Write-Host "  Chocolatey da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Chocolatey da co san." -ForegroundColor Green
}

# -- [2/8] Git --
Write-Host "[2/8] Cai Git..." -ForegroundColor Yellow
if (!(Get-Command git -ErrorAction SilentlyContinue)) {
    choco install -y git
    Refresh-Path
    Write-Host "  Git da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Git da co san: $(git --version)" -ForegroundColor Green
}

# -- [3/8] Rust --
Write-Host "[3/8] Cai Rust (rustup)..." -ForegroundColor Yellow
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    # Cai Visual Studio Build Tools (C++ compiler cho Rust)
    Write-Host "  Cai Visual Studio Build Tools..." -ForegroundColor DarkYellow
    choco install -y visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --norestart"
    Refresh-Path

    # Cai rustup
    Write-Host "  Cai rustup..." -ForegroundColor DarkYellow
    $rustupUrl = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
    $rustupExe = "$env:TEMP\rustup-init.exe"
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe
    & $rustupExe -y --default-toolchain stable 2>&1 | Out-Null
    $env:Path += ";$HOME\.cargo\bin"
    [System.Environment]::SetEnvironmentVariable('Path', [System.Environment]::GetEnvironmentVariable('Path','User') + ";$HOME\.cargo\bin", 'User')
    Remove-Item $rustupExe -Force -ErrorAction SilentlyContinue
    Write-Host "  Rust da cai xong: $(rustc --version)" -ForegroundColor Green
} else {
    Write-Host "  Rust da co san: $(rustc --version)" -ForegroundColor Green
}

# -- [4/8] Python 3.12 --
Write-Host "[4/8] Cai Python 3.12..." -ForegroundColor Yellow
if (!(Get-Command python -ErrorAction SilentlyContinue)) {
    choco install -y python312
    Refresh-Path
    Write-Host "  Python da cai xong: $(python --version)" -ForegroundColor Green
} else {
    Write-Host "  Python da co san: $(python --version)" -ForegroundColor Green
}

# -- [5/8] Docker Engine (nhe, khong can Docker Desktop) --
Write-Host "[5/8] Cai Docker Engine..." -ForegroundColor Yellow
if (!(Get-Command docker -ErrorAction SilentlyContinue)) {
    # Bat Windows Containers feature
    Write-Host "  Bat Containers feature..." -ForegroundColor DarkYellow
    try {
        $feat = Get-WindowsFeature -Name Containers -ErrorAction SilentlyContinue
        if ($feat -and $feat.InstallState -ne 'Installed') {
            Install-WindowsFeature -Name Containers | Out-Null
            Write-Host "  Containers feature da bat." -ForegroundColor Green
        }
    } catch {
        try {
            Enable-WindowsOptionalFeature -Online -FeatureName Containers -All -NoRestart | Out-Null
        } catch {
            Write-Host "  Khong bat duoc Containers feature. Tiep tuc..." -ForegroundColor DarkYellow
        }
    }

    # Tai va cai Docker Engine
    Write-Host "  Tai Docker Engine..." -ForegroundColor DarkYellow
    $dockerVersion = "27.4.1"
    $dockerUrl = "https://download.docker.com/win/static/stable/x86_64/docker-$dockerVersion.zip"
    $dockerZip = "$env:TEMP\docker.zip"
    $dockerDir = "$env:ProgramFiles\Docker"

    Invoke-WebRequest -Uri $dockerUrl -OutFile $dockerZip
    Expand-Archive -Path $dockerZip -DestinationPath $env:ProgramFiles -Force
    Remove-Item $dockerZip -Force -ErrorAction SilentlyContinue

    # Them vao PATH
    $machinePath = [System.Environment]::GetEnvironmentVariable('Path','Machine')
    if ($machinePath -notlike "*Docker*") {
        [System.Environment]::SetEnvironmentVariable('Path', "$machinePath;$dockerDir", 'Machine')
    }
    Refresh-Path

    # Dang ky Docker service
    Write-Host "  Dang ky Docker service..." -ForegroundColor DarkYellow
    & "$dockerDir\dockerd.exe" --register-service 2>&1 | Out-Null
    Start-Service docker
    Write-Host "  Docker Engine da cai xong: $(docker --version)" -ForegroundColor Green
} else {
    Write-Host "  Docker da co san: $(docker --version)" -ForegroundColor Green
}

# -- [6/8] Docker Compose plugin --
Write-Host "[6/8] Cai Docker Compose..." -ForegroundColor Yellow
$composeOk = $false
try { docker compose version 2>&1 | Out-Null; $composeOk = $true } catch {}
if (!$composeOk) {
    $composeVersion = "2.32.4"
    $composeUrl = "https://github.com/docker/compose/releases/download/v$composeVersion/docker-compose-windows-x86_64.exe"
    $composeDir = "$env:ProgramFiles\Docker\cli-plugins"
    New-Item -ItemType Directory -Force -Path $composeDir | Out-Null
    Invoke-WebRequest -Uri $composeUrl -OutFile "$composeDir\docker-compose.exe"
    Write-Host "  Docker Compose da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Docker Compose da co san." -ForegroundColor Green
}

# -- [7/8] Clone du an --
Write-Host "[7/8] Clone du an tu GitHub..." -ForegroundColor Yellow
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

# -- [8/8] Setup --
Write-Host "[8/8] Setup .env va thu muc..." -ForegroundColor Yellow
Push-Location $projectDir

# Build Rust gateway
Write-Host "  Build Rust gateway (release)..." -ForegroundColor DarkYellow
cargo build --release --manifest-path=rust\Cargo.toml

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
Write-Host "  Da cai: Git, Rust, Python 3.12, Docker Engine, Docker Compose" -ForegroundColor White
Write-Host "  Rust gateway da build: rust\target\release\codex-ai.exe" -ForegroundColor White
Write-Host ""
Write-Host "  Buoc tiep theo:" -ForegroundColor Yellow
Write-Host "  1. Edit .env:" -ForegroundColor Yellow
Write-Host "     cd $projectDir" -ForegroundColor White
Write-Host "     notepad .env" -ForegroundColor White
Write-Host ""
Write-Host "  2. Dien TELEGRAM_BOT_TOKEN, LLM_API_KEY, v.v." -ForegroundColor Yellow
Write-Host ""
Write-Host "  3. Chay DeerFlow + Gateway:" -ForegroundColor Yellow
Write-Host "     docker compose up deerflow -d" -ForegroundColor White
Write-Host "     .\rust\target\release\codex-ai.exe" -ForegroundColor White
Write-Host ""
Write-Host "  Hoac chay het bang Docker:" -ForegroundColor Yellow
Write-Host "     docker compose up -d --build" -ForegroundColor White
Write-Host ""
