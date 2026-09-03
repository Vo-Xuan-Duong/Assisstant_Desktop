param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Path
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($env:OS -ne "Windows_NT") {
    throw "Windows code signing is only supported on Windows."
}

if (-not (Test-Path $Path -PathType Leaf)) {
    throw "Signing target does not exist: $Path"
}

$thumbprint = ([string]$env:ASSISTANT_WINDOWS_CERT_SHA1) -replace '\s', ''
$timestampUrl = [string]$env:ASSISTANT_WINDOWS_TIMESTAMP_URL

if ([string]::IsNullOrWhiteSpace($thumbprint)) {
    throw "ASSISTANT_WINDOWS_CERT_SHA1 is required for public Windows signing."
}
if ($thumbprint -notmatch '^[0-9A-Fa-f]{40}$') {
    throw "ASSISTANT_WINDOWS_CERT_SHA1 must be a 40-character SHA-1 certificate thumbprint."
}
if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
    throw "ASSISTANT_WINDOWS_TIMESTAMP_URL is required for public Windows signing."
}

$certificate = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { ($_.Thumbprint -replace '\s', '') -ieq $thumbprint } |
    Select-Object -First 1

if (-not $certificate) {
    throw "No certificate matching ASSISTANT_WINDOWS_CERT_SHA1 was found in Cert:\CurrentUser\My."
}
if (-not $certificate.HasPrivateKey) {
    throw "The selected code-signing certificate does not expose a private key."
}

$signTool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signTool) {
    $kitsRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (Test-Path $kitsRoot) {
        $candidate = Get-ChildItem $kitsRoot -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            ForEach-Object { Join-Path $_.FullName "x64\signtool.exe" } |
            Where-Object { Test-Path $_ -PathType Leaf } |
            Select-Object -First 1
        if ($candidate) {
            $signTool = Get-Item $candidate
        }
    }
}

if (-not $signTool) {
    throw "signtool.exe was not found. Install the Windows SDK / Visual Studio Build Tools and ensure SignTool is available."
}

$signToolPath = if ($signTool.Source) { $signTool.Source } else { $signTool.FullName }

Write-Host "Signing $Path" -ForegroundColor Cyan
& $signToolPath sign /sha1 $thumbprint /s My /fd SHA256 /tr $timestampUrl /td SHA256 /v $Path
if ($LASTEXITCODE -ne 0) {
    throw "signtool sign failed with exit code $LASTEXITCODE"
}

& $signToolPath verify /pa /v $Path
if ($LASTEXITCODE -ne 0) {
    throw "signtool verify failed with exit code $LASTEXITCODE"
}

Write-Host "Signature verified: $Path" -ForegroundColor Green
