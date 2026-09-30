@echo off
setlocal
taskkill /F /IM obs64.exe 2>nul
timeout /t 2 /nobreak
set "PLUGIN_SRC=%~dp0..\..\plugins\obs-blewred\obs-blewred.dll"
if exist "%PLUGIN_SRC%" (
    copy /Y "%PLUGIN_SRC%" "%ProgramData%\obs-studio\plugins\obs-blewred\bin\64bit\obs-blewred.dll" >nul 2>&1
    copy /Y "%PLUGIN_SRC%" "%APPDATA%\obs-studio\plugins\obs-blewred\bin\64bit\obs-blewred.dll" >nul 2>&1
)
start "" "%ProgramFiles%\obs-studio\bin\64bit\obs64.exe"
