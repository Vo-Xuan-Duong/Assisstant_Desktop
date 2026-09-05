param(
    [switch]$Json,
    [switch]$PublicRelease
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DesktopDir = Join-Path $RepoRoot "apps\desktop"
$TauriDir = Join-Path $DesktopDir "src-tauri"
$Results = [System.Collections.Generic.List[object]]::new()
$ExpectedIconSha256 = "ea40a92ac075efbe3628dcc5777368408b516a93228a04f5cc3e998f9869d7d5"

function Add-Result {
    param(
        [string]$Id,
        [ValidateSet("ready", "optional", "blocking", "info")]
        [string]$Level,
        [string]$Detail,
        [string]$Path = ""
    )
    $Results.Add([pscustomobject]@{ id = $Id; level = $Level; detail = $Detail; path = $Path })
}

function Get-Property {
    param([object]$Object, [string]$Name)
    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Read-JsonFile {
    param([string]$Id, [string]$Path, [bool]$Required = $true)
    if (-not (Test-Path $Path -PathType Leaf)) {
        if ($Required) { Add-Result $Id "blocking" "Required JSON file is missing." $Path }
        return $null
    }
    try {
        $value = Get-Content $Path -Raw | ConvertFrom-Json
        Add-Result $Id "ready" "JSON parses successfully." $Path
        return $value
    }
    catch {
        Add-Result $Id "blocking" "JSON parse failed: $($_.Exception.Message)" $Path
        return $null
    }
}

function Require-File {
    param([string]$Id, [string]$Path, [string]$Detail)
    if (Test-Path $Path -PathType Leaf) {
        Add-Result $Id "ready" $Detail $Path
    }
    else {
        Add-Result $Id "blocking" "Required release file is missing." $Path
    }
}

function Find-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ([string]::IsNullOrWhiteSpace([string]$programFilesX86)) { return $null }

    $kitsRoot = Join-Path $programFilesX86 "Windows Kits\10\bin"
    if (-not (Test-Path $kitsRoot)) { return $null }

    return Get-ChildItem $kitsRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\signtool.exe" } |
        Where-Object { Test-Path $_ -PathType Leaf } |
        Select-Object -First 1
}

$Paths = @{
    tauri = Join-Path $TauriDir "tauri.conf.json"
    windows = Join-Path $TauriDir "tauri.windows.conf.json"
    signed = Join-Path $TauriDir "tauri.windows.signed.conf.json"
    rootPackage = Join-Path $RepoRoot "package.json"
    desktopPackage = Join-Path $DesktopDir "package.json"
    workspaceCargo = Join-Path $RepoRoot "Cargo.toml"
    desktopCargo = Join-Path $TauriDir "Cargo.toml"
    cargoLock = Join-Path $RepoRoot "Cargo.lock"
    pnpmLock = Join-Path $RepoRoot "pnpm-lock.yaml"
    rustToolchain = Join-Path $RepoRoot "rust-toolchain.toml"
    license = Join-Path $RepoRoot "LICENSE"
    prepare = Join-Path $RepoRoot "scripts\prepare-release.ps1"
    sidecarStage = Join-Path $DesktopDir "scripts\stage-sidecar.mjs"
    signScript = Join-Path $TauriDir "scripts\sign-windows.ps1"
    iconSvg = Join-Path $TauriDir "icons\app-icon.svg"
    iconPayload = Join-Path $TauriDir "icons\icon.ico.b64"
    icon = Join-Path $TauriDir "icons\icon.ico"
}

$IsWindowsHost = $env:OS -eq "Windows_NT"
Add-Result "platform" $(if ($IsWindowsHost) { "ready" } else { "blocking" }) $(if ($IsWindowsHost) { "Windows host detected." } else { "Release packaging must be verified on Windows." })

$tauri = Read-JsonFile "tauri_config" $Paths.tauri
$windows = Read-JsonFile "tauri_windows_config" $Paths.windows
$signed = Read-JsonFile "tauri_signed_windows_config" $Paths.signed $PublicRelease
$rootPackage = Read-JsonFile "root_package" $Paths.rootPackage
$desktopPackage = Read-JsonFile "desktop_package" $Paths.desktopPackage

Require-File "workspace_cargo" $Paths.workspaceCargo "Workspace Cargo manifest exists."
Require-File "desktop_cargo" $Paths.desktopCargo "Desktop Cargo manifest exists."
Require-File "license" $Paths.license "MIT license file exists."
Require-File "release_prepare" $Paths.prepare "Release preparation script exists."
Require-File "sidecar_stage" $Paths.sidecarStage "Sidecar staging script exists."
Require-File "sign_script" $Paths.signScript "Windows signing script exists."
Require-File "icon_svg" $Paths.iconSvg "Human-editable icon source exists."
Require-File "icon_payload" $Paths.iconPayload "Reproducible ICO payload exists."

if (Test-Path $Paths.rustToolchain -PathType Leaf) {
    $toolchain = Get-Content $Paths.rustToolchain -Raw
    Add-Result "rust_toolchain" $(if ($toolchain -match 'channel\s*=\s*"1\.98\.1"') { "ready" } else { "blocking" }) $(if ($toolchain -match 'channel\s*=\s*"1\.98\.1"') { "Rust release toolchain is pinned to 1.98.1." } else { "rust-toolchain.toml must pin 1.98.1." }) $Paths.rustToolchain
}
else {
    Add-Result "rust_toolchain" "blocking" "rust-toolchain.toml is missing." $Paths.rustToolchain
}

foreach ($lock in @(
    @{ id = "cargo_lock"; path = $Paths.cargoLock },
    @{ id = "pnpm_lock"; path = $Paths.pnpmLock }
)) {
    if (Test-Path $lock.path -PathType Leaf) {
        Add-Result $lock.id "ready" "Dependency lockfile exists." $lock.path
    }
    else {
        Add-Result $lock.id "blocking" "Lockfile is missing. Run `pnpm desktop:release:prepare`, review the resolver output, then commit it." $lock.path
    }
}

if (Test-Path $Paths.icon -PathType Leaf) {
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        $iconHash = [BitConverter]::ToString($sha256.ComputeHash([IO.File]::ReadAllBytes($Paths.icon))).Replace("-", "").ToLowerInvariant()
    }
    finally { $sha256.Dispose() }
    Add-Result "windows_icon" $(if ($iconHash -eq $ExpectedIconSha256) { "ready" } else { "blocking" }) $(if ($iconHash -eq $ExpectedIconSha256) { "Materialized icon matches the tracked payload." } else { "Materialized icon hash is wrong; run `pnpm desktop:assets:prepare`." }) $Paths.icon
}
else {
    Add-Result "windows_icon" "blocking" "Materialized icon is missing; run `pnpm desktop:assets:prepare`." $Paths.icon
}

