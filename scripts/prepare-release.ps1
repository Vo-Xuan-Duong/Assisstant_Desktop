param(
    [switch]$AssetsOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$TauriDir = Join-Path $RepoRoot "apps\desktop\src-tauri"
$IconDir = Join-Path $TauriDir "icons"
$IconPayload = Join-Path $IconDir "icon.ico.b64"
$IconOutput = Join-Path $IconDir "icon.ico"
$ExpectedIconSha256 = "ea40a92ac075efbe3628dcc5777368408b516a93228a04f5cc3e998f9869d7d5"

function Require-Command {
    param([string]$Name)
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Required command is not available in PATH: $Name"
    }
    return $command
}

function Materialize-ReleaseIcon {
    if (-not (Test-Path $IconPayload -PathType Leaf)) {
        throw "Missing tracked icon payload: $IconPayload"
    }

    New-Item -ItemType Directory -Force -Path $IconDir | Out-Null
    $encoded = (Get-Content $IconPayload -Raw) -replace '\s', ''
    try {
        $bytes = [Convert]::FromBase64String($encoded)
    }
    catch {
        throw "Release icon payload is not valid Base64: $($_.Exception.Message)"
    }

    if ($bytes.Length -lt 6 -or $bytes[0] -ne 0 -or $bytes[1] -ne 0 -or $bytes[2] -ne 1 -or $bytes[3] -ne 0) {
        throw "Decoded release icon does not contain a valid ICO header."
    }

    [IO.File]::WriteAllBytes($IconOutput, $bytes)
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        $actualHash = [BitConverter]::ToString($sha256.ComputeHash([IO.File]::ReadAllBytes($IconOutput))).Replace("-", "").ToLowerInvariant()
    }
    finally { $sha256.Dispose() }
    if ($actualHash -ne $ExpectedIconSha256) {
        Remove-Item $IconOutput -Force -ErrorAction SilentlyContinue
        throw "Release icon SHA-256 mismatch. Expected $ExpectedIconSha256, got $actualHash."
    }

    Write-Host "Materialized release icon: $IconOutput" -ForegroundColor Green
}

if ($env:OS -ne "Windows_NT") {
    throw "Release preparation is Windows-first and must be run on Windows."
}

Materialize-ReleaseIcon

if (-not $AssetsOnly) {
    Require-Command "cargo" | Out-Null
    Require-Command "pnpm" | Out-Null

    Write-Host "Generating Cargo.lock..." -ForegroundColor Cyan
    & cargo generate-lockfile
    if ($LASTEXITCODE -ne 0) {
        throw "cargo generate-lockfile failed with exit code $LASTEXITCODE"
    }

    Write-Host "Generating pnpm-lock.yaml without lifecycle scripts..." -ForegroundColor Cyan
    & pnpm install --lockfile-only --ignore-scripts
    if ($LASTEXITCODE -ne 0) {
        throw "pnpm install --lockfile-only --ignore-scripts failed with exit code $LASTEXITCODE"
    }

    Write-Host ""
    Write-Host "Lockfiles were generated locally. Review and commit Cargo.lock + pnpm-lock.yaml before building a release candidate." -ForegroundColor Yellow
}

Write-Host "Release preparation complete." -ForegroundColor Green
