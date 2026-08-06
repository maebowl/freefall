# Build the WebAssembly version and package it for an itch.io HTML5 upload.
#
#   .\scripts\build-web.ps1
#
# Produces dist/freefall-web.zip — upload it to itch.io and tick
# "This file will be played in the browser". index.html sits at the zip root.
#
# Requirements: the wasm32-unknown-unknown target and the wasm-bindgen CLI
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli   (version must match the wasm-bindgen crate)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent   # scripts/ -> repo root
Push-Location $root
try {
    Write-Host 'Building wasm (wasm-release, size-optimized)...' -ForegroundColor Cyan
    cargo build --profile wasm-release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $out = Join-Path $root 'dist\web'
    Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $out | Out-Null

    Write-Host 'Running wasm-bindgen...' -ForegroundColor Cyan
    $wasm = Join-Path $root 'target\wasm32-unknown-unknown\wasm-release\freefall.wasm'
    $bindgenArgs = @(
        '--no-typescript', '--target', 'web',
        '--out-dir', $out, '--out-name', 'freefall',
        $wasm
    )
    wasm-bindgen @bindgenArgs
    if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }

    Copy-Item (Join-Path $root 'web\index.html') $out

    # Shrink the wasm if Binaryen's wasm-opt is on PATH (optional but ~halves size).
    $bgWasm = Join-Path $out 'freefall_bg.wasm'
    if (Get-Command wasm-opt -ErrorAction SilentlyContinue) {
        Write-Host 'Optimizing with wasm-opt -Oz...' -ForegroundColor Cyan
        wasm-opt -Oz -o $bgWasm $bgWasm
    } else {
        Write-Host 'wasm-opt not found - skipping size optimization (optional).' -ForegroundColor Yellow
    }

    $zip = Join-Path $root 'dist\freefall-web.zip'
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    Write-Host 'Zipping...' -ForegroundColor Cyan
    Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip

    $sizeMB = [math]::Round((Get-Item $zip).Length / 1MB, 1)
    Write-Host "Done: $zip ($sizeMB MB)" -ForegroundColor Green
    Write-Host 'On itch.io: upload this zip, tick "This file will be played in the browser",'
    Write-Host 'and set the viewport to roughly 1280x720 (or Fullscreen).'
}
finally {
    Pop-Location
}
