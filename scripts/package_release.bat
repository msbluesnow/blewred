@echo off
setlocal EnableDelayedExpansion
title blewred - Release Packaging Tool

echo ========================================================
echo       blewred Release Packaging ^& Distribution
echo ========================================================
echo.

cd /d "%~dp0\.."

set "VERSION=v0.5.0-alpha.1"
set "DIST_DIR=dist"
set "PORTABLE_DIR=%DIST_DIR%\portable\blewred"

echo [*] Cleaning output directory: %DIST_DIR%
if exist "%DIST_DIR%" rmdir /s /q "%DIST_DIR%"
mkdir "%PORTABLE_DIR%"
mkdir "%PORTABLE_DIR%\models"
mkdir "%PORTABLE_DIR%\extensions"
mkdir "%PORTABLE_DIR%\plugins\obs-blewred\dist"
mkdir "%PORTABLE_DIR%\config"

echo [*] Compiling release binary with Cargo...
cargo build --release --manifest-path src-tauri/Cargo.toml --bin blewred
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Cargo release build failed!
    exit /b %ERRORLEVEL%
)

echo [*] Assembling portable distribution files...
copy "src-tauri\target\release\blewred.exe" "%PORTABLE_DIR%\" >nul
xcopy "ui" "%PORTABLE_DIR%\ui" /e /i /q /y >nul
xcopy "config" "%PORTABLE_DIR%\config" /e /i /q /y >nul
xcopy "extensions\chrome" "%PORTABLE_DIR%\extensions\chrome" /e /i /q /y >nul
if exist "plugins\obs-blewred\dist\obs-blewred.dll" (
    copy "plugins\obs-blewred\dist\obs-blewred.dll" "%PORTABLE_DIR%\plugins\obs-blewred\dist\" >nul
) else (
    copy "plugins\obs-blewred\obs-blewred.dll" "%PORTABLE_DIR%\plugins\obs-blewred\dist\" >nul
)
copy "models\README.txt" "%PORTABLE_DIR%\models\" >nul
copy "models\.gitkeep" "%PORTABLE_DIR%\models\" >nul
copy "scripts\install_obs_plugin.bat" "%PORTABLE_DIR%\" >nul
copy "launch_browser_with_extension.bat" "%PORTABLE_DIR%\" >nul
copy "README.md" "%PORTABLE_DIR%\" >nul
copy "LICENSE" "%PORTABLE_DIR%\" >nul

echo [*] Packaging Portable ZIP archive...
powershell -NoProfile -ExecutionPolicy Bypass -Command "Compress-Archive -Path '%PORTABLE_DIR%' -DestinationPath '%DIST_DIR%\blewred-%VERSION%-portable.zip' -Force"

echo [*] Packaging Standalone Chrome Extension ZIP...
powershell -NoProfile -ExecutionPolicy Bypass -Command "Compress-Archive -Path 'extensions\chrome\*' -DestinationPath '%DIST_DIR%\blewred-browser-extension-%VERSION%.zip' -Force"

echo [*] Checking for Inno Setup compiler (ISCC)...
set "ISCC_EXE="
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC_EXE=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe" set "ISCC_EXE=%ProgramFiles%\Inno Setup 6\ISCC.exe"

if defined ISCC_EXE (
    echo [*] Compiling Inno Setup standalone installer...
    "%ISCC_EXE%" "build\installer\setup.iss"
    if exist "build\dist\*.exe" (
        move /y "build\dist\*.exe" "%DIST_DIR%\" >nul
    )
) else (
    echo [NOTE] Inno Setup 6 was not detected in Program Files.
    echo        To build the setup.exe installer locally, install Inno Setup 6:
    echo        choco install innosetup  OR  winget install JRSoftware.InnoSetup
)

echo [*] Generating SHA-256 Checksums...
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%DIST_DIR%\*.zip', '%DIST_DIR%\*.exe' -ErrorAction SilentlyContinue | Get-FileHash -Algorithm SHA256 | ForEach-Object { \"$($_.Hash)  $($_.Path | Split-Path -Leaf)\" } | Out-File -FilePath '%DIST_DIR%\SHA256SUMS.txt' -Encoding utf8"

echo.
echo ========================================================
echo [SUCCESS] Release artifacts packaged successfully in: %DIST_DIR%\
echo ========================================================
dir "%DIST_DIR%"
echo.
