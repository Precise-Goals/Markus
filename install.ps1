# markus installer — builds and installs from source for Windows
# Usage: .\install.ps1

$ErrorActionPreference = "Stop"
$EngineDir = Join-Path $PSScriptRoot "engine"
$InstallDir = Join-Path $env:USERPROFILE ".local\bin"
$BinaryName = "markus.exe"

Write-Host "`n  ▸ MARKUS INSTALLER  v3.0.0 — Pure Rust (Windows)`n" -ForegroundColor Red

# 1. Check Rust
if (!(Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "  ◆  Rust is not installed. Please install Rust from https://rustup.rs/ and try again." -ForegroundColor Cyan
    exit 1
}

$RustVer = (rustc --version) -split ' ' | Select-Object -Index 1
Write-Host "  ✔  Rust $RustVer ready" -ForegroundColor Green

# 2. Build
Write-Host "  ◆  Building markus (release)... this may take a few minutes." -ForegroundColor Cyan
Push-Location $EngineDir
try {
    cargo build --release
} finally {
    Pop-Location
}

$BuildPath = Join-Path $EngineDir "target\release\markus-engine.exe"
if (!(Test-Path $BuildPath)) {
    Write-Host "  ✘  Build failed." -ForegroundColor Red
    exit 1
}
Write-Host "  ✔  Build complete" -ForegroundColor Green

# 3. Install
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

$DestPath = Join-Path $InstallDir $BinaryName
Copy-Item -Path $BuildPath -Destination $DestPath -Force
Write-Host "  ✔  Installed to $DestPath" -ForegroundColor Green

# 4. PATH check
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notmatch [regex]::Escape($InstallDir)) {
    Write-Host "  ◆  Adding $InstallDir to user PATH..." -ForegroundColor Cyan
    $NewPath = $UserPath + ";" + $InstallDir
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "  ✔  PATH updated. Please restart your terminal for changes to take effect." -ForegroundColor Green
}

Write-Host "`n  ✔  markus is ready!`n" -ForegroundColor Green
Write-Host "  Type 'markus' from any command prompt to start.`n" -ForegroundColor Gray
