; Windows installer for OZAN Synth (Inno Setup 6). Build it with
;   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\installer.ps1
; which builds the program and plugins first and passes the version.

#ifndef AppVersion
  #error Pass the version: ISCC /DAppVersion=1.0.0 installer\ozan.iss
#endif

[Setup]
; Never change the AppId: Windows recognises updates of the same program by it.
AppId={{1F6B8D6D-99CA-46BA-99CF-2BC56478E103}
AppName=OZAN Synth
AppVersion={#AppVersion}
AppVerName=OZAN Synth {#AppVersion}
AppPublisher=OZAN Synth
AppPublisherURL=https://github.com/sozden/ozan-synth
AppSupportURL=https://github.com/sozden/ozan-synth/issues
AppUpdatesURL=https://github.com/sozden/ozan-synth/releases
DefaultDirName={autopf}\OZAN Synth
DefaultGroupName=OZAN Synth
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=OZAN-Synth-{#AppVersion}-Setup
SetupIconFile=..\crates\ozan\assets\icon.ico
UninstallDisplayIcon={app}\ozan.exe
UninstallDisplayName=OZAN Synth
; The plugins go to the shared VST3 and CLAP folders, which need administrator rights.
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[CustomMessages]
english.TypeFull=Program and plugins
english.TypeCustom=Choose
english.CompApp=Standalone program
english.CompVst3=VST3 plugin (for most DAWs)
english.CompClap=CLAP plugin (REAPER, Bitwig and others)
turkish.TypeFull=Program ve eklentiler
turkish.TypeCustom=Seçerek
turkish.CompApp=Bağımsız program
turkish.CompVst3=VST3 eklentisi (çoğu DAW için)
turkish.CompClap=CLAP eklentisi (REAPER, Bitwig ve diğerleri)

[Types]
Name: "full"; Description: "{cm:TypeFull}"
Name: "custom"; Description: "{cm:TypeCustom}"; Flags: iscustom

[Components]
Name: "app"; Description: "{cm:CompApp}"; Types: full custom
Name: "vst3"; Description: "{cm:CompVst3}"; Types: full custom
Name: "clap"; Description: "{cm:CompClap}"; Types: full

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Components: app; Flags: unchecked

[Files]
Source: "..\target\release\ozan.exe"; DestDir: "{app}"; Components: app; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\dist\OZAN Synth.vst3\*"; DestDir: "{commoncf64}\VST3\OZAN Synth.vst3"; Components: vst3; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\dist\OZAN Synth.clap"; DestDir: "{commoncf64}\CLAP"; Components: clap; Flags: ignoreversion

[Icons]
Name: "{group}\OZAN Synth"; Filename: "{app}\ozan.exe"; Components: app
Name: "{group}\{cm:UninstallProgram,OZAN Synth}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\OZAN Synth"; Filename: "{app}\ozan.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ozan.exe"; Description: "{cm:LaunchProgram,OZAN Synth}"; Components: app; Flags: nowait postinstall skipifsilent

; Settings and the user's own presets (%APPDATA%\Ozan) stay when it is uninstalled.
