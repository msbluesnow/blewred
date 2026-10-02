@echo off
setlocal
cd /d "%~dp0.."
set "SRC=%~dp0..\plugins\obs-blewred\obs-blewred.dll"
if not exist "%SRC%" set "SRC=%~dp0..\plugins\obs-blewred\dist\obs-blewred.dll"

echo ======================================================================
echo          blewred - Updating Native OBS Studio GPU Filter Plugin
echo ======================================================================
echo.

REM 1. Update user profile directories
echo [1/2] Updating user profile plugin directories...
if not exist "%APPDATA%\obs-studio\plugins\obs-blewred\bin\64bit" mkdir "%APPDATA%\obs-studio\plugins\obs-blewred\bin\64bit" >nul 2>&1
copy /Y "%SRC%" "%APPDATA%\obs-studio\plugins\obs-blewred\bin\64bit\obs-blewred.dll" >nul 2>&1

if not exist "%APPDATA%\obs-studio\obs-plugins\64bit" mkdir "%APPDATA%\obs-studio\obs-plugins\64bit" >nul 2>&1
copy /Y "%SRC%" "%APPDATA%\obs-studio\obs-plugins\64bit\obs-blewred.dll" >nul 2>&1
echo [OK] User profile directories updated.

REM 2. Update Program Files system directory
echo.
echo [2/2] Updating system OBS plugin directory...
net session >nul 2>&1
if %errorlevel% equ 0 (
    copy /Y "%SRC%" "%ProgramFiles%\obs-studio\obs-plugins\64bit\obs-blewred.dll" >nul 2>&1
    echo [OK] System directory updated.
    goto :done
)

echo [INFO] Administrator permission required for:
echo        "%ProgramFiles%\obs-studio\obs-plugins\64bit\obs-blewred.dll"
echo Prompting for elevation...
powershell -NoProfile -Command "Start-Process cmd.exe -ArgumentList '/c copy /Y \"\"%SRC%\"\" \"\"%ProgramFiles%\obs-studio\obs-plugins\64bit\obs-blewred.dll\"\"' -Verb RunAs -Wait"

:done
echo.
echo ======================================================================
echo   SUCCESS: Native OBS plugin updated!
echo   Please restart OBS Studio to load the updated plugin.
echo ======================================================================
echo.
timeout /t 4 >nul
