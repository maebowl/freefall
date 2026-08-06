# Build a standalone Windows binary (assets embedded) and zip it for itch.io.
#
#   .\scripts\build-native.ps1
#
# Produces dist/freefall-windows.zip containing a single self-contained
# freefall.exe — no assets folder needed alongside it. On itch.io, upload it as
# a downloadable file and tag the platform as Windows.

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent   # scripts/ -> repo root
Push-Location $root
try {
    Write-Host 'Building native (release, embedded assets)...' -ForegroundColor Cyan
    cargo build --release --features embed
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $out = Join-Path $root 'dist\windows'
    Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $out | Out-Null

    Copy-Item (Join-Path $root 'target\release\freefall.exe') $out
    Set-Content -Path (Join-Path $out 'README.txt') -Encoding utf8 -Value @'
Freefall — playtest build

Run freefall.exe to play. Everything is bundled into the one file; there is no
separate assets folder to keep next to it.

Controls
  Move        Arrow keys / WASD  (or left stick)
  Jump        Space              (or A / cross)
  Dash        Left Shift         (or left trigger)
  Walk        Hold Left Ctrl     (or right trigger)
  Pause       Esc
'@

    $zip = Join-Path $root 'dist\freefall-windows.zip'
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    Write-Host 'Zipping...' -ForegroundColor Cyan
    Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip

    $sizeMB = [math]::Round((Get-Item $zip).Length / 1MB, 1)
    Write-Host "Done: $zip ($sizeMB MB)" -ForegroundColor Green
}
finally {
    Pop-Location
}
