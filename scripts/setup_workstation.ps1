# ==============================================================================
# blewred - Automated Workstation Setup & Dependency Installer
# Configures Rust, MSVC C++ Build Tools, Node.js, WebView2, Antigravity & Models
# ==============================================================================

[CmdletBinding()]
param(
    [switch]$NonInteractive,
    [switch]$ForceRebuild,
    [switch]$SkipBuild
)

$Host.UI.RawUI.WindowTitle = "blewred - Workstation Environment Setup"

function Write-Step {
    param([string]$Title, [int]$Step, [int]$Total = 8)
    Write-Host ""
    Write-Host "==================================================================" -ForegroundColor DarkCyan
    Write-Host " [$Step/$Total] $Title" -ForegroundColor Cyan
    Write-Host "==================================================================" -ForegroundColor DarkCyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host " [OK] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host " [WARN] $Message" -ForegroundColor Yellow
}

function Write-Err {
    param([string]$Message)
    Write-Host " [ERROR] $Message" -ForegroundColor Red
}

function Write-Info {
    param([string]$Message)
    Write-Host " [INFO] $Message" -ForegroundColor Gray
}

function Refresh-PathEnv {
    $userPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
    $sysPath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
    $cargoPath = Join-Path $env:USERPROFILE ".cargo\bin"
    $env:Path = "$cargoPath;$userPath;$sysPath"
}

Clear-Host
Write-Host ""
Write-Host "  ================================================================" -ForegroundColor Cyan
Write-Host "                 blewred - WORKSTATION AUTO-SETUP                " -ForegroundColor White
Write-Host "     Automated Environment Provisioning & Dependency Installer   " -ForegroundColor Gray
Write-Host "  ================================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Resolve-Path "$PSScriptRoot"
Set-Location "$ProjectRoot"
Write-Info "Project Root Directory: $ProjectRoot"

# ------------------------------------------------------------------------------
# STEP 1: Check Windows Environment & Package Managers
# ------------------------------------------------------------------------------
Write-Step "Checking Windows OS and Package Managers" 1

$isWindows = [System.Environment]::OSVersion.Platform -eq "Win32NT"
if (-not $isWindows) {
    Write-Err "This installer is designed for Microsoft Windows."
    exit 1
}
Write-Ok "Operating System: $([System.Environment]::OSVersion.VersionString)"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($isAdmin) {
    Write-Ok "Running with Administrator privileges."
} else {
    Write-Warn "Running without Administrator privileges. Component installation may request UAC."
}

$hasWinget = $false
try {
    $wingetVer = (winget --version 2>$null)
    if ($wingetVer) {
        $hasWinget = $true
        Write-Ok "Windows Package Manager detected (winget: $wingetVer)"
    }
} catch {}

if (-not $hasWinget) {
    Write-Warn "Winget not found. Installation will use direct web download fallback."
}

# ------------------------------------------------------------------------------
# STEP 2: Visual Studio C++ Build Tools (MSVC Toolchain)
# ------------------------------------------------------------------------------
Write-Step "Checking Microsoft Visual C++ Build Tools (MSVC Toolchain)" 2

$hasMsvc = $false
$progFilesX86 = [System.Environment]::GetFolderPath('ProgramFilesX86')
if (-not $progFilesX86) { $progFilesX86 = "C:\Program Files (x86)" }
$vswhere = Join-Path $progFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"

if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($vsPath) {
        $hasMsvc = $true
        Write-Ok "MSVC C++ Tools detected at: $vsPath"
    }
}

if (-not $hasMsvc) {
    if (Get-Command "link.exe" -ErrorAction SilentlyContinue) {
        $hasMsvc = $true
        Write-Ok "MSVC Linker (link.exe) found in system PATH."
    }
}

