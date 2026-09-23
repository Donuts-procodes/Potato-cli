# ==============================================================================
# Potato CLI — PowerShell Runner (Windows Terminal, PowerShell 5.1 & Core 7+)
# ==============================================================================
[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ForwardedArgs
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$TargetExe = Join-Path $ScriptDir "target\release\potato.exe"
$NpmExe = Join-Path $ScriptDir "npm\win32-x64\bin\potato.exe"
$RunJs = Join-Path $ScriptDir "bin\run.js"

if (Test-Path $TargetExe) {
    & $TargetExe @ForwardedArgs
} elseif (Test-Path $NpmExe) {
    & $NpmExe @ForwardedArgs
} else {
    node $RunJs @ForwardedArgs
}

exit $LASTEXITCODE
