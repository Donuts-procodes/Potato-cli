[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ForwardedArgs
)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
& (Join-Path $ScriptDir "potato.ps1") @ForwardedArgs
exit $LASTEXITCODE
