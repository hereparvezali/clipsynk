[Setup]
AppName=ClipSynk
AppVersion=0.1.0
DefaultDirName={autopf}\ClipSynk
DefaultGroupName=ClipSynk
OutputDir=..\..\target\release
OutputBaseFilename=ClipSynk-Setup-v0.1.0
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "..\..\target\release\clipsynk.exe"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: autostart; Description: "Start ClipSynk automatically when Windows starts"; GroupDescription: "Startup:"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "ClipSynk"; ValueData: """{app}\clipsynk.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\clipsynk.exe"; Description: "Launch ClipSynk now"; Flags: nowait postinstall skipifsilent
