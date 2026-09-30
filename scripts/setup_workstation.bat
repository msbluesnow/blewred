@echo off
setlocal enabledelayedexpansion
title blewred - Workstation Setup

echo ========================================================
echo        blewred Stream Compliance System Setup
echo    Automated Environment Provisioning for New PC
echo ========================================================
echo.

REM Check administrator privileges
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [i] Requesting Administrator privileges...
    powershell.exe -NoProfile -Command "Start-Process cmd.exe -ArgumentList '/c `\"%~f0`\"' -Verb RunAs"
    exit /b
)

cd /d "%~dp0"

echo [i] Launching PowerShell setup wizard...
echo.

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup_workstation.ps1"

echo.
echo ========================================================
echo Press any key to exit...
pause >nul
endlocal
