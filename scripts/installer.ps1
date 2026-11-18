# Builds the Windows installer, dist\OZAN-Synth-<version>-Setup.exe: the program, the
# VST3 and CLAP plugins (checked by scripts\bundle.ps1 first), README and licence.
# Needs Inno Setup 6 (winget install JRSoftware.InnoSetup).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\installer.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 not found: winget install JRSoftware.InnoSetup' }

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value

& (Join-Path $PSScriptRoot 'bundle.ps1')
cargo build --release -p ozan
if ($LASTEXITCODE -ne 0) { throw 'build failed' }

& $iscc /Qp "/DAppVersion=$version" (Join-Path $root 'installer\ozan.iss')
if ($LASTEXITCODE -ne 0) { throw 'installer failed' }
Write-Host "Installer: $(Join-Path $root "dist\OZAN-Synth-$version-Setup.exe")"
