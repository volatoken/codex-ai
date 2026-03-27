# ============================================================
# Codex AI â€” Cai dat tu dong tren Windows Server VPS
# Chay PowerShell voi quyen Administrator:
#   powershell -ExecutionPolicy Bypass -File install-vps.ps1
#
# Cai dat: Chocolatey, Git, Rust (MSVC), Python 3.12,
#          Docker Engine, DeerFlow Backend, CLIProxyAPI
# KHONG can Docker Desktop, KHONG can Hyper-V
#
# FIX: Rust dung MSVC toolchain (khong dung GNU â€” loi dlltool)
# FIX: Docker Engine dung Microsoft install script
# ============================================================
$ErrorActionPreference = 'Stop'

function Refresh-Path {
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path','User')
    # Dam bao cargo luon trong PATH
    if (Test-Path "$HOME\.cargo\bin") {
        if ($env:Path -notlike "*\.cargo\bin*") {
            $env:Path += ";$HOME\.cargo\bin"
        }
    }
}

Write-Host ""
Write-Host "========================================================" -ForegroundColor Cyan
Write-Host "  Codex AI - Cai dat VPS Windows Server (Full Stack)"    -ForegroundColor Cyan
Write-Host "  Rust Gateway + DeerFlow Backend + CLIProxyAPI"         -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan
Write-Host ""

# ==============================================================
# [1/10] Chocolatey
# ==============================================================
Write-Host "[1/10] Cai Chocolatey..." -ForegroundColor Yellow
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (!(Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    Refresh-Path
    Write-Host "  Chocolatey da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Chocolatey da co san." -ForegroundColor Green
}

# ==============================================================
# [2/10] Git
# ==============================================================
Write-Host "[2/10] Cai Git..." -ForegroundColor Yellow
if (!(Get-Command git -ErrorAction SilentlyContinue)) {
    choco install -y git
    Refresh-Path
    Write-Host "  Git da cai xong." -ForegroundColor Green
} else {
    Write-Host "  Git da co san: $(git --version)" -ForegroundColor Green
}

# ==============================================================
# [3/10] Visual Studio Build Tools (MSVC C++ compiler cho Rust)
# ==============================================================
Write-Host "[3/10] Cai Visual Studio Build Tools..." -ForegroundColor Yellow
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsInstalled = $false
if (Test-Path $vsWhere) {
    $vsPath = & $vsWhere -latest -property installationPath 2>$null
    if ($vsPath) { $vsInstalled = $true }
}
if (!$vsInstalled) {
    Write-Host "  Tai va cai VS Build Tools (mat vai phut)..." -ForegroundColor DarkYellow
    choco install -y visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --norestart"
    Refresh-Path
    Write-Host "  VS Build Tools da cai xong." -ForegroundColor Green
} else {
    Write-Host "  VS Build Tools da co san." -ForegroundColor Green
}

# ==============================================================
# [4/10] Rust (MSVC toolchain â€” FIX loi dlltool.exe)
# ==============================================================
Write-Host "[4/10] Cai Rust (MSVC toolchain)..." -ForegroundColor Yellow
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "  Tai rustup-init.exe..." -ForegroundColor DarkYellow
    $rustupUrl = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
    $rustupExe = "$env:TEMP\rustup-init.exe"
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe
    # Cai voi default-host = msvc (QUAN TRONG: khong dung GNU)
    & $rustupExe -y --default-toolchain stable-x86_64-pc-windows-msvc --default-host x86_64-pc-windows-msvc 2>&1 | Out-Null
    $env:Path += ";$HOME\.cargo\bin"
    [System.Environment]::SetEnvironmentVariable('Path', [System.Environment]::GetEnvironmentVariable('Path','User') + ";$HOME\.cargo\bin", 'User')
    Remove-Item $rustupExe -Force -ErrorAction SilentlyContinue
    Refresh-Path
    Write-Host "  Rust da cai xong: $(rustc --version)" -ForegroundColor Green
} else {
    # Dam bao dang dung MSVC, khong phai GNU
    $currentToolchain = rustup show active-toolchain 2>$null
    if ($currentToolchain -like "*gnu*") {
        Write-Host "  Dang dung GNU toolchain â€” chuyen sang MSVC..." -ForegroundColor DarkYellow
        rustup toolchain install stable-x86_64-pc-windows-msvc 2>&1 | Out-Null
        rustup default stable-x86_64-pc-windows-msvc 2>&1 | Out-Null
        Write-Host "  Da chuyen sang MSVC: $(rustc --version)" -ForegroundColor Green
    } else {
        Write-Host "  Rust da co san (MSVC): $(rustc --version)" -ForegroundColor Green
    }
}

