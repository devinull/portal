#ifndef PortalVersion
  #define PortalVersion "0.0.1"
#endif

#ifndef PortalBinary
  #define PortalBinary "..\..\target\release\portal.exe"
#endif

#ifndef PortalOutputDir
  #define PortalOutputDir "..\..\dist"
#endif

[Setup]
AppId={{8E3D0A51-1E52-49C0-B606-7D30D7D28465}
AppName=Portal
AppVersion={#PortalVersion}
AppVerName=Portal {#PortalVersion}
AppPublisher=Portal contributors
AppPublisherURL=https://github.com/devinull/portal
AppSupportURL=https://github.com/devinull/portal/issues
AppUpdatesURL=https://github.com/devinull/portal/releases
VersionInfoVersion={#PortalVersion}
VersionInfoDescription=Portal terminal interface
DefaultDirName={autopf}\Portal
DefaultGroupName=Portal
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
UsePreviousPrivileges=yes
OutputDir={#PortalOutputDir}
OutputBaseFilename=PortalSetup-{#PortalVersion}-x86_64
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern dark includetitlebar hidebevels
WizardSizePercent=110
SetupLogging=yes
ChangesEnvironment=yes
LicenseFile=..\..\LICENSE
UninstallDisplayName=Portal
UninstallDisplayIcon={app}\portal.exe
CloseApplications=no
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add Portal to PATH"; GroupDescription: "Command-line integration:"; Flags: checkedonce
Name: "startmenu"; Description: "Create a Start menu shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce

[Files]
Source: "{#PortalBinary}"; DestDir: "{app}"; DestName: "portal.exe"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Portal"; Filename: "{app}\portal.exe"; WorkingDir: "{%USERPROFILE}"; Tasks: startmenu
Name: "{group}\Uninstall Portal"; Filename: "{uninstallexe}"; Tasks: startmenu

[Run]
Filename: "{app}\portal.exe"; Description: "Launch Portal"; WorkingDir: "{%USERPROFILE}"; Flags: nowait postinstall skipifsilent
Filename: "https://tailscale.com/download/windows"; Description: "Get Tailscale (not detected)"; Flags: shellexec postinstall skipifsilent unchecked; Check: not TailscaleInstalled
Filename: "https://github.com/rustdesk/rustdesk/releases/latest"; Description: "Get RustDesk (not detected)"; Flags: shellexec postinstall skipifsilent unchecked; Check: not RustDeskInstalled

[Code]
function NormalizedPath(const Value: String): String;
begin
  Result := Trim(Value);
  while (Length(Result) > 3) and (Result[Length(Result)] = '\') do
    Delete(Result, Length(Result), 1);
end;

function PathContains(const PathValue, Entry: String): Boolean;
var
  Remaining, Item: String;
  Separator: Integer;
begin
  Result := False;
  Remaining := PathValue;
  while Remaining <> '' do
  begin
    Separator := Pos(';', Remaining);
    if Separator = 0 then
    begin
      Item := Remaining;
      Remaining := '';
    end
    else
    begin
      Item := Copy(Remaining, 1, Separator - 1);
      Delete(Remaining, 1, Separator);
    end;
    if CompareText(NormalizedPath(Item), NormalizedPath(Entry)) = 0 then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

function PortalRegistryRoot: Integer;
begin
  if IsAdminInstallMode then
    Result := HKLM
  else
    Result := HKCU;
end;

function EnvironmentRegistryKey: String;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

procedure AddPortalToPath;
var
  CurrentPath, PortalPath: String;
begin
  PortalPath := ExpandConstant('{app}');
  if not RegQueryStringValue(
    PortalRegistryRoot, EnvironmentRegistryKey, 'Path', CurrentPath) then
    CurrentPath := '';
  if not PathContains(CurrentPath, PortalPath) then
  begin
    if (CurrentPath <> '') and (CurrentPath[Length(CurrentPath)] <> ';') then
      CurrentPath := CurrentPath + ';';
    RegWriteExpandStringValue(
      PortalRegistryRoot, EnvironmentRegistryKey, 'Path', CurrentPath + PortalPath);
    RegWriteDWordValue(
      PortalRegistryRoot, 'Software\Portal', 'PathAddedByInstaller', 1);
  end;
end;

procedure RemovePortalFromPath;
var
  CurrentPath, PortalPath, Remaining, Item, NewPath: String;
  Separator: Integer;
  PathAdded: Cardinal;
begin
  if not RegQueryDWordValue(
    PortalRegistryRoot, 'Software\Portal', 'PathAddedByInstaller', PathAdded) or
    (PathAdded <> 1) then
    Exit;
  PortalPath := ExpandConstant('{app}');
  if not RegQueryStringValue(
    PortalRegistryRoot, EnvironmentRegistryKey, 'Path', CurrentPath) then
    Exit;

  Remaining := CurrentPath;
  NewPath := '';
  while Remaining <> '' do
  begin
    Separator := Pos(';', Remaining);
    if Separator = 0 then
    begin
      Item := Remaining;
      Remaining := '';
    end
    else
    begin
      Item := Copy(Remaining, 1, Separator - 1);
      Delete(Remaining, 1, Separator);
    end;

    if (Trim(Item) <> '') and
       (CompareText(NormalizedPath(Item), NormalizedPath(PortalPath)) <> 0) then
    begin
      if NewPath <> '' then
        NewPath := NewPath + ';';
      NewPath := NewPath + Item;
    end;
  end;
  RegWriteExpandStringValue(
    PortalRegistryRoot, EnvironmentRegistryKey, 'Path', NewPath);
  RegDeleteValue(
    PortalRegistryRoot, 'Software\Portal', 'PathAddedByInstaller');
  RegDeleteKeyIfEmpty(PortalRegistryRoot, 'Software\Portal');
end;

function TailscaleInstalled: Boolean;
begin
  Result :=
    (FileSearch('tailscale.exe', GetEnv('PATH')) <> '') or
    FileExists(ExpandConstant('{pf}\Tailscale\tailscale.exe')) or
    FileExists(ExpandConstant('{pf64}\Tailscale\tailscale.exe')) or
    FileExists(ExpandConstant('{localappdata}\Tailscale\tailscale.exe'));
end;

function RustDeskInstalled: Boolean;
begin
  Result :=
    (FileSearch('rustdesk.exe', GetEnv('PATH')) <> '') or
    FileExists(ExpandConstant('{pf}\RustDesk\rustdesk.exe')) or
    FileExists(ExpandConstant('{pf64}\RustDesk\rustdesk.exe')) or
    FileExists(ExpandConstant('{localappdata}\Programs\RustDesk\rustdesk.exe'));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddPortalToPath;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemovePortalFromPath;
end;
