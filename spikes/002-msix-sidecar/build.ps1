# SPIKE 002 build script — throwaway.
#
# Regenerates the test package from scratch. Nothing here is production code.
#
# Usage:  powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1
# Then:   Add-AppxPackage -Register .\package\AppxManifest.xml
#         Start-Process "shell:AppsFolder\<PFN>!App"
#
# NOTE: this registers a LOOSE layout. It does NOT install a real MSIX, because
# installing even a self-signed package requires machine-scope cert trust, which
# needs elevation. See README.md.

$ErrorActionPreference = 'Stop'
$spike = $PSScriptRoot
$repo = Resolve-Path (Join-Path $spike '..\..')
$sdk = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64'

Write-Host '=== 1. manifest PNGs from the app icon (headless Chrome) ==='
$chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
$svg = Join-Path $repo 'assets\brand\app-icon.svg'
New-Item -ItemType Directory -Force -Path "$spike\assets" | Out-Null
foreach ($spec in @(
        @{ n = 'Square150x150Logo.png'; s = 150 },
        @{ n = 'Square44x44Logo.png'; s = 44 },
        @{ n = 'StoreLogo.png'; s = 50 }
    )) {
    $out = Join-Path "$spike\assets" $spec.n
    & $chrome --headless --disable-gpu --hide-scrollbars --screenshot="$out" `
        --window-size="$($spec.s),$($spec.s)" "file:///$($svg -replace '\\', '/')" 2>$null | Out-Null
    Write-Host "  $($spec.n)"
}

Write-Host '=== 2. launcher (Rust, zero dependencies) ==='
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
Push-Location (Join-Path $spike 'launcher')
try { & $cargo build --release } finally { Pop-Location }

Write-Host '=== 3. assemble package layout ==='
New-Item -ItemType Directory -Force -Path "$spike\package\Assets" | Out-Null
Copy-Item (Join-Path $spike 'launcher\target\release\launcher.exe') "$spike\package\launcher.exe" -Force
Copy-Item (Get-Command node).Source "$spike\package\node.exe" -Force
Copy-Item "$spike\assets\*.png" "$spike\package\Assets\" -Force
Write-Host '  launcher.exe + node.exe + Assets'

Write-Host '=== 4. pack with makeappx ==='
$makeappx = Join-Path $sdk 'makeappx.exe'
$msix = Join-Path $spike 'CapybarasSpike.msix'
Remove-Item $msix -Force -ErrorAction SilentlyContinue
& $makeappx pack /d "$spike\package" /p $msix /o | Out-Null
Write-Host ('  ' + [math]::Round((Get-Item $msix).Length / 1MB, 1) + ' MB')

Write-Host ''
Write-Host 'Next:'
Write-Host '  Add-AppxPackage -Register ".\package\AppxManifest.xml"'
Write-Host '  Start-Process "shell:AppsFolder\<PFN>!App"'
Write-Host ''
Write-Host 'Proof lands in the VIRTUALISED store, not plain LOCALAPPDATA:'
Write-Host '  %LOCALAPPDATA%\Packages\<PFN>\LocalCache\Local\CapybarasSpike\'