if ($tauri) {
    $bundle = Get-Property $tauri "bundle"
    $identifier = [string](Get-Property $tauri "identifier")
    $version = [string](Get-Property $tauri "version")
    $externalBin = @(Get-Property $bundle "externalBin")

    Add-Result "bundle_identifier" $(if ($identifier -eq "com.voduong.assisstantdesktop") { "ready" } else { "blocking" }) "Bundle identifier: $identifier"
    Add-Result "external_sidecar" $(if ($externalBin -contains "binaries/assistant-mcp") { "ready" } else { "blocking" }) $(if ($externalBin -contains "binaries/assistant-mcp") { "assistant-mcp external sidecar is declared." } else { "binaries/assistant-mcp is missing from externalBin." })
    Add-Result "app_version" $(if ($version -match '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') { "ready" } else { "blocking" }) "Tauri version: $version"

    $updaterArtifacts = Get-Property $bundle "createUpdaterArtifacts"
    Add-Result "updater_artifacts" $(if ($updaterArtifacts -eq $true) { "optional" } else { "ready" }) $(if ($updaterArtifacts -eq $true) { "Updater artifacts are enabled and require a complete updater trust policy." } else { "Updater artifacts remain disabled." })
}

if ($windows) {
    $build = Get-Property $windows "build"
    $bundle = Get-Property $windows "bundle"
    $native = Get-Property $bundle "windows"
    $nsis = Get-Property $native "nsis"
    $features = @(Get-Property $build "features")
    $targets = @(Get-Property $bundle "targets")
    $icons = @(Get-Property $bundle "icon")
    $publisher = [string](Get-Property $bundle "publisher")
    $installMode = [string](Get-Property $nsis "installMode")

    foreach ($feature in @("voice-whisper", "wake-word")) {
        Add-Result "feature_$feature" $(if ($features -contains $feature) { "ready" } else { "blocking" }) $(if ($features -contains $feature) { "Windows build enables $feature." } else { "Windows release must enable $feature." })
    }
    Add-Result "windows_bundle_target" $(if ($targets.Count -eq 1 -and $targets[0] -eq "nsis") { "ready" } else { "blocking" }) "Windows bundle target(s): $($targets -join ', ')"
    Add-Result "windows_bundle_icon" $(if ($icons -contains "icons/icon.ico") { "ready" } else { "blocking" }) $(if ($icons -contains "icons/icon.ico") { "Windows bundle uses icons/icon.ico." } else { "Windows bundle must use icons/icon.ico." })
    Add-Result "nsis_install_mode" $(if ($installMode -eq "currentUser") { "ready" } else { "blocking" }) "NSIS installMode: $installMode"
    Add-Result "publisher" $(if ([string]::IsNullOrWhiteSpace($publisher)) { "blocking" } else { "ready" }) "Publisher: $publisher"
}

