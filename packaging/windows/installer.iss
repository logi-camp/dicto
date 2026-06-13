[Setup]
AppName=dicto
AppVersion={#AppVersion}
DefaultDirName={autopf}\dicto
DefaultGroupName=dicto
AllowNoIcons=yes
OutputDir=Output
OutputBaseFilename=dicto-{#AppVersion}-windows-x86_64-installer
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64

[Files]
Source: "target\x86_64-pc-windows-gnu\release\dicto.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\dicto"; Filename: "{app}\dicto.exe"
Name: "{group}\Uninstall dicto"; Filename: "{uninstallexe}"
Name: "{autodesktop}\dicto"; Filename: "{app}\dicto.exe"

[Run]
Filename: "{app}\dicto.exe"; Description: "Launch dicto"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"