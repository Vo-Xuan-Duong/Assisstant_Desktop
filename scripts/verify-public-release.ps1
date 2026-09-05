param(
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$BaseVerifier = Join-Path $PSScriptRoot "verify-release.ps1"
$PackagePath = Join-Path $RepoRoot "package.json"

$package = Get-Content $PackagePath -Raw | ConvertFrom-Json
$property = $package.scripts.PSObject.Properties["desktop:release:build:public"]
if ($null -eq $property -or [string]::IsNullOrWhiteSpace([string]$property.Value)) {
    throw "desktop:release:build:public is missing from package.json."
}

$command = [string]$property.Value
$nativeRunner = Join-Path $PSScriptRoot "run-native.ps1"
$usesSignedRunner = $command -match 'run-native\.ps1\s+-Mode\s+build-public' -and
    (Test-Path $nativeRunner -PathType Leaf) -and
    ((Get-Content $nativeRunner -Raw) -match '"build-public"\s*\{\s*& pnpm --filter ''@assisstant/desktop'' tauri build --config src-tauri/tauri\.windows\.signed\.conf\.json\s*\}')
if ($command -notmatch 'tauri\.windows\.signed\.conf\.json' -and -not $usesSignedRunner) {
    throw "Public build command does not load tauri.windows.signed.conf.json. Refusing a potentially unsigned public build."
}
if ($command -notmatch 'desktop:release:verify:public') {
    throw "Public build command must run desktop:release:verify:public before invoking Tauri."
}

if (-not $Json) {
    Write-Host "Public build command is bound to the signed Tauri overlay." -ForegroundColor Green
}

if ($Json) {
    & $BaseVerifier -PublicRelease -Json
}
else {
    & $BaseVerifier -PublicRelease
}
