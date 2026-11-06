; Taful Windows installer (Inno Setup 6). Built by CI; see docs/development.md.
;
; Unsigned, no auto-update (D14). Per-user install: no UAC prompt, files go
; to %LOCALAPPDATA%\Programs\Taful and the shortcut to the user's Start menu.
; The app keeps its data in %APPDATA%\Taful (database, backups, default
; library folder); the installer never writes, reads or deletes that folder.
;
; Compile: ISCC /DAppVersion=<workspace version> /DSourceExe=<taful.exe> installer\taful.iss

#ifndef AppVersion
  #error Pass /DAppVersion=<workspace version>
#endif
#ifndef SourceExe
  #define SourceExe "..\target\release\taful.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\target\installer"
#endif

[Setup]
; Never change AppId: upgrades and uninstall find the previous install by it.
AppId={{6F1C2B7E-4D3A-4E58-9A51-7B2C0D9E8F14}
AppName=Taful
AppVersion={#AppVersion}
AppVerName=Taful {#AppVersion}
AppPublisher=Fernando Lisboa
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\Taful
DefaultGroupName=Taful
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=taful-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Pick the language from Windows; ask only when it is neither pt-BR nor English.
ShowLanguageDialog=auto
UninstallDisplayIcon={app}\taful.exe
; Close a running Taful before replacing taful.exe on upgrade.
CloseApplications=yes

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Taful"; Filename: "{app}\taful.exe"

[Run]
Filename: "{app}\taful.exe"; Description: "{cm:LaunchProgram,Taful}"; Flags: nowait postinstall skipifsilent
