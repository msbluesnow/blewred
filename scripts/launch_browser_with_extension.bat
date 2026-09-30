@echo off
setlocal
title blewred - Browser Extension Launcher

echo ========================================================
echo   blewred Browser Lookahead & Compliance Interceptor
echo   Auto-launching Chromium browser with extension preloaded
echo ========================================================
echo.

if exist "%~dp0extensions\chrome" (
    set "EXT_PATH=%~dp0extensions\chrome"
) else (
    set "EXT_PATH=%~dp0..\extensions\chrome"
)
set "DEFAULT_URL=https://kinobox.in"

if not "%~1"=="" set "DEFAULT_URL=%~1"

REM 1. Check Thorium Browser
if exist "%LOCALAPPDATA%\Thorium\Application\thorium.exe" (
    echo [Launcher] Found Thorium Browser at %LOCALAPPDATA%\Thorium\Application\thorium.exe
    start "" "%LOCALAPPDATA%\Thorium\Application\thorium.exe" --load-extension="%EXT_PATH%" "%DEFAULT_URL%"
    goto :launched
)

REM 2. Check Google Chrome
if exist "%ProgramFiles%\Google\Chrome\Application\chrome.exe" (
    echo [Launcher] Found Google Chrome at %ProgramFiles%\Google\Chrome\Application\chrome.exe
    start "" "%ProgramFiles%\Google\Chrome\Application\chrome.exe" --load-extension="%EXT_PATH%" "%DEFAULT_URL%"
    goto :launched
)
if exist "%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe" (
    echo [Launcher] Found Google Chrome at %ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe
    start "" "%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe" --load-extension="%EXT_PATH%" "%DEFAULT_URL%"
    goto :launched
)

REM 3. Check Microsoft Edge
if exist "%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe" (
    echo [Launcher] Found Microsoft Edge at %ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe
    start "" "%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe" --load-extension="%EXT_PATH%" "%DEFAULT_URL%"
    goto :launched
)

REM 4. Check Yandex Browser
if exist "%LOCALAPPDATA%\Yandex\YandexBrowser\Application\browser.exe" (
    echo [Launcher] Found Yandex Browser at %LOCALAPPDATA%\Yandex\YandexBrowser\Application\browser.exe
    start "" "%LOCALAPPDATA%\Yandex\YandexBrowser\Application\browser.exe" --load-extension="%EXT_PATH%" "%DEFAULT_URL%"
    goto :launched
)

echo [Launcher] No standard Chromium browser automatically found.
echo Please open your browser's Extension page (e.g. chrome://extensions or edge://extensions),
echo enable "Developer mode", and click "Load unpacked", then select:
echo %EXT_PATH%
echo.
pause
goto :done

:launched
echo [Launcher] Browser successfully launched with blewred extension active!
echo [Launcher] Opening test player: %DEFAULT_URL%
echo.

:done
endlocal
exit /b 0
