param(
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$BaseVerifier = Join-Path $PSScriptRoot "verify-release.ps1"
$PackagePath = Join-Path $RepoRoot "package.json"

if ($Json) {
    & $BaseVerifier -PublicRelease -Json
}
else {
    & $BaseVerifier -PublicRelease
}

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$package = Get-Content $PackagePath -Raw | ConvertFrom-Json
$property = $package.scripts.PSObject.Properties["desktop:release:build:public"]
if ($null -eq $property -or [string]::IsNullOrWhiteSpace([string]$property.Value)) {
    throw "desktop:release:build:public is missing from package.json."
}

$command = [string]$property.Value
if ($command -notmatch 'tauri\.windows\.signed\.conf\.json') {
    throw "Public build command does not load tauri.windows.signed.conf.json. Refusing a potentially unsigned public build."
}
if ($command -notmatch 'desktop:release:verify:public') {
    throw "Public build command must run desktop:release:verify:public before invoking Tauri."
}

if (-not $Json) {
    Write-Host "Public build command is bound to the signed Tauri overlay." -ForegroundColor Green
}
exit 0