# ==============================================================
# [5/10] Python 3.12
# ==============================================================
Write-Host "[5/10] Cai Python 3.12..." -ForegroundColor Yellow
if (!(Get-Command python -ErrorAction SilentlyContinue)) {
    choco install -y python312
    Refresh-Path
    Write-Host "  Python da cai xong: $(python --version)" -ForegroundColor Green
} else {
    Write-Host "  Python da co san: $(python --version)" -ForegroundColor Green
}

# ==============================================================
# [6/10] Docker Engine + Docker Compose
# ==============================================================
Write-Host "[6/10] Cai Docker Engine..." -ForegroundColor Yellow
if (!(Get-Command docker -ErrorAction SilentlyContinue)) {
    # Dung Microsoft official install script
    Write-Host "  Tai Docker CE bang Microsoft install script..." -ForegroundColor DarkYellow
    try {
        Invoke-WebRequest -Uri "https://raw.githubusercontent.com/microsoft/Windows-Containers/Main/helpful_tools/Install-DockerCE/install-docker-ce.ps1" -OutFile "$env:TEMP\install-docker-ce.ps1"
        & "$env:TEMP\install-docker-ce.ps1" -NoRestart
        Remove-Item "$env:TEMP\install-docker-ce.ps1" -Force -ErrorAction SilentlyContinue
        Refresh-Path
    } catch {
        Write-Host "  Microsoft script that bai. Thu cach thu cong..." -ForegroundColor DarkYellow
        # Fallback: tai Docker Engine static binary
        $dockerVersion = "27.4.1"
        $dockerUrl = "https://download.docker.com/win/static/stable/x86_64/docker-$dockerVersion.zip"
        $dockerZip = "$env:TEMP\docker.zip"
        $dockerDir = "$env:ProgramFiles\Docker"
        Invoke-WebRequest -Uri $dockerUrl -OutFile $dockerZip
        Expand-Archive -Path $dockerZip -DestinationPath $env:ProgramFiles -Force
        Remove-Item $dockerZip -Force -ErrorAction SilentlyContinue
        $machinePath = [System.Environment]::GetEnvironmentVariable('Path','Machine')
        if ($machinePath -notlike "*Docker*") {
            [System.Environment]::SetEnvironmentVariable('Path', "$machinePath;$dockerDir", 'Machine')
        }
        Refresh-Path
        & "$dockerDir\dockerd.exe" --register-service 2>&1 | Out-Null
        Start-Service docker
    }
    Write-Host "  Docker Engine da cai xong: $(docker --version)" -ForegroundColor Green
} else {
    Write-Host "  Docker da co san: $(docker --version)" -ForegroundColor Green
}

# Docker Compose plugin
Write-Host "  Kiem tra Docker Compose..." -ForegroundColor DarkYellow
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

# ==============================================================
# [7/10] Clone du an tu GitHub
# ==============================================================
Write-Host "[7/10] Clone du an tu GitHub..." -ForegroundColor Yellow
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

# ==============================================================
# [8/10] Build Rust Gateway
# ==============================================================
Write-Host "[8/10] Build Rust Gateway (release)..." -ForegroundColor Yellow
Push-Location $projectDir
Refresh-Path

# Xoa cache cu neu co loi tu GNU toolchain truoc do
if (Test-Path "rust\target\release\.fingerprint") {
    Write-Host "  Xoa build cache cu (tranh loi toolchain cu)..." -ForegroundColor DarkYellow
    Remove-Item -Recurse -Force "rust\target\release" -ErrorAction SilentlyContinue
}

