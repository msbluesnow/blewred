@echo off
setlocal enabledelayedexpansion
title blewred - Broadcast Compliance Shield

echo ========================================================
echo        blewred Stream Compliance System Launching
echo   Core Stack: Native Rust + Tauri v2 Desktop
echo   Video Engine: Real-time Screen Analysis and OBS Shield
echo   Video Buffer: OBS Studio WebSocket v5
echo ========================================================
echo.

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "RELEASE_EXE=%~dp0src-tauri\target\release\blewred.exe"
set "DEBUG_EXE=%~dp0src-tauri\target\debug\blewred.exe"

REM Ensure clean single-instance: terminate any background orphan blewred.exe and its WebView2 child processes
taskkill /F /IM blewred.exe >nul 2>&1
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name = 'msedgewebview2.exe'\" -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*com.blewred.streamcompliance*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1
timeout /t 1 /nobreak >nul

REM Ensure Windows shell and Task Manager MuiCache display strict lowercase 'blewred'
reg add "HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache" /v "%RELEASE_EXE%.FriendlyAppName" /d "blewred" /f >nul 2>&1
reg add "HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache" /v "%DEBUG_EXE%.FriendlyAppName" /d "blewred" /f >nul 2>&1

set "PATH_MARKER=%LOCALAPPDATA%\blewred\last_run_root.txt"
set "NEED_CLEAN=0"
if "%1"=="--clean" set "NEED_CLEAN=1"
if "%2"=="--clean" set "NEED_CLEAN=1"
if "%1"=="--reset" set "NEED_CLEAN=1"
if "%2"=="--reset" set "NEED_CLEAN=1"

if not exist "%PATH_MARKER%" (
    set "NEED_CLEAN=1"
) else (
    set /p LAST_ROOT=<"%PATH_MARKER%"
    if not "!LAST_ROOT!"=="%~dp0" set "NEED_CLEAN=1"
)

if "%NEED_CLEAN%"=="1" (
    echo [Launcher] Sanitizing WebView2 cache and application state...
    rd /s /q "%LOCALAPPDATA%\com.blewred.streamcompliance" >nul 2>&1
    if not exist "%LOCALAPPDATA%\blewred" mkdir "%LOCALAPPDATA%\blewred" >nul 2>&1
    echo %~dp0>"%PATH_MARKER%"
    echo [Launcher] Environment synced to: %~dp0
)

REM Quick attempt to copy native OBS plugin if target is writable
if exist "%ProgramFiles%\obs-studio\obs-plugins\64bit" (
    if not exist "%ProgramFiles%\obs-studio\obs-plugins\64bit\obs-blewred.dll" (
        copy /Y "%~dp0plugins\obs-blewred\dist\obs-blewred.dll" "%ProgramFiles%\obs-studio\obs-plugins\64bit\" >nul 2>&1
    )
)

set "NEED_BUILD=0"
if "%1"=="--build" set "NEED_BUILD=1"
if "%2"=="--build" set "NEED_BUILD=1"
if "%1"=="-b" set "NEED_BUILD=1"
if "%2"=="-b" set "NEED_BUILD=1"
if "%1"=="--rebuild" set "NEED_BUILD=1"
if "%2"=="--rebuild" set "NEED_BUILD=1"
if "%NEED_BUILD%"=="1" goto :do_build

if exist "%RELEASE_EXE%" goto :launch_release
if exist "%DEBUG_EXE%" goto :launch_debug

:do_build
echo [Launcher] Compiling latest blewred Rust desktop core...
cd /d "%~dp0src-tauri"
cargo build --release
if errorlevel 1 goto :build_error

cd /d "%~dp0"
start "" "%RELEASE_EXE%"
echo [Launcher] blewred built and launched successfully.
goto :done

:launch_release
echo [Launcher] Starting blewred Desktop Core...
cd /d "%~dp0"
start "" "%RELEASE_EXE%"
echo [Launcher] blewred application window launched.
echo [Launcher] Note: In Windows 11, the tray icon may be located inside the taskbar overflow menu (^).
goto :done

:launch_debug
echo [Launcher] Starting blewred Debug build...
cd /d "%~dp0"
start "" "%DEBUG_EXE%"
echo [Launcher] blewred application window launched.
echo [Launcher] Note: In Windows 11, the tray icon may be located inside the taskbar overflow menu (^).
goto :done

:build_error
echo.
echo An error occurred while building blewred.
pause
goto :eof

:done
endlocal
exit /b 0
