@echo off
setlocal enabledelayedexpansion
title blewred - OBS Studio Plugin Installer

echo ======================================================================
echo             blewred - Native OBS Studio Plugin Setup
echo ======================================================================
echo.

set "PLUGIN_SRC=%~dp0..\plugins\obs-blewred\dist\obs-blewred.dll"
if not exist "%PLUGIN_SRC%" (
    set "PLUGIN_SRC=%~dp0..\plugins\obs-blewred\obs-blewred.dll"
)
if not exist "%PLUGIN_SRC%" (
    echo [ERROR] Plugin binary obs-blewred.dll not found: "%PLUGIN_SRC%"
    pause
    exit /b 1
)

REM 1. Install into User Profile (OBS 28+ Standard, NO Admin Rights Required!)
echo [1/2] Installing into user profile (standard user permissions)...
set "USER_PLUGIN_DIR=%APPDATA%\obs-studio\plugins\obs-blewred\bin\64bit"
if not exist "%USER_PLUGIN_DIR%" mkdir "%USER_PLUGIN_DIR%" >nul 2>&1

copy /Y "%PLUGIN_SRC%" "%USER_PLUGIN_DIR%\obs-blewred.dll" >nul
if %errorLevel% equ 0 (
    echo [OK] Plugin installed into user profile: "%USER_PLUGIN_DIR%\obs-blewred.dll"
) else (
    echo [WARNING] Failed to copy into user profile directory.
)

REM Also copy to legacy user directory
set "LEGACY_DIR=%APPDATA%\obs-studio\obs-plugins\64bit"
if not exist "%LEGACY_DIR%" mkdir "%LEGACY_DIR%" >nul 2>&1
copy /Y "%PLUGIN_SRC%" "%LEGACY_DIR%\obs-blewred.dll" >nul 2>&1

REM 2. Configure OBS WebSocket (Default port 4455, enabled)
echo.
echo [2/2] Verifying obs-websocket configuration...
set "CFG_DIR=%APPDATA%\obs-studio\plugin_config\obs-websocket"
if not exist "%CFG_DIR%" mkdir "%CFG_DIR%" >nul 2>&1
set "CFG_FILE=%CFG_DIR%\config.json"

if not exist "%CFG_FILE%" (
    (
        echo {
        echo   "ServerEnabled": true,
        echo   "alerts_enabled": false,
        echo   "auth_required": false,
        echo   "first_load": false,
        echo   "server_enabled": true,
        echo   "server_port": 4455
        echo }
    ) > "%CFG_FILE%"
    echo [OK] Created default obs-websocket configuration: port 4455, server enabled.
) else (
    echo [OK] obs-websocket configuration is already present.
)

REM 3. Optional System directory installation (only if already running elevated)
net session >nul 2>&1
if %errorLevel% equ 0 (
    set "OBS_DIR=%ProgramFiles%\obs-studio"
    for /f "tokens=2*" %%a in ('reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\OBS Studio" /v InstallLocation 2^>nul ^| findstr /i "InstallLocation"') do (
        set "OBS_DIR=%%b"
    )
    if exist "!OBS_DIR!\bin\64bit\obs64.exe" (
        set "DEST_DIR=!OBS_DIR!\obs-plugins\64bit"
        if not exist "!DEST_DIR!" mkdir "!DEST_DIR!" >nul 2>&1
        copy /Y "%PLUGIN_SRC%" "!DEST_DIR!\obs-blewred.dll" >nul 2>&1
        if !errorLevel! equ 0 (
            echo [OK] Copied to system plugin directory: "!DEST_DIR!\obs-blewred.dll"
        )
    )
)

echo.
echo ======================================================================
echo    SUCCESS: Native blewred plugin and OBS Studio configured!
echo ======================================================================
echo.
timeout /t 3 >nul
