# Empacota o Chess Studio para Windows em dist\: .zip portátil (executável + assets).
# Uso (PowerShell, na raiz do repositório): .\scripts\package-windows.ps1 [-SkipBuild]
param([switch]$SkipBuild)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$meta = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$version = ($meta.packages | Where-Object { $_.name -eq "chess-studio" }).version
$target = $meta.target_directory
$name = "chess-studio-$version-windows-x86_64"

if (-not $SkipBuild) { cargo build --release -p chess-studio; if ($LASTEXITCODE) { throw "cargo build falhou" } }
$bin = Join-Path $target "release\chess-studio.exe"
if (-not (Test-Path $bin)) { throw "binário não encontrado: $bin" }
if (-not (Test-Path THIRD-PARTY-LICENSES)) { throw "THIRD-PARTY-LICENSES ausente (gere no Linux com scripts/gerar-licencas.sh e commite)" }

$stage = "dist\$name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $bin "$stage\chess-studio.exe"
Copy-Item -Recurse crates\chess-studio\assets "$stage\assets"
Copy-Item LICENSE-MIT, LICENSE-APACHE, THIRD-PARTY-LICENSES "$stage\"
Copy-Item docs\instalacao.md "$stage\INSTALACAO.md"

$zip = "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Get-FileHash $zip -Algorithm SHA256 | ForEach-Object { "$($_.Hash.ToLower())  $name.zip" } | Set-Content dist\SHA256SUMS-windows
Write-Host "gerado: $zip"
