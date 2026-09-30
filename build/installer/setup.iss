; ==============================================================================
; Inno Setup Script for blewred - Preemptive Live Stream Compliance Suite
; Creates a lightweight standalone Windows installer.
; ==============================================================================

#define MyAppName "blewred"
#define MyAppVersion "0.5.0-alpha.1"
#define MyAppPublisher "msbluesnow"
#define MyAppURL "https://github.com/msbluesnow/blewred"
#define MyAppExeName "blewred.exe"

[Setup]
AppId={{D6494F3C-8561-4A59-BF27-184719C0E3F2}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=..\dist
OutputBaseFilename=blewred_setup_v0.5.0-alpha.1
SetupIconFile=..\..\assets\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Dirs]
; Ensure models and config folders have write permissions for seamless in-app downloads
Name: "{app}\models"; Permissions: users-full
Name: "{app}\config"; Permissions: users-full
Name: "{app}\plugins\obs-blewred\dist"; Permissions: users-full

[Files]
; Main Executable
Source: "..\..\src-tauri\target\release\blewred.exe"; DestDir: "{app}"; Flags: ignoreversion

; Application Icon
Source: "..\..\assets\icons\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

; UI Frontend Assets
Source: "..\..\ui\*"; DestDir: "{app}\ui"; Flags: ignoreversion recursesubdirs createallsubdirs

; Browser Extension (for player timeline sync)
Source: "..\..\extensions\*"; DestDir: "{app}\extensions"; Flags: ignoreversion recursesubdirs createallsubdirs

; Native OBS Plugin DLL
Source: "..\..\plugins\obs-blewred\obs-blewred.dll"; DestDir: "{app}\plugins\obs-blewred\dist"; Flags: ignoreversion

; Configuration (only copy default configs if they do NOT exist, preserving user custom settings on upgrades)
Source: "..\..\config\*"; DestDir: "{app}\config"; Flags: ignoreversion recursesubdirs createallsubdirs onlyifdoesntexist uninsneveruninstall

; Models Folder Setup (Packages instructions/gitkeep WITHOUT huge .onnx files; keeps installer lightweight ~15 MB)
; Models are either downloaded automatically in-app via the Model Manager or manually placed by user.
Source: "..\..\models\README.txt"; DestDir: "{app}\models"; Flags: ignoreversion
Source: "..\..\models\.gitkeep"; DestDir: "{app}\models"; Flags: ignoreversion

; Helper Scripts
Source: "..\..\scripts\launch_browser_with_extension.bat"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
; Configure Windows Firewall rules silently to prevent stream interruption
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""blewred Core"" dir=in action=allow program=""{app}\{#MyAppExeName}"" enable=yes"; Flags: runhidden; StatusMsg: "Configuring Windows Firewall..."
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""blewred Direct OBS (UDP 51799)"" dir=in action=allow protocol=UDP localport=51799 enable=yes"; Flags: runhidden; StatusMsg: "Configuring Windows Firewall..."

Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: postinstall nowait skipifsilent

[UninstallRun]
; Remove Windows Firewall rules cleanly on uninstall
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""blewred Core"""; Flags: runhidden
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""blewred Direct OBS (UDP 51799)"""; Flags: runhidden

[UninstallDelete]
; Clean up installed assets while keeping models and user configs intact
Type: filesandordirs; Name: "{app}\ui"
Type: filesandordirs; Name: "{app}\extensions"
Type: filesandordirs; Name: "{app}\plugins"
Type: files; Name: "{app}\launch_browser_with_extension.bat"
Type: files; Name: "{app}\icon.ico"
