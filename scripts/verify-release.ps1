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

    $Results.Add([pscustomobject]@{
        id = $Id
        level = $Level
        detail = $Detail
        path = $Path
    })
}

function Read-JsonFile {
    param([string]$Id, [string]$Path, [bool]$Required = $true)

    if (-not (Test-Path $Path -PathType Leaf)) {
        if ($Required) {
            Add-Result $Id "blocking" "Required JSON file is missing." $Path
        }
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

function Get-Property {
    param([object]$Object, [string]$Name)

    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Require-File {
    param([string]$Id, [string]$Path, [string]$Detail)

    if (Test-Path $Path -PathType Leaf) {
        Add-Result $Id "ready" $Detail $Path
        return $true
    }

    Add-Result $Id "blocking" "Required release file is missing." $Path
    return $false
}

function Find-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $kitsRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (-not (Test-Path $kitsRoot)) {
        return $null
    }

    return Get-ChildItem $kitsRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\signtool.exe" } |
        Where-Object { Test-Path $_ -PathType Leaf } |
        Select-Object -First 1
}

$TauriConfigPath = Join-Path $TauriDir "tauri.conf.json"
$WindowsConfigPath = Join-Path $TauriDir "tauri.windows.conf.json"
$SignedWindowsConfigPath = Join-Path $TauriDir "tauri.windows.signed.conf.json"
$RootPackagePath = Join-Path $RepoRoot "package.json"
$DesktopPackagePath = Join-Path $DesktopDir "package.json"
$WorkspaceCargoPath = Join-Path $RepoRoot "Cargo.toml"
$DesktopCargoPath = Join-Path $TauriDir "Cargo.toml"
$CargoLockPath = Join-Path $RepoRoot "Cargo.lock"
$RustToolchainPath = Join-Path $RepoRoot "rust-toolchain.toml"
$LicensePath = Join-Path $RepoRoot "LICENSE"
$PnpmLockPath = Join-Path $RepoRoot "pnpm-lock.yaml"
$PrepareReleasePath = Join-Path $RepoRoot "scripts\prepare-release.ps1"
$SidecarStagePath = Join-Path $DesktopDir "scripts\stage-sidecar.mjs"
$SignScriptPath = Join-Path $TauriDir "scripts\sign-windows.ps1"
$IconSvgPath = Join-Path $TauriDir "icons\app-icon.svg"
$IconPayloadPath = Join-Path $TauriDir "icons\icon.ico.b64"
$IconPath = Join-Path $TauriDir "icons\icon.ico"

if ($env:OS -eq "Windows_NT") {
    Add-Result "platform" "ready" "Windows host detected."
}
else {
    Add-Result "platform" "blocking" "Release packaging is Windows-first and must be verified on Windows."
}

$tauri = Read-JsonFile "tauri_config" $TauriConfigPath
$windows = Read-JsonFile "tauri_windows_config" $WindowsConfigPath
$signedWindows = Read-JsonFile "tauri_signed_windows_config" $SignedWindowsConfigPath $PublicRelease
$rootPackage = Read-JsonFile "root_package" $RootPackagePath
$desktopPackage = Read-JsonFile "desktop_package" $DesktopPackagePath

Require-File "workspace_cargo" $WorkspaceCargoPath "Workspace Cargo manifest exists." | Out-Null
Require-File "desktop_cargo" $DesktopCargoPath "Desktop Cargo manifest exists." | Out-Null
Require-File "license" $LicensePath "Repository license file exists." | Out-Null
Require-File "release_prepare" $PrepareReleasePath "Deterministic release preparation script exists." | Out-Null
Require-File "sidecar_stage" $SidecarStagePath "Release sidecar staging script exists." | Out-Null
Require-File "sign_script" $SignScriptPath "Environment-driven Windows signing script exists." | Out-Null
Require-File "icon_svg" $IconSvgPath "Human-editable application icon source exists." | Out-Null
Require-File "icon_payload" $IconPayloadPath "Tracked reproducible ICO payload exists." | Out-Null

if (Test-Path $RustToolchainPath -PathType Leaf) {
    $toolchainText = Get-Content $RustToolchainPath -Raw
    if ($toolchainText -match 'channel\s*=\s*"1\.85\.0"') {
        Add-Result "rust_toolchain" "ready" "Rust toolchain is pinned to 1.85.0, matching the workspace release baseline." $RustToolchainPath
    }
    else {
        Add-Result "rust_toolchain" "blocking" "rust-toolchain.toml must pin channel 1.85.0 for the current release baseline." $RustToolchainPath
    }
}
else {
    Add-Result "rust_toolchain" "blocking" "rust-toolchain.toml is missing." $RustToolchainPath
}

foreach ($lock in @(
    @{ id = "cargo_lock"; path = $CargoLockPath; detail = "Cargo.lock is committed for reproducible Rust dependency resolution." },
    @{ id = "pnpm_lock"; path = $PnpmLockPath; detail = "pnpm-lock.yaml is committed for reproducible frontend dependency resolution." }
)) {
    if (Test-Path $lock.path -PathType Leaf) {
        Add-Result $lock.id "ready" $lock.detail $lock.path
    }
    else {
        Add-Result $lock.id "blocking" "Lockfile is missing. Run `pnpm desktop:release:prepare`, review generated lockfiles, then commit them." $lock.path
    }
}

if (Test-Path $IconPath -PathType Leaf) {
    $iconHash = (Get-FileHash -Algorithm SHA256 $IconPath).Hash.ToLowerInvariant()
    if ($iconHash -eq $ExpectedIconSha256) {
        Add-Result "windows_icon" "ready" "Materialized Windows icon matches the tracked release payload." $IconPath
    }
    else {
        Add-Result "windows_icon" "blocking" "Materialized icon hash does not match the tracked payload. Run `pnpm desktop:assets:prepare`." $IconPath
    }
}
else {
    Add-Result "windows_icon" "blocking" "Materialized release icon is missing. Run `pnpm desktop:assets:prepare`." $IconPath
}

if ($tauri) {
    $identifier = Get-Property $tauri "identifier"
    $tauriBundle = Get-Property $tauri "bundle"
    $externalBin = @(Get-Property $tauriBundle "externalBin")
    $version = [string](Get-Property $tauri "version")

    if ($identifier -eq "com.voduong.assisstantdesktop") {
        Add-Result "bundle_identifier" "ready" "Stable bundle identifier is configured."
    }
    else {
        Add-Result "bundle_identifier" "blocking" "Unexpected or missing bundle identifier: $identifier"
    }

    if ($externalBin -contains "binaries/assistant-mcp") {
        Add-Result "external_sidecar" "ready" "assistant-mcp is declared as a Tauri external sidecar."
    }
    else {
        Add-Result "external_sidecar" "blocking" "Tauri bundle must declare binaries/assistant-mcp as externalBin."
    }

    if ($version -match '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') {
        Add-Result "app_version" "ready" "Tauri app version is semantic: $version"
    }
    else {
        Add-Result "app_version" "blocking" "Tauri app version is not a release semantic version: $version"
    }

    $createUpdaterArtifacts = Get-Property $tauriBundle "createUpdaterArtifacts"
    if ($createUpdaterArtifacts -eq $true) {
        Add-Result "updater_artifacts" "optional" "Updater artifacts are enabled; verify updater signing key and endpoint policy before publishing."
    }
    else {
        Add-Result "updater_artifacts" "ready" "Automatic updater artifacts are disabled pending a trusted updater policy."
    }
}

if ($windows) {
    $windowsBuild = Get-Property $windows "build"
    $windowsBundle = Get-Property $windows "bundle"
    $nativeWindows = Get-Property $windowsBundle "windows"
    $nsis = Get-Property $nativeWindows "nsis"
    $features = @(Get-Property $windowsBuild "features")
    $targets = @(Get-Property $windowsBundle "targets")
    $icons = @(Get-Property $windowsBundle "icon")
    $installMode = Get-Property $nsis "installMode"
    $publisher = [string](Get-Property $windowsBundle "publisher")

    foreach ($requiredFeature in @("voice-whisper", "wake-word")) {
        if ($features -contains $requiredFeature) {
            Add-Result "feature_$requiredFeature" "ready" "Windows Tauri builds enable Cargo feature $requiredFeature."
        }
        else {
            Add-Result "feature_$requiredFeature" "blocking" "Windows release must enable Cargo feature $requiredFeature."
        }
    }

    if ($targets.Count -eq 1 -and $targets[0] -eq "nsis") {
        Add-Result "windows_bundle_target" "ready" "Windows release target is locked to NSIS."
    }
    else {
        Add-Result "windows_bundle_target" "blocking" "Expected exactly one Windows bundle target: nsis. Actual: $($targets -join ', ')"
    }

    if ($icons -contains "icons/icon.ico") {
        Add-Result "windows_bundle_icon" "ready" "Windows bundle is bound to the reproducible ICO asset."
    }
    else {
        Add-Result "windows_bundle_icon" "blocking" "Windows bundle must explicitly use icons/icon.ico."
    }

    if ($installMode -eq "currentUser") {
        Add-Result "nsis_install_mode" "ready" "NSIS installer uses current-user installation without mandatory elevation."
    }
    else {
        Add-Result "nsis_install_mode" "blocking" "NSIS installMode must be currentUser unless the elevation/security model is reviewed."
    }

    if ([string]::IsNullOrWhiteSpace($publisher)) {
        Add-Result "publisher" "blocking" "Windows bundle publisher is missing."
    }
    else {
        Add-Result "publisher" "ready" "Windows bundle publisher is configured: $publisher"
    }
}

if ($signedWindows) {
    $signedBundle = Get-Property $signedWindows "bundle"
    $signedNativeWindows = Get-Property $signedBundle "windows"
    $signCommand = [string](Get-Property $signedNativeWindows "signCommand")
    if ($signCommand -match 'scripts/sign-windows\.ps1' -and $signCommand -match '%1') {
        Add-Result "signed_overlay" "ready" "Signed release overlay delegates every Tauri signing target to the reviewed local signing script."
    }
    else {
        Add-Result "signed_overlay" "blocking" "Signed release overlay must call scripts/sign-windows.ps1 and preserve the Tauri %1 file placeholder."
    }
}
elseif ($PublicRelease) {
    Add-Result "signed_overlay" "blocking" "Public release requires tauri.windows.signed.conf.json."
}
else {
    Add-Result "signed_overlay" "info" "Signed overlay is not required for local unsigned packaging."
}

if ($PublicRelease) {
    $thumbprint = ([string]$env:ASSISTANT_WINDOWS_CERT_SHA1) -replace '\s', ''
    $timestampUrl = [string]$env:ASSISTANT_WINDOWS_TIMESTAMP_URL

    if ($thumbprint -match '^[0-9A-Fa-f]{40}$') {
        Add-Result "signing_thumbprint" "ready" "Signing certificate thumbprint environment variable has the expected SHA-1 shape."
    }
    else {
        Add-Result "signing_thumbprint" "blocking" "Set ASSISTANT_WINDOWS_CERT_SHA1 to the 40-character thumbprint of the installed code-signing certificate."
    }

    $parsedTimestamp = $null
    if (-not [string]::IsNullOrWhiteSpace($timestampUrl)) {
        try { $parsedTimestamp = [Uri]$timestampUrl } catch { $parsedTimestamp = $null }
    }
    if ($parsedTimestamp -and $parsedTimestamp.IsAbsoluteUri -and $parsedTimestamp.Scheme -in @("http", "https")) {
        Add-Result "timestamp_url" "ready" "Timestamp URL is configured: $timestampUrl"
    }
    else {
        Add-Result "timestamp_url" "blocking" "Set ASSISTANT_WINDOWS_TIMESTAMP_URL to the HTTP(S) RFC3161 timestamp service required by your certificate provider."
    }

    if ($thumbprint -match '^[0-9A-Fa-f]{40}$' -and $env:OS -eq "Windows_NT") {
        try {
            $certificate = Get-ChildItem Cert:\CurrentUser\My |
                Where-Object { ($_.Thumbprint -replace '\s', '') -ieq $thumbprint } |
                Select-Object -First 1
            if ($certificate -and $certificate.HasPrivateKey) {
                if ($certificate.NotAfter -le (Get-Date)) {
                    Add-Result "signing_certificate" "blocking" "Matching certificate is expired: $($certificate.NotAfter.ToString('u'))"
                }
                else {
                    Add-Result "signing_certificate" "ready" "Matching CurrentUser certificate with private key is available; expires $($certificate.NotAfter.ToString('u'))."
                }
            }
            elseif ($certificate) {
                Add-Result "signing_certificate" "blocking" "Matching certificate exists but no private key is available."
            }
            else {
                Add-Result "signing_certificate" "blocking" "No matching certificate was found in Cert:\CurrentUser\My."
            }
        }
        catch {
            Add-Result "signing_certificate" "blocking" "Could not inspect CurrentUser certificate store: $($_.Exception.Message)"
        }
    }

    $signTool = Find-SignTool
    if ($signTool) {
        Add-Result "signtool" "ready" "Windows SignTool is available." $signTool
    }
    else {
        Add-Result "signtool" "blocking" "signtool.exe was not found. Install a Windows SDK / Visual Studio Build Tools component that includes SignTool."
    }
}

if ($rootPackage) {
    $scripts = Get-Property $rootPackage "scripts"
    foreach ($scriptName in @(
        "desktop:assets:prepare",
        "desktop:dev",
        "desktop:build",
        "desktop:release:prepare",
        "desktop:release:verify",
        "desktop:release:verify:public",
        "desktop:release:build",
        "desktop:release:build:public"
    )) {
        $scriptValue = [string](Get-Property $scripts $scriptName)
        if ([string]::IsNullOrWhiteSpace($scriptValue)) {
            Add-Result "script_$scriptName" "blocking" "Root package script is missing: $scriptName"
        }
        else {
            Add-Result "script_$scriptName" "ready" "Root package script exists: $scriptName"
        }
    }
}

if ($desktopPackage -and $tauri) {
    $desktopVersion = [string](Get-Property $desktopPackage "version")
    $tauriVersion = [string](Get-Property $tauri "version")
    if ($desktopVersion -eq $tauriVersion -and -not [string]::IsNullOrWhiteSpace($desktopVersion)) {
        Add-Result "frontend_version_match" "ready" "Desktop package version matches Tauri version: $tauriVersion"
    }
    else {
        Add-Result "frontend_version_match" "blocking" "Frontend version ($desktopVersion) does not match Tauri version ($tauriVersion)."
    }
}

$git = Get-Command git -ErrorAction SilentlyContinue
if ($git) {
    try {
        $status = (& git -C $RepoRoot status --porcelain 2>&1 | Out-String).Trim()
        if ([string]::IsNullOrWhiteSpace($status)) {
            Add-Result "git_clean" "ready" "Working tree is clean."
        }
        else {
            Add-Result "git_clean" "blocking" "Working tree has uncommitted changes; release from a reviewed commit only."
        }

        $branch = (& git -C $RepoRoot branch --show-current 2>&1 | Out-String).Trim()
        Add-Result "git_branch" "info" "Current branch: $branch"
    }
    catch {
        Add-Result "git_status" "info" "Could not inspect Git status: $($_.Exception.Message)"
    }
}
else {
    Add-Result "git" "info" "git command is not available; clean-tree verification was skipped."
}

$Blocking = @($Results | Where-Object level -eq "blocking").Count
$Optional = @($Results | Where-Object level -eq "optional").Count
$Ready = @($Results | Where-Object level -eq "ready").Count

if ($Json) {
    [pscustomobject]@{
        summary = [pscustomobject]@{
            ready = $Ready
            optional = $Optional
            blocking = $Blocking
            public_release = [bool]$PublicRelease
        }
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
        $color = switch ($result.level) {
            "ready" { "Green" }
            "optional" { "Yellow" }
            "blocking" { "Red" }
            default { "DarkGray" }
        }
        Write-Host ("[{0,-8}] {1}: {2}" -f $result.level.ToUpperInvariant(), $result.id, $result.detail) -ForegroundColor $color
        if ($result.path) {
            Write-Host "           $($result.path)" -ForegroundColor DarkGray
        }
    }

    Write-Host ""
    Write-Host "Summary: $Ready ready / $Optional optional / $Blocking blocking"
    Write-Host ""
    Write-Host "This verifier never builds, signs, installs, downloads models, or invokes GitHub Actions." -ForegroundColor DarkGray
}

if ($Blocking -gt 0) { exit 1 }
exit 0
