; Taful Windows installer (Inno Setup 6). Built by CI; see docs/development.md.
;
; Unsigned (D14). Per-user install: no UAC prompt, files go to
; %LOCALAPPDATA%\Programs\Taful and the shortcut to the user's Start menu.
;
; Auto-update (D18): the app downloads this setup from GitHub Releases and
; runs it with /SILENT (or /VERYSILENT) /SUPPRESSMSGBOXES /NORESTART
; /WAITPID=<app pid> [/RELAUNCH=1]. Setup waits (up to 60 s) for that process
; to exit before it replaces taful.exe, and starts the new version when
; /RELAUNCH=1 is given.
; The app keeps its data in %APPDATA%\Taful (database, backups, default
; library folder); the installer never writes, reads or deletes that folder.
;
; Compile: ISCC /DAppVersion=<workspace version> installer\taful.iss
; (optional: /DNumericVersion=<x.y.z without suffix> /DSourceExe=<taful.exe>)
; Needs Inno Setup 6.3 or later (x64compatible).

#ifndef AppVersion
  #error Pass /DAppVersion=<workspace version>
#endif
#ifndef NumericVersion
  #define NumericVersion AppVersion
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
VersionInfoVersion={#NumericVersion}
DefaultDirName={autopf}\Taful
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

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Taful"; Filename: "{app}\taful.exe"

[Run]
Filename: "{app}\taful.exe"; Description: "{cm:LaunchProgram,Taful}"; Flags: nowait postinstall skipifsilent
; An update started from the app reopens it (silent installs skip the line above).
Filename: "{app}\taful.exe"; Flags: nowait; Check: ShouldRelaunch

[Code]
const
  SYNCHRONIZE = $00100000;

function OpenProcess(dwDesiredAccess: Cardinal; bInheritHandle: LongBool; dwProcessId: Cardinal): THandle;
  external 'OpenProcess@kernel32.dll stdcall';
function WaitForSingleObject(hHandle: THandle; dwMilliseconds: Cardinal): Cardinal;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(hObject: THandle): LongBool;
  external 'CloseHandle@kernel32.dll stdcall';

// /WAITPID=<pid>: the app that started this update is closing; wait for it
// so taful.exe is not in use when files are replaced.
function InitializeSetup(): Boolean;
var
  Pid: Integer;
  Process: THandle;
begin
  Result := True;
  Pid := StrToIntDef(ExpandConstant('{param:WAITPID|0}'), 0);
  if Pid > 0 then
  begin
    Process := OpenProcess(SYNCHRONIZE, False, Pid);
    if Process <> 0 then
    begin
      Log(Format('Waiting for process %d to exit', [Pid]));
      WaitForSingleObject(Process, 60000);
      CloseHandle(Process);
    end;
  end;
end;

function ShouldRelaunch(): Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;
