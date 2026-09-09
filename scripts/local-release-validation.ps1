param(
    [switch]$PublicRelease,
    [switch]$Build
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$VerifyScript = Join-Path $PSScriptRoot "verify-release.ps1"
$ReleaseGuide = Join-Path $RepoRoot "docs\RELEASE_READINESS.md"

function Require-Command {
    param([string]$Name)
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Required command is not available in PATH: $Name"
    }
    return $command
}

function Write-Section {
    param([string]$Title)
    Write-Host ""
    Write-Host "== $Title ==" -ForegroundColor Cyan
}

if ($env:OS -ne "Windows_NT") {
    throw "Local release validation is Windows-first and must be run on Windows."
}

Require-Command "git" | Out-Null
Require-Command "node" | Out-Null
Require-Command "pnpm" | Out-Null
Require-Command "cargo" | Out-Null

if (-not (Test-Path $VerifyScript -PathType Leaf)) {
    throw "Release verifier is missing: $VerifyScript"
}

Write-Section "Repository"
$branch = (& git -C $RepoRoot branch --show-current 2>&1 | Out-String).Trim()
$status = (& git -C $RepoRoot status --porcelain 2>&1 | Out-String).Trim()
Write-Host "Branch: $branch"
Write-Host "Working tree: $(if ([string]::IsNullOrWhiteSpace($status)) { 'clean' } else { 'has local changes' })"

if ($branch -ne "main") {
    Write-Warning "Release acceptance should normally be performed from main. Current branch: $branch"
}

Write-Section "Static release preflight"
$verifyArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $VerifyScript, "-Json")
if ($PublicRelease) { $verifyArgs += "-PublicRelease" }

$verifyOutput = & powershell @verifyArgs 2>&1 | Out-String
$verifyExit = $LASTEXITCODE

try {
    $report = $verifyOutput | ConvertFrom-Json
}
catch {
    Write-Host $verifyOutput
    throw "Release verifier did not return valid JSON."
}

foreach ($item in $report.results) {
    $color = switch ($item.level) {
        "ready" { "Green" }
        "optional" { "Yellow" }
        "blocking" { "Red" }
        default { "DarkGray" }
    }
    Write-Host ("[{0,-8}] {1}: {2}" -f $item.level.ToUpperInvariant(), $item.id, $item.detail) -ForegroundColor $color
    if ($item.path) { Write-Host "           $($item.path)" -ForegroundColor DarkGray }
}

Write-Host ""
Write-Host ("Summary: {0} ready / {1} optional / {2} blocking" -f $report.summary.ready, $report.summary.optional, $report.summary.blocking)

if ($verifyExit -ne 0 -or [int]$report.summary.blocking -gt 0) {
    Write-Host ""
    Write-Host "Preflight is BLOCKED. Fix the blocking items before building a release candidate." -ForegroundColor Red
    Write-Host "Guide: $ReleaseGuide" -ForegroundColor DarkGray
    exit 1
}

Write-Section "Next local acceptance steps"
Write-Host "1. Start the desktop app locally:" -ForegroundColor White
Write-Host "   pnpm desktop:dev" -ForegroundColor Gray
Write-Host "2. Open Quick -> Control -> System and clear every runtime Blocking item." -ForegroundColor White
Write-Host "3. Open Quick -> Release and execute the 58 real-device acceptance checks." -ForegroundColor White
Write-Host "4. Validate Android QR/STT/Keystore, DPAPI restart, SAPI, Tailscale mobile-data, wake/mic and NSIS behavior on the target devices." -ForegroundColor White
Write-Host "5. The local gate is eligible for Ready only when runtime Blocking = 0 and manual required passed = 58/58." -ForegroundColor White
Write-Host "Guide: $ReleaseGuide" -ForegroundColor DarkGray

if (-not $Build) {
    Write-Host ""
    Write-Host "No build was started. When local acceptance is ready, run:" -ForegroundColor Yellow
    if ($PublicRelease) {
        Write-Host "   pnpm desktop:release:build:public" -ForegroundColor Gray
    }
    else {
        Write-Host "   pnpm desktop:release:build" -ForegroundColor Gray
    }
    exit 0
}

Write-Section "Release candidate build"
if ($PublicRelease) {
    Write-Host "Starting explicitly requested signed/public release build..." -ForegroundColor Yellow
    & pnpm desktop:release:build:public
}
else {
    Write-Host "Starting explicitly requested local unsigned release build..." -ForegroundColor Yellow
    & pnpm desktop:release:build
}

if ($LASTEXITCODE -ne 0) {
    throw "Release build failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Build command completed. Continue with installer/startup validation on the target Windows installation." -ForegroundColor Green