cargo build --release --manifest-path=rust\Cargo.toml
if ($LASTEXITCODE -ne 0) {
    Write-Host "  LOI: Build Rust that bai! Kiem tra lai VS Build Tools + MSVC toolchain." -ForegroundColor Red
    Write-Host "  Chay: rustup show  â€” phai thay stable-x86_64-pc-windows-msvc (default)" -ForegroundColor Red
    Pop-Location
    exit 1
}
Write-Host "  Build thanh cong: rust\target\release\codex-ai.exe" -ForegroundColor Green
Pop-Location

# ==============================================================
# [9/10] Cai CLIProxyAPI (OpenAI-compatible proxy)
# ==============================================================
Write-Host "[9/10] Cai CLIProxyAPI..." -ForegroundColor Yellow
$cliproxyDir = "$HOME\cliproxyapi"
$cliproxyExe = "$cliproxyDir\CLIProxyAPI.exe"

if (!(Test-Path $cliproxyExe)) {
    Write-Host "  Tai CLIProxyAPI tu GitHub Releases..." -ForegroundColor DarkYellow
    New-Item -ItemType Directory -Force -Path $cliproxyDir | Out-Null

    # Lay version moi nhat tu GitHub API
    try {
        $releaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/router-for-me/CLIProxyAPI/releases/latest"
        $cliproxyVersion = $releaseInfo.tag_name
        Write-Host "  Phien ban moi nhat: $cliproxyVersion" -ForegroundColor DarkYellow

        # Tim asset cho Windows amd64
        $asset = $releaseInfo.assets | Where-Object { $_.name -like "*windows*amd64*" -or $_.name -like "*Windows*x86_64*" } | Select-Object -First 1
        if ($asset) {
            $downloadUrl = $asset.browser_download_url
            $downloadFile = "$env:TEMP\cliproxyapi.zip"
            Write-Host "  Tai: $($asset.name)..." -ForegroundColor DarkYellow
            Invoke-WebRequest -Uri $downloadUrl -OutFile $downloadFile
            Expand-Archive -Path $downloadFile -DestinationPath $cliproxyDir -Force
            Remove-Item $downloadFile -Force -ErrorAction SilentlyContinue

            # Tim file .exe trong thu muc giai nen (co the nam trong subfolder)
            $exeFile = Get-ChildItem -Path $cliproxyDir -Recurse -Filter "*.exe" | Select-Object -First 1
            if ($exeFile -and $exeFile.FullName -ne $cliproxyExe) {
                Move-Item -Path $exeFile.FullName -Destination $cliproxyExe -Force
            }
        } else {
            Write-Host "  Khong tim thay file Windows. Tai thu cong tu:" -ForegroundColor Red
            Write-Host "  https://github.com/router-for-me/CLIProxyAPI/releases" -ForegroundColor White
        }
    } catch {
        Write-Host "  Loi khi tai CLIProxyAPI: $_" -ForegroundColor Red
        Write-Host "  Tai thu cong: https://github.com/router-for-me/CLIProxyAPI/releases" -ForegroundColor White
    }

    # Tao config.yaml mac dinh cho CLIProxyAPI
    if (!(Test-Path "$cliproxyDir\config.yaml")) {
        @"
# CLIProxyAPI Configuration
# Chinh sua theo nhu cau â€” xem: https://help.router-for.me/

server:
  port: 8080
  host: 127.0.0.1
"@ | Out-File -FilePath "$cliproxyDir\config.yaml" -Encoding UTF8
    }

    if (Test-Path $cliproxyExe) {
        Write-Host "  CLIProxyAPI da cai xong: $cliproxyExe" -ForegroundColor Green
    } else {
        Write-Host "  CLIProxyAPI chua cai duoc. Can tai thu cong." -ForegroundColor DarkYellow
    }
} else {
    Write-Host "  CLIProxyAPI da co san: $cliproxyExe" -ForegroundColor Green
}

# ==============================================================
# [10/10] Setup .env, thu muc, va DeerFlow Backend
# ==============================================================
Write-Host "[10/10] Setup .env, DeerFlow Backend, va thu muc..." -ForegroundColor Yellow
Push-Location $projectDir

