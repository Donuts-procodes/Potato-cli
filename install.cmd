@echo off
setlocal
echo ===================================================
echo  Potato CLI - Universal Terminal Installer (Windows)
echo ===================================================

set "SCRIPT_DIR=%~dp0"
set "BIN_SRC=%SCRIPT_DIR%target\release\potato.exe"

if not exist "%BIN_SRC%" (
    if exist "%SCRIPT_DIR%npm\win32-x64\bin\potato.exe" (
        set "BIN_SRC=%SCRIPT_DIR%npm\win32-x64\bin\potato.exe"
    ) else (
        echo Compiling native release binary...
        cargo build --release -p potato-cli --bin potato
    )
)

if not exist "%BIN_SRC%" (
    echo Error: Could not find or build potato.exe
    exit /b 1
)

:: 1. Install to Cargo bin directory
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
if not exist "%CARGO_BIN%" mkdir "%CARGO_BIN%"
copy /Y "%BIN_SRC%" "%CARGO_BIN%\potato.exe" >nul
copy /Y "%BIN_SRC%" "%CARGO_BIN%\pot.exe" >nul
echo [OK] Installed to %CARGO_BIN%\potato.exe and pot.exe

:: 2. Install to NPM bin directory if it exists
set "NPM_BIN=%APPDATA%\npm"
if exist "%NPM_BIN%" (
    copy /Y "%BIN_SRC%" "%NPM_BIN%\potato.exe" >nul
    copy /Y "%BIN_SRC%" "%NPM_BIN%\pot.exe" >nul
    if exist "%NPM_BIN%\potato.ps1" del /F /Q "%NPM_BIN%\potato.ps1"
    if exist "%NPM_BIN%\pot.ps1" del /F /Q "%NPM_BIN%\pot.ps1"
    echo [OK] Installed to %NPM_BIN%\potato.exe and pot.exe (ExecutionPolicy guard active)
)

echo.
echo Success! Potato CLI is installed globally across all terminals.
echo Test commands:
echo   potato --version
echo   pot --version
"%CARGO_BIN%\pot.exe" --version
