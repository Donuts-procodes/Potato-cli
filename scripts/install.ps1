# ==============================================================================
# Potato CLI — Windows Universal Terminal Installer
# Registers `potato` globally for PowerShell, CMD, Git Bash, Windows Terminal, WSL
# ==============================================================================
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
if (-not $ScriptDir) {
    $ScriptDir = Get-Location
}

Write-Host "🥔 Installing Potato CLI for all Windows terminals..." -ForegroundColor Cyan

# 1. Locate or compile release binary
$SourceExe = Join-Path $ScriptDir "target\release\potato.exe"
if (-not (Test-Path $SourceExe)) {
    $Prebuilt = Join-Path $ScriptDir "npm\win32-x64\bin\potato.exe"
    if (Test-Path $Prebuilt) {
        $SourceExe = $Prebuilt
    } else {
        Write-Host "Compiling native release binary..." -ForegroundColor Yellow
        & cargo build --release -p potato-cli --bin potato
    }
}

# 2. Target 1: User Cargo bin directory
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Test-Path $CargoBin)) {
    New-Item -ItemType Directory -Path $CargoBin -Force | Out-Null
}
Copy-Item $SourceExe (Join-Path $CargoBin "potato.exe") -Force
Copy-Item $SourceExe (Join-Path $CargoBin "pot.exe") -Force
Write-Host "✔ Installed: $CargoBin\potato.exe and pot.exe" -ForegroundColor Green

# 3. Target 2: User NPM global bin directory
$NpmBin = Join-Path $env:APPDATA "npm"
if (Test-Path $NpmBin) {
    Copy-Item $SourceExe (Join-Path $NpmBin "potato.exe") -Force
    Copy-Item $SourceExe (Join-Path $NpmBin "pot.exe") -Force
    # Remove any unsigned .ps1 shim that triggers ExecutionPolicy Restricted errors
    Remove-Item (Join-Path $NpmBin "potato.ps1") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $NpmBin "pot.ps1") -Force -ErrorAction SilentlyContinue
    Write-Host "✔ Installed: $NpmBin\potato.exe and pot.exe" -ForegroundColor Green
}

# 4. Verify user PATH contains at least one destination
$UserPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$CargoBin*") {
    [System.Environment]::SetEnvironmentVariable("Path", "$UserPath;$CargoBin", "User")
    $env:PATH = "$CargoBin;$env:PATH"
    Write-Host "✔ Added $CargoBin to user PATH" -ForegroundColor Green
}

Write-Host "`n🎉 Successfully installed! You can now run 'potato' from ANY terminal:" -ForegroundColor Cyan
Write-Host "   • Windows Terminal / PowerShell: potato --version" -ForegroundColor White
Write-Host "   • Windows Command Prompt (CMD):  potato --version" -ForegroundColor White
Write-Host "   • Git Bash / MSYS2:             potato --version" -ForegroundColor White
Write-Host "   • WSL:                           potato.exe --version" -ForegroundColor White
& potato --version
