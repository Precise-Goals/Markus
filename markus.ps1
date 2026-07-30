<#
.SYNOPSIS
    Markus - AI Model Manager CLI (PowerShell Universal Wrapper for Windows)
.DESCRIPTION
    Dynamically locates an available Bash environment (Git Bash, MSYS2, Cygwin, or WSL)
    and executes the universal markus CLI script with full interactive TUI, color, and argument support
    in Windows PowerShell, PowerShell Core, and Windows Terminal.
#>

[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$ErrorActionPreference = 'Stop'

# 1. Locate compatible bash executable on Windows
function Get-MarkusBash {
    $candidates = @(
        "$env:ProgramFiles\Git\bin\bash.exe",
        "${env:ProgramFiles(x86)}\Git\bin\bash.exe",
        "$env:LOCALAPPDATA\Programs\Git\bin\bash.exe",
        "C:\Program Files\Git\bin\bash.exe",
        "D:\Softwares\Git\bin\bash.exe",
        "E:\Softwares\Git\bin\bash.exe"
    )

    # Check Windows Registry for Git for Windows
    try {
        $gitReg = Get-ItemProperty -Path "HKLM:\SOFTWARE\GitForWindows" -Name "InstallPath" -ErrorAction SilentlyContinue
        if ($gitReg -and $gitReg.InstallPath) {
            $regBash = Join-Path $gitReg.InstallPath "bin\bash.exe"
            if (Test-Path $regBash) { return $regBash }
        }
    } catch {}

    foreach ($path in $candidates) {
        if ($path -and (Test-Path $path)) {
            return $path
        }
    }

    # Check PATH for bash.exe
    $pathBash = Get-Command "bash.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
    if ($pathBash -and (Test-Path $pathBash)) {
        return $pathBash
    }

    # Check for WSL as fallback
    $pathWsl = Get-Command "wsl.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
    if ($pathWsl) {
        return "WSL:$pathWsl"
    }

    return $null
}

# 2. Locate the markus core bash script
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$MarkusScript = Join-Path $ScriptDir "markus"

if (-not (Test-Path $MarkusScript)) {
    $altPaths = @(
        "$env:USERPROFILE\.local\bin\markus",
        "$env:USERPROFILE\.local\share\markus\bin\markus",
        "$env:ProgramData\markus\bin\markus"
    )
    foreach ($alt in $altPaths) {
        if (Test-Path $alt) {
            $MarkusScript = $alt
            break
        }
    }
}

if (-not (Test-Path $MarkusScript)) {
    Write-Host "[ERROR] Could not find 'markus' main script in $ScriptDir or standard installation directories." -ForegroundColor Red
    exit 1
}

$BashExe = Get-MarkusBash
if (-not $BashExe) {
    Write-Host "[ERROR] No Bash environment detected on Windows." -ForegroundColor Red
    Write-Host "Please install Git for Windows (Git Bash) from https://git-scm.com/download/win or enable WSL." -ForegroundColor Yellow
    exit 1
}

# 3. Prepare script path formatting for Bash / WSL
if ($BashExe -like "WSL:*") {
    $WslCmd = $BashExe.Substring(4)
    # Convert Windows path to WSL path
    $WslPath = ($MarkusScript -replace '\\', '/').Replace('C:', '/mnt/c').Replace('D:', '/mnt/d').Replace('E:', '/mnt/e')
    & $WslCmd bash "$WslPath" @Arguments
    exit $LASTEXITCODE
} else {
    # Forward slashes for Git Bash / MSYS / Cygwin
    $PosixScriptPath = $MarkusScript -replace '\\', '/'
    & $BashExe "--norc" "--noprofile" "$PosixScriptPath" @Arguments
    exit $LASTEXITCODE
}
