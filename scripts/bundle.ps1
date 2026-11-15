# Builds OZAN Synth as plugins and lays them out the way hosts expect:
#   dist\OZAN Synth.clap                                      (CLAP: the DLL renamed)
#   dist\OZAN Synth.vst3\Contents\x86_64-win\OZAN Synth.vst3  (VST3 bundle folder)
# then loads the CLAP once with ozan-lab to check that it plays.
#
#   powershell -ExecutionPolicy Bypass -File scripts\bundle.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
cargo build --release -p ozan-plugin -p ozan-lab
if ($LASTEXITCODE -ne 0) { throw 'build failed' }
$dll = Join-Path $root 'target\release\ozan_plugin.dll'
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item $dll (Join-Path $dist 'OZAN Synth.clap') -Force
$vst3 = Join-Path $dist 'OZAN Synth.vst3\Contents\x86_64-win'
New-Item -ItemType Directory -Force $vst3 | Out-Null
Copy-Item $dll (Join-Path $vst3 'OZAN Synth.vst3') -Force
& (Join-Path $root 'target\release\ozan-lab.exe') plugin (Join-Path $dist 'OZAN Synth.clap')
if ($LASTEXITCODE -ne 0) { throw 'plugin check failed' }
Write-Host "Plugins in $dist"
