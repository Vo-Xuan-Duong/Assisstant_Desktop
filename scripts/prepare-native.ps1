$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# LLVM's libclang wheel supplies the standalone DLL; Python is not required.
# https://pypi.org/project/libclang/18.1.1/
$RepoRoot = Split-Path -Parent $PSScriptRoot
$NativeDir = Join-Path $RepoRoot "target\native\libclang"
$Library = Join-Path $NativeDir "libclang.dll"
if (Test-Path $Library -PathType Leaf) {
    Write-Host "libclang is ready: $Library"
    exit 0
}
if ($env:OS -ne "Windows_NT" -or $env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "Native dependency preparation currently supports Windows x64."
}

$Url = "https://files.pythonhosted.org/packages/0b/2d/3f480b1e1d31eb3d6de5e3ef641954e5c67430d5ac93b7fa7e07589576c7/libclang-18.1.1-py2.py3-none-win_amd64.whl"
$ExpectedHash = "4dd2d3b82fab35e2bf9ca717d7b63ac990a3519c7e312f19fa8e86dcc712f7fb"
New-Item -ItemType Directory -Force -Path $NativeDir | Out-Null
$Archive = Join-Path $NativeDir "libclang-18.1.1.whl"
if (-not (Test-Path $Archive -PathType Leaf)) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $client = New-Object Net.WebClient
    try { $client.DownloadFile($Url, $Archive) }
    finally { $client.Dispose() }
}
$sha256 = [Security.Cryptography.SHA256]::Create()
try {
    $hash = [BitConverter]::ToString($sha256.ComputeHash([IO.File]::ReadAllBytes($Archive))).Replace("-", "").ToLowerInvariant()
}
finally { $sha256.Dispose() }
if ($hash -ne $ExpectedHash) { throw "libclang archive SHA-256 mismatch: $Archive" }

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($Archive)
try {
    $entry = $zip.GetEntry("libclang-18.1.1.data/platlib/clang/native/libclang.dll")
    if ($null -eq $entry) { throw "Verified archive does not contain libclang.dll." }
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $Library, $true)
    [IO.Compression.ZipFileExtensions]::ExtractToFile($zip.GetEntry("libclang-18.1.1.dist-info/LICENSE.TXT"), (Join-Path $NativeDir "LICENSE.TXT"), $true)
}
finally { $zip.Dispose() }
Write-Host "Prepared verified libclang 18.1.1: $Library"