if ($signed) {
    $signedBundle = Get-Property $signed "bundle"
    $signedNative = Get-Property $signedBundle "windows"
    $signCommand = [string](Get-Property $signedNative "signCommand")
    $validSignCommand = $signCommand -match 'scripts/sign-windows\.ps1' -and $signCommand -match '%1'
    Add-Result "signed_overlay" $(if ($validSignCommand) { "ready" } else { "blocking" }) $(if ($validSignCommand) { "Signed overlay calls the reviewed signing script with the Tauri %1 placeholder." } else { "Signed overlay must call scripts/sign-windows.ps1 and preserve %1." }) $Paths.signed
}
elseif (-not $PublicRelease) {
    Add-Result "signed_overlay" "info" "Signed overlay is optional for local unsigned packaging."
}

if ($PublicRelease) {
    $thumbprint = ([string]$env:ASSISTANT_WINDOWS_CERT_SHA1) -replace '\s', ''
    $timestampUrl = [string]$env:ASSISTANT_WINDOWS_TIMESTAMP_URL
    $thumbprintValid = $thumbprint -match '^[0-9A-Fa-f]{40}$'
    Add-Result "signing_thumbprint" $(if ($thumbprintValid) { "ready" } else { "blocking" }) $(if ($thumbprintValid) { "Signing thumbprint has valid SHA-1 syntax." } else { "Set ASSISTANT_WINDOWS_CERT_SHA1 to a 40-character certificate thumbprint." })

    $timestampValid = $false
    if (-not [string]::IsNullOrWhiteSpace($timestampUrl)) {
        try {
            $uri = [Uri]$timestampUrl
            $timestampValid = $uri.IsAbsoluteUri -and $uri.Scheme -in @("http", "https")
        }
        catch { $timestampValid = $false }
    }
    Add-Result "timestamp_url" $(if ($timestampValid) { "ready" } else { "blocking" }) $(if ($timestampValid) { "Timestamp URL is configured: $timestampUrl" } else { "Set ASSISTANT_WINDOWS_TIMESTAMP_URL to an absolute HTTP(S) RFC3161 timestamp URL." })

    if ($IsWindowsHost -and $thumbprintValid) {
        try {
            $certificate = Get-ChildItem Cert:\CurrentUser\My |
                Where-Object { ($_.Thumbprint -replace '\s', '') -ieq $thumbprint } |
                Select-Object -First 1
            if (-not $certificate) {
                Add-Result "signing_certificate" "blocking" "No matching certificate exists in Cert:\CurrentUser\My."
            }
            elseif (-not $certificate.HasPrivateKey) {
                Add-Result "signing_certificate" "blocking" "Matching certificate has no private key."
            }
            elseif ($certificate.NotAfter -le (Get-Date)) {
                Add-Result "signing_certificate" "blocking" "Matching certificate is expired: $($certificate.NotAfter.ToString('u'))"
            }
            else {
                Add-Result "signing_certificate" "ready" "Matching certificate/private key is available; expires $($certificate.NotAfter.ToString('u'))."
            }
        }
        catch {
            Add-Result "signing_certificate" "blocking" "Could not inspect CurrentUser certificate store: $($_.Exception.Message)"
        }
    }
    elseif (-not $IsWindowsHost) {
        Add-Result "signing_certificate" "blocking" "Certificate-store verification requires Windows."
    }

    $signTool = Find-SignTool
    Add-Result "signtool" $(if ($signTool) { "ready" } else { "blocking" }) $(if ($signTool) { "Windows SignTool is available." } else { "signtool.exe is missing; install the Windows SDK / Visual Studio Build Tools." }) $(if ($signTool) { [string]$signTool } else { "" })
}