# Tao .env tu template
if (!(Test-Path .env)) {
    Copy-Item .env.example .env
    Write-Host "  Tao .env tu .env.example." -ForegroundColor Green
} else {
    Write-Host "  .env da ton tai, giu nguyen." -ForegroundColor Green
}

# Tao cac thu muc can thiet
New-Item -ItemType Directory -Force -Path data, "workspace\projects", logs | Out-Null
Write-Host "  Thu muc data/, workspace/, logs/ da tao." -ForegroundColor Green

# Pull DeerFlow Docker image
Write-Host "  Pull DeerFlow Docker image (ghcr.io/bytedance/deer-flow:latest)..." -ForegroundColor DarkYellow
try {
    docker pull ghcr.io/bytedance/deer-flow:latest 2>&1
    Write-Host "  DeerFlow image da pull xong." -ForegroundColor Green
} catch {
    Write-Host "  Loi pull DeerFlow image. Chay sau: docker pull ghcr.io/bytedance/deer-flow:latest" -ForegroundColor DarkYellow
}

Pop-Location

# ==============================================================
# HOAN THANH
# ==============================================================
Write-Host ""
Write-Host "========================================================" -ForegroundColor Green
Write-Host "  CAI DAT HOAN TAT!" -ForegroundColor Green
Write-Host "========================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Da cai:" -ForegroundColor White
Write-Host "    [OK] Git, Python 3.12, VS Build Tools" -ForegroundColor Green
Write-Host "    [OK] Rust (MSVC toolchain â€” khong loi dlltool)" -ForegroundColor Green
Write-Host "    [OK] Docker Engine + Docker Compose" -ForegroundColor Green
Write-Host "    [OK] Rust Gateway: rust\target\release\codex-ai.exe" -ForegroundColor Green
Write-Host "    [OK] CLIProxyAPI: $cliproxyDir\CLIProxyAPI.exe" -ForegroundColor Green
Write-Host "    [OK] DeerFlow Docker image" -ForegroundColor Green
Write-Host ""
Write-Host "  ==============================" -ForegroundColor Cyan
Write-Host "  BUOC TIEP THEO:" -ForegroundColor Cyan
Write-Host "  ==============================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  1. Sua .env voi thong tin cua ban:" -ForegroundColor Yellow
Write-Host "     cd $projectDir" -ForegroundColor White
Write-Host "     notepad .env" -ForegroundColor White
Write-Host ""
Write-Host "  2. Dien vao .env:" -ForegroundColor Yellow
Write-Host "     TELEGRAM_BOT_TOKEN=<token tu BotFather>" -ForegroundColor White
Write-Host "     TELEGRAM_GROUP_ID=<group id am, vd: -1003740018844>" -ForegroundColor White
Write-Host "     TELEGRAM_ADMIN_USER_ID=<user id cua ban>" -ForegroundColor White
Write-Host "     LLM_API_KEY=<API key tu OpenRouter hoac CLIProxyAPI>" -ForegroundColor White
Write-Host ""
Write-Host "  3. Khoi dong DeerFlow Backend:" -ForegroundColor Yellow
Write-Host "     cd $projectDir" -ForegroundColor White
Write-Host "     docker compose up deerflow -d" -ForegroundColor White
Write-Host ""
Write-Host "  4. (Tuy chon) Khoi dong CLIProxyAPI:" -ForegroundColor Yellow
Write-Host "     cd $cliproxyDir" -ForegroundColor White
Write-Host "     .\CLIProxyAPI.exe" -ForegroundColor White
Write-Host "     # Roi sua .env: LLM_BASE_URL=http://localhost:8080/v1" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  5. Chay Rust Gateway (bot Telegram):" -ForegroundColor Yellow
Write-Host "     cd $projectDir" -ForegroundColor White
Write-Host "     .\rust\target\release\codex-ai.exe" -ForegroundColor White
Write-Host ""
Write-Host "  Hoac chay TAT CA bang Docker:" -ForegroundColor Yellow
Write-Host "     docker compose up -d --build" -ForegroundColor White
Write-Host ""

