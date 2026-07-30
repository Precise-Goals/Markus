<#
.SYNOPSIS
    MARKUS - Windows Universal One-Line Installer (PowerShell)
.DESCRIPTION
    Installs markus, markus.ps1, markus.cmd, and markus.bat into %USERPROFILE%\.local\bin
    and automatically adds the directory to your Windows user PATH.
.EXAMPLE
    irm https://raw.githubusercontent.com/<YOUR_USERNAME>/markus/main/install.ps1 | iex
#>

$ErrorActionPreference = 'Stop'

# Color Tokens for PowerShell Output
$BrandColor = "Red"
$AccentColor = "Cyan"
$SuccessColor = "Green"
$WarningColor = "Yellow"

Write-Host ""
Write-Host "   __  __   _   ___  _  _ _   _ ___ " -ForegroundColor $BrandColor
Write-Host "  |  \/  | /_\ | _ \| |/ / | | / __|" -ForegroundColor $BrandColor
Write-Host "  | |\/| |/ _ \|   /| ' <| |_| \__ \" -ForegroundColor $BrandColor
Write-Host "  |_|  |_/_/ \_\_|_\|_|\_\\___/|___/" -ForegroundColor $BrandColor
Write-Host "  MARKUS - Universal AI Model Manager and Chatbot CLI Installer (Windows)`n" -ForegroundColor White

# 1. Determine installation directory
$InstallDir = Join-Path $env:USERPROFILE ".local\bin"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Write-Host "  [*] Created installation directory: $InstallDir" -ForegroundColor Gray
}

# 2. Check if running locally or downloading from remote repo
$RepoUrl = $env:MARKUS_REPO_URL
if (-not $RepoUrl) {
    $RepoUrl = "https://raw.githubusercontent.com/USER/markus/main"
}

$FilesToInstall = @("markus", "markus.ps1", "markus.cmd", "markus.bat")
$LocalDir = $PSScriptRoot
if (-not $LocalDir -or -not (Test-Path (Join-Path $LocalDir "markus"))) {
    $LocalDir = Get-Location
}

Write-Host "  [*] Installing Markus to $InstallDir..." -ForegroundColor White

foreach ($file in $FilesToInstall) {
    $targetPath = Join-Path $InstallDir $file
    $localFile = Join-Path $LocalDir $file

    if (Test-Path $localFile) {
        Copy-Item -Path $localFile -Destination $targetPath -Force
    } else {
        try {
            Write-Host "    Downloading $file..." -ForegroundColor Gray
            Invoke-WebRequest -Uri "$RepoUrl/$file" -OutFile $targetPath -UseBasicParsing
        } catch {
            Write-Host "  [WARN] Could not download $file from $RepoUrl." -ForegroundColor $WarningColor
        }
    }
}

# 3. Initialize default data directories
$ConfigDir = Join-Path $env:USERPROFILE ".config\markus"
$ModelsDir = Join-Path $env:USERPROFILE ".local\share\markus\models"
New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
New-Item -ItemType Directory -Path $ModelsDir -Force | Out-Null

Write-Host "  [OK] Successfully installed markus to $InstallDir" -ForegroundColor $SuccessColor

# 4. Check and Update User PATH environment variable
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if (-not ($userPath -split ";" -contains $InstallDir)) {
    Write-Host "  [*] Adding $InstallDir to your User PATH..." -ForegroundColor White
    $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Host "  [OK] Added to User PATH. (Restart open terminals if needed)" -ForegroundColor $SuccessColor
} else {
    Write-Host "  [OK] $InstallDir is already in your User PATH." -ForegroundColor $SuccessColor
}

# 5. Check for Git Bash / WSL
Write-Host "`n  --- Checking Bash dependency -----------------------------------------" -ForegroundColor Gray
$gitBashDetected = $false
$gitPaths = @(
    "$env:ProgramFiles\Git\bin\bash.exe",
    "${env:ProgramFiles(x86)}\Git\bin\bash.exe",
    "$env:LOCALAPPDATA\Programs\Git\bin\bash.exe",
    "C:\Program Files\Git\bin\bash.exe",
    "D:\Softwares\Git\bin\bash.exe",
    "E:\Softwares\Git\bin\bash.exe"
)
foreach ($gp in $gitPaths) {
    if (Test-Path $gp) {
        $gitBashDetected = $true
        break
    }
}
if (-not $gitBashDetected -and (Get-Command "bash.exe" -ErrorAction SilentlyContinue)) {
    $gitBashDetected = $true
}

if ($gitBashDetected) {
    Write-Host "  [OK] Compatible Bash environment detected on Windows." -ForegroundColor $SuccessColor
} else {
    Write-Host "  [WARN] Git for Windows (Git Bash) was not found." -ForegroundColor $WarningColor
    Write-Host "         Please install Git for Windows from https://git-scm.com/download/win" -ForegroundColor $WarningColor
}

Write-Host "`n  +--------------------------------------------------------------------+" -ForegroundColor $BrandColor
Write-Host "  |  Launch the interactive menu:   markus                             |" -ForegroundColor White
Write-Host "  |  Check system and status:       markus status                      |" -ForegroundColor White
Write-Host "  |  Download a model:              markus pull llama3                 |" -ForegroundColor White
Write-Host "  |  Start interactive chat:        markus run <model>                 |" -ForegroundColor White
Write-Host "  +--------------------------------------------------------------------+`n" -ForegroundColor $BrandColor
