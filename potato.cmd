@echo off
if exist "%~dp0target\release\potato.exe" (
    "%~dp0target\release\potato.exe" %*
) else (
    node "%~dp0bin\run.js" %*
)