if ($rootPackage) {
    $scripts = Get-Property $rootPackage "scripts"
    foreach ($name in @(
        "desktop:assets:prepare",
        "desktop:dev",
        "desktop:build",
        "desktop:release:prepare",
        "desktop:release:verify",
        "desktop:release:verify:public",
        "desktop:release:build",
        "desktop:release:build:public"
    )) {
        $value = [string](Get-Property $scripts $name)
        Add-Result "script_$name" $(if ([string]::IsNullOrWhiteSpace($value)) { "blocking" } else { "ready" }) $(if ([string]::IsNullOrWhiteSpace($value)) { "Missing root script: $name" } else { "Root script exists: $name" })
    }
}

if ($desktopPackage -and $tauri) {
    $desktopVersion = [string](Get-Property $desktopPackage "version")
    $tauriVersion = [string](Get-Property $tauri "version")
    Add-Result "frontend_version_match" $(if ($desktopVersion -eq $tauriVersion -and -not [string]::IsNullOrWhiteSpace($desktopVersion)) { "ready" } else { "blocking" }) "Frontend $desktopVersion / Tauri $tauriVersion"
}

$git = Get-Command git -ErrorAction SilentlyContinue
if ($git) {
    try {
        $status = (& git -C $RepoRoot status --porcelain 2>&1 | Out-String).Trim()
        Add-Result "git_clean" $(if ([string]::IsNullOrWhiteSpace($status)) { "ready" } else { "blocking" }) $(if ([string]::IsNullOrWhiteSpace($status)) { "Working tree is clean." } else { "Working tree has uncommitted changes." })
        $branch = (& git -C $RepoRoot branch --show-current 2>&1 | Out-String).Trim()
        Add-Result "git_branch" "info" "Current branch: $branch"
    }
    catch {
        Add-Result "git_status" "info" "Could not inspect Git status: $($_.Exception.Message)"
    }
}
else {
    Add-Result "git" "info" "git command is unavailable; clean-tree verification was skipped."
}

$Blocking = @($Results | Where-Object level -eq "blocking").Count
$Optional = @($Results | Where-Object level -eq "optional").Count
$Ready = @($Results | Where-Object level -eq "ready").Count

if ($Json) {
    [pscustomobject]@{
        summary = [pscustomobject]@{ ready = $Ready; optional = $Optional; blocking = $Blocking; public_release = [bool]$PublicRelease }
        results = $Results
    } | ConvertTo-Json -Depth 8
}
else {
    Write-Host ""
    Write-Host "Assisstant Desktop - Release Readiness" -ForegroundColor Cyan
    Write-Host "Repo: $RepoRoot"
    Write-Host "Public release policy: $([bool]$PublicRelease)"
    Write-Host ""
    foreach ($result in $Results) {
        $color = switch ($result.level) { "ready" { "Green" }; "optional" { "Yellow" }; "blocking" { "Red" }; default { "DarkGray" } }
        Write-Host ("[{0,-8}] {1}: {2}" -f $result.level.ToUpperInvariant(), $result.id, $result.detail) -ForegroundColor $color
        if ($result.path) { Write-Host "           $($result.path)" -ForegroundColor DarkGray }
    }
    Write-Host ""
    Write-Host "Summary: $Ready ready / $Optional optional / $Blocking blocking"
    Write-Host "This verifier never builds, signs, installs, downloads models, or invokes GitHub Actions." -ForegroundColor DarkGray
}

if ($Blocking -gt 0) { exit 1 }
exit 0
