param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("test", "test-models", "dev", "build", "build-public", "check")]
    [string]$Mode
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$RepoRoot = Split-Path -Parent $PSScriptRoot

# libclang does not discover newer Visual Studio installations automatically.
# Give bindgen the installed Windows C headers without pinning machine paths.
if ([string]::IsNullOrWhiteSpace($env:BINDGEN_EXTRA_CLANG_ARGS)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw "Install Visual Studio C++ Build Tools first." }
    $vsRoot = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
    if ([string]::IsNullOrWhiteSpace($vsRoot)) { throw "No Visual Studio C++ toolchain found." }
    $vcVersion = Get-Content (Join-Path $vsRoot "VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt") | Select-Object -First 1
    $vcInclude = Join-Path $vsRoot "VC\Tools\MSVC\$($vcVersion.Trim())\include"
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Include"
    $sdk = Get-ChildItem $sdkRoot -Directory | Where-Object { Test-Path (Join-Path $_.FullName "ucrt\stdio.h") } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if ($null -eq $sdk -or -not (Test-Path $vcInclude)) { throw "Windows SDK / MSVC headers are missing." }
    $ucrt = Join-Path $sdk.FullName "ucrt"
    $env:BINDGEN_EXTRA_CLANG_ARGS = '-isystem "' + $vcInclude + '" -isystem "' + $ucrt + '"'
}

Push-Location $RepoRoot
try {
    switch ($Mode) {
        "test" { & cargo test --workspace --locked --all-features --no-fail-fast }
        "test-models" {
            if ([string]::IsNullOrWhiteSpace($env:ASSISTANT_TEST_MODELS_DIR)) {
                $env:ASSISTANT_TEST_MODELS_DIR = Join-Path $env:LOCALAPPDATA "com.voduong.assisstantdesktop\models"
            }
            & cargo test --workspace --locked --all-features --test native_models -- --ignored
        }
        "check" { & cargo check -p assisstant-desktop --locked --all-features }
        "dev" { & pnpm --filter '@assisstant/desktop' tauri dev }
        "build" { & pnpm --filter '@assisstant/desktop' tauri build }
        "build-public" { & pnpm --filter '@assisstant/desktop' tauri build --config src-tauri/tauri.windows.signed.conf.json }
    }
    $result = $LASTEXITCODE
}
finally { Pop-Location }
exit $result
