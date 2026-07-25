; MuniANCI Inno Setup Script — STUB, not wired into any build
;
; STATUS: exploratory skeleton, adapted from RadarCL's installer
; (C:/Projects/RadarCL/RadarCL.iss) at Felipe's request, gated on research into
; whether Tauri + Inno Setup is workable at all (2026-07-24, 4 web searches):
;
;   - Tauri v2 has NO native Inno Setup bundler target. Windows output is either
;     NSIS (.exe, current default here per gui/tauri.conf.json bundle.targets:"all")
;     or WiX (.msi). Using Inno Setup means bypassing `cargo tauri build`'s own
;     bundling step entirely and hand-driving ISCC over the raw
;     gui/src-tauri/target/release/ output — a fully custom pipeline, not a supported
;     alternative bundler.
;   - There IS a real motivation to consider it anyway: a known Tauri/NSIS bug
;     (tauri-apps/tauri#15134) where the externalBin sidecar binary isn't replaced
;     on reinstall/upgrade — a real risk for this app's Python sidecar. A
;     hand-written Inno Setup script could fix that by controlling the install step
;     explicitly, at the cost of maintaining the whole packaging pipeline by hand.
;   - RadarCL is a much simpler case: one self-contained PyInstaller exe, no sidecar,
;     no multi-GB model distribution. MuniANCI ships a Tauri app + a Python sidecar
;     (PyInstaller --onedir per ROADMAP.md 0.4.0 D1) + ~8GB of GGUF models
;     distributed via the hybrid scheme in ROADMAP.md 0.4.0 D2 (first-run download
;     with SHA256+resume, or an offline USB pack) — none of that is expressible by
;     copying RadarCL's [Files]/[Run] sections as-is. The TODOs below mark exactly
;     where this diverges from RadarCL and needs real design work before it compiles
;     or replaces anything.
;
; Do NOT compile this yet. Do NOT treat this as MuniANCI's actual installer — that
; remains Tauri's own NSIS bundler (`cargo tauri build`) until/unless this stub is
; finished and a decision is made to switch.
;
; Requirements (once finished):
;   - Inno Setup 6.x (https://jrsoftware.org/isinfo.php)
;   - `cargo tauri build` (or an equivalent manual cargo build) run first, to produce
;     gui/src-tauri/target/release/ output — path TODO below, not yet confirmed.
;
; Build (once wired):
;   Open this file in Inno Setup Compiler and click Build > Compile
;   Output: installer\MuniANCI-vX.Y.Z-Setup.exe

#define AppName "MuniANCI"
#define AppVersion "0.5.0"
#define AppPublisher "Felipe Carvajal Brown"
#define AppURL "https://github.com/fcarvajalbrown/MuniANCI" ; TODO: confirm actual repo/support URL
#define AppExeName "muniani-gui.exe"
#define AppDescription "Escaner de cumplimiento Ley 21.663 + Asistente legal RAG offline"

[Setup]
; TODO: generate a fresh GUID for MuniANCI — never reuse RadarCL's AppId.
AppId={{TODO-GENERATE-NEW-GUID}}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} v{#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
AppComments={#AppDescription}

; Installation directory
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
AllowNoIcons=yes

; Output
OutputDir=.
OutputBaseFilename=MuniANCI-v{#AppVersion}-Setup
SetupIconFile=..\gui\icons\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}

; Compression
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes

; Appearance
WizardStyle=modern
WizardResizable=yes
ShowLanguageDialog=no
LanguageDetectionMethod=none

; Windows version requirement (Windows 10+, matches gui/tauri.conf.json target)
MinVersion=10.0

; Privileges
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

; Restart
DisableWelcomePage=no
DisableReadyPage=no
DisableFinishedPage=no

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon";   Description: "Crear un acceso directo en el &escritorio"; GroupDescription: "Iconos adicionales:"; Flags: unchecked
Name: "startmenuicon"; Description: "Crear un acceso directo en el &menu Inicio";  GroupDescription: "Iconos adicionales:";

[Files]
; TODO — main exe: confirm the real path once a manual (non-`cargo tauri build`)
; release build is wired. Left commented out so this script cannot accidentally
; "compile clean" while actually shipping nothing.
; Source: "..\gui\src-tauri\target\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; TODO — Python sidecar (Asistente backend): PyInstaller --onedir output
; (ROADMAP.md 0.4.0 D1) has no equivalent in RadarCL, which ships no sidecar at all.
; Needs its own DestDir + Flags: recursesubdirs entry once the onedir output path is
; confirmed.

; TODO — GGUF models (~8GB, ROADMAP.md 0.4.0 D2): the hybrid distribution decision
; (first-run download+resume vs. offline USB pack) means these are almost certainly
; NOT a plain [Files] entry — likely a [Code] section calling fetch_models.py logic,
; or a separate offline-pack flow. Not attempted in this stub.

; WebView2 bootstrapper: gui/tauri.conf.json already sets
; bundle.windows.webviewInstallMode.type = "offlineInstaller", which is Tauri's own
; NSIS-equivalent handling. TODO: confirm whether Inno Setup needs to replicate this
; manually (embed + invoke the bootstrapper) or whether WebView2 is assumed present.

Source: "..\gui\icons\icon.ico"; DestDir: "{app}\icons"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
; Start Menu
Name: "{group}\{#AppName}";           Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\icons\icon.ico"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"

; Desktop (optional)
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\icons\icon.ico"; Tasks: desktopicon

[Run]
; Offer to launch after install
Filename: "{app}\{#AppExeName}"; \
    Description: "Iniciar {#AppName} ahora"; \
    Flags: nowait postinstall skipifsilent

[UninstallDelete]
; TODO — RadarCL cleans up one folder ({userappdata}\.radarcl). MuniANCI's per-install
; footprint is not yet mapped for this script: config.json, an eventual license file
; (ROADMAP.md Horizonte C), and the LanceDB stores (db/, db_<comuna>/) per
; assistant/CLAUDE.md all need their real paths confirmed before an uninstall-cleanup
; entry can be written safely (deleting the wrong path is a data-loss risk).

[Messages]
WelcomeLabel2=Este instalador configurara [name/ver] en su computador.%n%nMuniANCI es un escaner de cumplimiento de ciberseguridad (Ley 21.663) con un Asistente legal RAG totalmente offline, desarrollado por Felipe Carvajal Brown.%n%nHaga clic en Siguiente para continuar.
FinishedLabel=MuniANCI se ha instalado correctamente.%n%nPuede encontrarlo en el menu Inicio o iniciarlo ahora usando la casilla de abajo.