if (-not $hasMsvc) {
    Write-Warn "C++ Build Tools / Windows SDK not detected."
    Write-Info "Installing Visual Studio Build Tools with C++ Workload..."
    if ($hasWinget) {
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" --accept-source-agreements --accept-package-agreements
    } else {
        $vsInstallerUrl = "https://aka.ms/vs/17/release/vs_BuildTools.exe"
        $installerPath = Join-Path $env:TEMP "vs_BuildTools.exe"
        Write-Info "Downloading Visual Studio Build Tools..."
        Invoke-WebRequest -Uri $vsInstallerUrl -OutFile $installerPath
        Write-Info "Running silent installation..."
        Start-Process -FilePath $installerPath -ArgumentList "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" -Wait
    }
    Write-Ok "Visual Studio C++ Build Tools installed."
} else {
    Write-Ok "MSVC environment is ready for Rust native compilation."
}

# ------------------------------------------------------------------------------
# STEP 3: Rust & Cargo Toolchain
# ------------------------------------------------------------------------------
Write-Step "Checking Rust Toolchain (rustc, cargo, rustup)" 3

Refresh-PathEnv
$cargoBinPath = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$hasCargo = (Get-Command "cargo.exe" -ErrorAction SilentlyContinue) -or (Test-Path $cargoBinPath)

if ($hasCargo) {
    Refresh-PathEnv
    $rustcBin = Join-Path $env:USERPROFILE ".cargo\bin\rustc.exe"
    if (Test-Path $rustcBin) {
        $rustcVer = & $rustcBin --version 2>$null
    } else {
        $rustcVer = rustc --version 2>$null
    }
    Write-Ok "Rust toolchain is active: $rustcVer"
} else {
    Write-Warn "Rust not found. Automatically installing via rustup..."
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupExe = Join-Path $env:TEMP "rustup-init.exe"
    Write-Info "Downloading rustup-init from $rustupUrl..."
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe
    
    Write-Info "Installing Rust (default toolchain: stable-x86_64-pc-windows-msvc)..."
    Start-Process -FilePath $rustupExe -ArgumentList "-y --default-toolchain stable-x86_64-pc-windows-msvc" -Wait
    Remove-Item $rustupExe -ErrorAction SilentlyContinue
    Refresh-PathEnv
    Write-Ok "Rust installed successfully!"
}

# ------------------------------------------------------------------------------
# STEP 4: Node.js & Git
# ------------------------------------------------------------------------------
Write-Step "Checking Node.js and Git" 4

