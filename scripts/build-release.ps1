param([Parameter(Mandatory=$true)][string]$Version)

$ErrorActionPreference = 'Stop'

Write-Host "Building ProjectPacker v$Version" -ForegroundColor Cyan

pnpm install --frozen-lockfile
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p projectpacker-app --bin emit-bindings
pnpm --filter projectpacker-frontend typecheck

pnpm tauri build --bundles msi,nsis

$out = "dist"
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out | Out-Null }

$msi = Get-ChildItem "crates/app/target/release/bundle/msi/*.msi" | Select-Object -First 1
$exe = Get-ChildItem "crates/app/target/release/bundle/nsis/*.exe" | Select-Object -First 1

Copy-Item $msi.FullName "$out/ProjectPacker_${Version}_x64-setup.msi"
Copy-Item $exe.FullName "$out/ProjectPacker_${Version}_x64-portable.exe"

Get-FileHash "$out/ProjectPacker_${Version}_x64-setup.msi" -Algorithm SHA256 | Format-List
Get-FileHash "$out/ProjectPacker_${Version}_x64-portable.exe" -Algorithm SHA256 | Format-List

Write-Host "Done. Artifacts in $out" -ForegroundColor Green
