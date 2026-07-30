@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "MARKUS_SCRIPT=%SCRIPT_DIR%markus"

if not exist "%MARKUS_SCRIPT%" (
    if exist "%USERPROFILE%\.local\bin\markus" (
        set "MARKUS_SCRIPT=%USERPROFILE%\.local\bin\markus"
    ) else if exist "%USERPROFILE%\.local\share\markus\bin\markus" (
        set "MARKUS_SCRIPT=%USERPROFILE%\.local\share\markus\bin\markus"
    )
)

if not exist "%MARKUS_SCRIPT%" (
    echo [ERROR] Could not find 'markus' core script.
    exit /b 1
)

:: 1. Check common Git Bash installation paths
if exist "%ProgramFiles%\Git\bin\bash.exe" (
    set "BASH_EXE=%ProgramFiles%\Git\bin\bash.exe"
    goto :run_bash
)
if exist "%ProgramFiles(x86)%\Git\bin\bash.exe" (
    set "BASH_EXE=%ProgramFiles(x86)%\Git\bin\bash.exe"
    goto :run_bash
)
if exist "%LOCALAPPDATA%\Programs\Git\bin\bash.exe" (
    set "BASH_EXE=%LOCALAPPDATA%\Programs\Git\bin\bash.exe"
    goto :run_bash
)
if exist "C:\Program Files\Git\bin\bash.exe" (
    set "BASH_EXE=C:\Program Files\Git\bin\bash.exe"
    goto :run_bash
)
if exist "D:\Softwares\Git\bin\bash.exe" (
    set "BASH_EXE=D:\Softwares\Git\bin\bash.exe"
    goto :run_bash
)
if exist "E:\Softwares\Git\bin\bash.exe" (
    set "BASH_EXE=E:\Softwares\Git\bin\bash.exe"
    goto :run_bash
)

:: 2. Check PATH for bash.exe
for %%i in (bash.exe) do (
    if not "%%~dp$PATH:i"=="" (
        set "BASH_EXE=%%~dp$PATH:ibash.exe"
        goto :run_bash
    )
)

:: 3. Check for wsl.exe as fallback
for %%i in (wsl.exe) do (
    if not "%%~dp$PATH:i"=="" (
        wsl.exe bash "%MARKUS_SCRIPT:\=/%" %*
        exit /b %ERRORLEVEL%
    )
)

echo [ERROR] No Bash environment detected on Windows.
echo Please install Git for Windows (Git Bash) from https://git-scm.com/download/win or enable WSL.
exit /b 1

:run_bash
set "POSIX_SCRIPT=%MARKUS_SCRIPT:\=/%"
"%BASH_EXE%" --norc --noprofile "%POSIX_SCRIPT%" %*
exit /b %ERRORLEVEL%
