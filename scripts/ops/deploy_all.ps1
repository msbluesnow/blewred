$ErrorActionPreference = "Continue"

$rootDir = (Resolve-Path "$PSScriptRoot\..\..").Path

Write-Host "1. Terminating any running instances..."
Get-Process blewred, obs64 -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

Write-Host "2. Copying obs-blewred.dll to ProgramData and AppData..."
$srcDll = Join-Path $rootDir "plugins\obs-blewred\obs-blewred.dll"
$pdBin = "$env:ProgramData\obs-studio\plugins\obs-blewred\bin\64bit\obs-blewred.dll"
$adBin = "$env:APPDATA\obs-studio\plugins\obs-blewred\bin\64bit\obs-blewred.dll"

if (Test-Path $srcDll) {
    Copy-Item -Path $srcDll -Destination $pdBin -Force -ErrorAction SilentlyContinue
    Copy-Item -Path $srcDll -Destination $adBin -Force -ErrorAction SilentlyContinue
    Copy-Item -Path (Join-Path $rootDir "plugins\obs-blewred\data\*") -Destination "$env:ProgramData\obs-studio\plugins\obs-blewred\data" -Recurse -Force -ErrorAction SilentlyContinue
    Copy-Item -Path (Join-Path $rootDir "plugins\obs-blewred\data\*") -Destination "$env:APPDATA\obs-studio\plugins\obs-blewred\data" -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host "Copied successfully:"
    Get-Item $pdBin -ErrorAction SilentlyContinue | Select-Object FullName, Length, LastWriteTime
}

Write-Host "3. Clearing OBS crash sentinel..."
$sentinel = "$env:APPDATA\obs-studio\.sentinel"
if (Test-Path $sentinel) {
    Remove-Item "$sentinel\*" -Force -ErrorAction SilentlyContinue
}

Write-Host "4. Launching blewred (release build)..."
$blewredExe = Join-Path $rootDir "src-tauri\target\release\blewred.exe"
if (-not (Test-Path $blewredExe)) {
    $blewredExe = Join-Path $rootDir "src-tauri\target\debug\blewred.exe"
}
$proc = [wmiclass]"Win32_Process"
$resBlew = $proc.Create("`"$blewredExe`"", (Join-Path $rootDir "src-tauri"), $null)
Write-Host "blewred PID: $($resBlew.ProcessId)"

Start-Sleep -Seconds 2

Write-Host "5. Launching OBS Studio 64-bit..."
$obsDir = "$env:ProgramFiles\obs-studio\bin\64bit"
$obsExe = Join-Path $obsDir "obs64.exe"
if (Test-Path $obsExe) {
    $resObs = $proc.Create("`"$obsExe`"", $obsDir, $null)
    Write-Host "OBS Studio PID: $($resObs.ProcessId)"
}

Start-Sleep -Seconds 3
Get-Process blewred, obs64 -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, MainWindowTitle