Refresh-PathEnv
$hasNode = Get-Command "node.exe" -ErrorAction SilentlyContinue
if ($hasNode) {
    $nodeVer = node --version
    Write-Ok "Node.js is installed: $nodeVer"
} else {
    Write-Warn "Node.js not found. Installing Node.js LTS..."
    if ($hasWinget) {
        winget install --id OpenJS.NodeJS.LTS -e --source winget --accept-source-agreements --accept-package-agreements
    } else {
        Write-Info "Downloading Node.js LTS installer..."
        $nodeUrl = "https://nodejs.org/dist/v20.18.0/node-v20.18.0-x64.msi"
        $nodeMsi = Join-Path $env:TEMP "node.msi"
        Invoke-WebRequest -Uri $nodeUrl -OutFile $nodeMsi
        Start-Process msiexec.exe -ArgumentList "/i `"$nodeMsi`" /qn" -Wait
        Remove-Item $nodeMsi -ErrorAction SilentlyContinue
    }
    Refresh-PathEnv
    Write-Ok "Node.js installed."
}

$hasGit = Get-Command "git.exe" -ErrorAction SilentlyContinue
if ($hasGit) {
    $gitVer = git --version
    Write-Ok "Git is installed: $gitVer"
} else {
    Write-Info "Git not detected."
    if ($hasWinget) {
        Write-Info "Installing Git via winget..."
        winget install --id Git.Git -e --source winget --accept-source-agreements --accept-package-agreements
        Refresh-PathEnv
    }
}

# ------------------------------------------------------------------------------
# STEP 5: WebView2 Runtime (Tauri Windows Requirement)
# ------------------------------------------------------------------------------
Write-Step "Checking Microsoft Edge WebView2 Runtime" 5

$hasWebView2 = $false
$wvRegPaths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)
foreach ($reg in $wvRegPaths) {
    if (Test-Path $reg) {
        $pv = (Get-ItemProperty -Path $reg -ErrorAction SilentlyContinue).pv
        if ($pv) {
            $hasWebView2 = $true
            Write-Ok "Microsoft Edge WebView2 Runtime active (version: $pv)"
            break
        }
    }
}

if (-not $hasWebView2) {
    Write-Warn "WebView2 Runtime not found. Installing Evergreen Bootstrapper..."
    if ($hasWinget) {
        winget install --id Microsoft.EdgeWebView2Runtime -e --accept-source-agreements --accept-package-agreements
    } else {
        $wvUrl = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
        $wvBoot = Join-Path $env:TEMP "MicrosoftEdgeWebview2Setup.exe"
        Invoke-WebRequest -Uri $wvUrl -OutFile $wvBoot
        Start-Process -FilePath $wvBoot -ArgumentList "/silent /install" -Wait
        Remove-Item $wvBoot -ErrorAction SilentlyContinue
    }
    Write-Ok "WebView2 Runtime installed."
}

# ------------------------------------------------------------------------------
# STEP 6: Antigravity IDE & Antigravity CLI (agy)
# ------------------------------------------------------------------------------
Write-Step "Checking Antigravity IDE & Developer Tooling" 6

$hasAgy = Get-Command "agy" -ErrorAction SilentlyContinue
$antigravityIdePaths = @(
    "$env:LOCALAPPDATA\Programs\Antigravity\Antigravity.exe",
    "$env:PROGRAMFILES\Antigravity\Antigravity.exe",
    "$env:USERPROFILE\AppData\Local\Programs\antigravity\Antigravity.exe"
)

$hasIde = $false
foreach ($p in $antigravityIdePaths) {
    if (Test-Path $p) {
        $hasIde = $true
        Write-Ok "Antigravity IDE detected at: $p"
        break
    }
}

if ($hasAgy) {
    Write-Ok "Antigravity CLI (agy) detected."
} else {
    Write-Info "Antigravity CLI (agy) not found in global PATH."
}

if (-not $hasIde -and -not $hasAgy) {
    Write-Info "To download Antigravity IDE, visit the official portal: https://antigravity.google"
}

# Ensure .vscode workspace settings exist
$vscodeDir = Join-Path $ProjectRoot ".vscode"
if (-not (Test-Path $vscodeDir)) {
    New-Item -ItemType Directory -Path $vscodeDir -Force | Out-Null
}

$extJsonPath = Join-Path $vscodeDir "extensions.json"
if (-not (Test-Path $extJsonPath)) {
    $extJson = '{"recommendations":["rust-lang.rust-analyzer","tauri-apps.tauri-vscode","tamasfe.even-better-toml","serayuzgur.crates"]}'
    Set-Content -Path $extJsonPath -Value $extJson -Encoding UTF8
    Write-Ok "Created recommended extensions configuration (.vscode/extensions.json)"
}

# ------------------------------------------------------------------------------
# STEP 7: Check Neural Models & OBS Configuration
# ------------------------------------------------------------------------------
Write-Step "Verifying AI Models and OBS Configuration" 7

$modelsDir = Join-Path $ProjectRoot "models"
$requiredModels = @(
    @{ Name = "vit_nsfw.onnx"; MinSize = 50000000; Desc = "Vision Transformer Screener" },
    @{ Name = "640m.onnx"; MinSize = 80000000; Desc = "NudeNet 640m Precision Localizer" }
)

$allModelsOk = $true
foreach ($m in $requiredModels) {
    $mPath = Join-Path $modelsDir $m.Name
    if (Test-Path $mPath) {
        $size = (Get-Item $mPath).Length
        if ($size -ge $m.MinSize) {
            $sizeMb = [math]::Round($size / 1MB, 1)
            Write-Ok "Model $($m.Name) ($($m.Desc)): $sizeMb MB [VERIFIED]"
        } else {
            Write-Warn "Model $($m.Name) file is incomplete ($size bytes)"
            $allModelsOk = $false
        }
    } else {
        Write-Warn "Model $($m.Name) not found in models/ folder"
        $allModelsOk = $false
    }
}

if (-not $allModelsOk) {
    Write-Warn "Some AI models are missing. Make sure the 'models' folder is completely downloaded."
} else {
    Write-Ok "All AI neural network models verified."
}

# OBS Studio verification & auto-configuration
$obsExe = $null
$obsCmd = Get-Command "obs64.exe" -ErrorAction SilentlyContinue
if ($obsCmd) { $obsExe = $obsCmd.Source }
if (-not $obsExe) {
    $obsReg = (Get-ItemProperty "HKLM:\SOFTWARE\OBS Studio" -ErrorAction SilentlyContinue).'(default)'
    if ($obsReg -and (Test-Path "$obsReg\bin\64bit\obs64.exe")) {
        $obsExe = "$obsReg\bin\64bit\obs64.exe"
    }
}
if (-not $obsExe) {
    $commonObsPaths = @(
        "$env:ProgramFiles\obs-studio\bin\64bit\obs64.exe",
        "${env:ProgramFiles(x86)}\obs-studio\bin\64bit\obs64.exe",
        "D:\Program Files\obs-studio\bin\64bit\obs64.exe"
    )
    foreach ($p in $commonObsPaths) {
        if (Test-Path $p) { $obsExe = $p; break }
    }
}

if ($obsExe) {
    Write-Ok "OBS Studio detected at: $obsExe"
} else {
    Write-Info "OBS Studio not detected. If streaming, install via: winget install OBSProject.OBSStudio"
}

# Auto-configure OBS WebSocket in AppData
$obsWsDir = Join-Path $env:APPDATA "obs-studio\plugin_config\obs-websocket"
if (-not (Test-Path $obsWsDir)) {
    New-Item -ItemType Directory -Path $obsWsDir -Force | Out-Null
}
$obsWsCfg = Join-Path $obsWsDir "config.json"
if (-not (Test-Path $obsWsCfg)) {
    $defaultWsCfg = '{"alerts_enabled":false,"auth_required":false,"first_load":false,"server_enabled":true,"ServerEnabled":true,"server_port":4455}'
    Set-Content -Path $obsWsCfg -Value $defaultWsCfg -Encoding UTF8
    Write-Ok "Created default obs-websocket configuration (port 4455, auto-enabled)"
} else {
    try {
        $cfgJson = Get-Content $obsWsCfg -Raw | ConvertFrom-Json
        $needSave = $false
        if ($cfgJson.server_enabled -ne $true) { $cfgJson.server_enabled = $true; $needSave = $true }
        if ($cfgJson.ServerEnabled -ne $true) { $cfgJson.ServerEnabled = $true; $needSave = $true }
        if ($needSave) {
            $cfgJson | ConvertTo-Json | Set-Content -Path $obsWsCfg -Encoding UTF8
            Write-Ok "Enabled obs-websocket server in $obsWsCfg"
        } else {
            Write-Ok "obs-websocket server is already enabled in config."
        }
    } catch {}
}

# Auto-install OBS Native Plugin (obs-blewred.dll) to user profile
$pluginSrc = Join-Path $ProjectRoot "plugins\obs-blewred\obs-blewred.dll"
if (Test-Path $pluginSrc) {
    $userPluginDir = Join-Path $env:APPDATA "obs-studio\plugins\obs-blewred\bin\64bit"
    if (-not (Test-Path $userPluginDir)) {
        New-Item -ItemType Directory -Path $userPluginDir -Force | Out-Null
    }
    Copy-Item -Path $pluginSrc -Destination (Join-Path $userPluginDir "obs-blewred.dll") -Force
    Write-Ok "Installed blewred OBS plugin to user profile: $userPluginDir"
}

# ------------------------------------------------------------------------------
# STEP 8: Project Compilation & Verification
# ------------------------------------------------------------------------------
Write-Step "Compiling blewred Desktop Binary" 8

Refresh-PathEnv

$cargoExe = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargoExe)) {
    $cargoCmd = Get-Command "cargo.exe" -ErrorAction SilentlyContinue
    if ($cargoCmd) { $cargoExe = $cargoCmd.Source }
}

$releaseExe = Join-Path $ProjectRoot "src-tauri\target\release\blewred.exe"

if ($SkipBuild) {
    $needBuild = $false
    Write-Info "Compilation skipped via -SkipBuild flag."
} elseif ($ForceRebuild) {
    $needBuild = $true
    Write-Info "Forced rebuild requested via -ForceRebuild flag."
} elseif (Test-Path $releaseExe) {
    $lastWrite = (Get-Item $releaseExe).LastWriteTime
    Write-Ok "Release binary already exists: $releaseExe ($lastWrite)"
    if ($NonInteractive) {
        $needBuild = $false
    } else {
        Write-Host ""
        $rebuild = Read-Host "Do you want to run a fresh release build? (y/N)"
        if ($rebuild -eq 'y' -or $rebuild -eq 'Y') {
            $needBuild = $true
        } else {
            $needBuild = $false
        }
    }
} else {
    $needBuild = $true
}

if ($needBuild) {
    Write-Info "Building blewred Core (cargo build --release)..."
    Set-Location (Join-Path $ProjectRoot "src-tauri")
    
    & $cargoExe build --release
    
    if ($LASTEXITCODE -eq 0 -and (Test-Path $releaseExe)) {
        Write-Ok "blewred successfully compiled to release binary!"
    } else {
        Write-Err "Build failed. Check compiler output above."
        Set-Location "$ProjectRoot"
        exit 1
    }
    Set-Location "$ProjectRoot"
}

# Create Desktop Shortcut
$desktopPath = [System.Environment]::GetFolderPath("Desktop")
$shortcutPath = Join-Path $desktopPath "blewred Compliance Shield.lnk"
try {
    $wshShell = New-Object -ComObject WScript.Shell
    $shortcut = $wshShell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = Join-Path $ProjectRoot "run.bat"
    $shortcut.WorkingDirectory = $ProjectRoot
    $iconPath = Join-Path $ProjectRoot "src-tauri\icons\icon.ico"
    if (-not (Test-Path $iconPath)) {
        $iconPath = Join-Path $ProjectRoot "ui\logo.png"
    }
    if (Test-Path $iconPath) {
        $shortcut.IconLocation = $iconPath
    }
    $shortcut.Description = "blewred - Stream Compliance and Video Shield"
    $shortcut.Save()
    Write-Ok "Desktop shortcut created: $shortcutPath"
} catch {
    Write-Info "Could not create desktop shortcut (non-critical)."
}

# ------------------------------------------------------------------------------
# SUMMARY & LAUNCH
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "==================================================================" -ForegroundColor Green
Write-Host "          WORKSTATION SETUP COMPLETED SUCCESSFULLY!               " -ForegroundColor Green
Write-Host "==================================================================" -ForegroundColor Green
Write-Host ""
Write-Host " All required dependencies are configured and verified:" -ForegroundColor White
Write-Host "  - Rust & Cargo Toolchain (stable MSVC x64)" -ForegroundColor Gray
Write-Host "  - Microsoft Visual C++ Build Tools & Windows SDK" -ForegroundColor Gray
Write-Host "  - Microsoft Edge WebView2 Evergreen Runtime" -ForegroundColor Gray
Write-Host "  - DirectML GPU Neural Network Acceleration" -ForegroundColor Gray
Write-Host "  - blewred executable built and ready" -ForegroundColor Gray
Write-Host ""
Write-Host " How to run the application:" -ForegroundColor Cyan
Write-Host "  1. Double click the desktop shortcut or 'run.bat'" -ForegroundColor White
Write-Host "  2. For development, open the project folder in Antigravity IDE / VS Code" -ForegroundColor White
Write-Host ""

if (-not $NonInteractive) {
    $launchNow = Read-Host "Launch blewred right now? (Y/n)"
    if ($launchNow -ne 'n' -and $launchNow -ne 'N') {
        Write-Info "Launching blewred..."
        Start-Process -FilePath (Join-Path $ProjectRoot "run.bat") -WorkingDirectory $ProjectRoot
    }
}
