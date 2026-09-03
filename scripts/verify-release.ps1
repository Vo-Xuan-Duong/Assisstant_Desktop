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
    param([string]$Id, [string]$Path)

    if (-not (Test-Path $Path -PathType Leaf)) {
        Add-Result $Id "blocking" "Required JSON file is missing." $Path
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
        return
    }

    Add-Result $Id "blocking" "Required release file is missing." $Path
}

$TauriConfigPath = Join-Path $TauriDir "tauri.conf.json"
$WindowsConfigPath = Join-Path $TauriDir "tauri.windows.conf.json"
$RootPackagePath = Join-Path $RepoRoot "package.json"
$DesktopPackagePath = Join-Path $DesktopDir "package.json"
$WorkspaceCargoPath = Join-Path $RepoRoot "Cargo.toml"
$DesktopCargoPath = Join-Path $TauriDir "Cargo.toml"
$CargoLockPath = Join-Path $RepoRoot "Cargo.lock"
$RustToolchainPath = Join-Path $RepoRoot "rust-toolchain.toml"
$LicensePath = Join-Path $RepoRoot "LICENSE"
$PnpmLockPath = Join-Path $RepoRoot "pnpm-lock.yaml"
$SidecarStagePath = Join-Path $DesktopDir "scripts\stage-sidecar.mjs"
$IconPath = Join-Path $TauriDir "icons\icon.ico"

if ($env:OS -eq "Windows_NT") {
    Add-Result "platform" "ready" "Windows host detected."
}
else {
    Add-Result "platform" "blocking" "Release packaging is Windows-first and must be verified on Windows."
}

$tauri = Read-JsonFile "tauri_config" $TauriConfigPath
$windows = Read-JsonFile "tauri_windows_config" $WindowsConfigPath
$rootPackage = Read-JsonFile "root_package" $RootPackagePath
$desktopPackage = Read-JsonFile "desktop_package" $DesktopPackagePath

Require-File "workspace_cargo" $WorkspaceCargoPath "Workspace Cargo manifest exists."
Require-File "desktop_cargo" $DesktopCargoPath "Desktop Cargo manifest exists."
Require-File "license" $LicensePath "Repository license file exists."
Require-File "sidecar_stage" $SidecarStagePath "Release sidecar staging script exists."

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
    @{ id = "cargo_lock"; path = $CargoLockPath; detail = "Cargo.lock is committed for reproducible Rust dependency resolution."; fix = "Run `cargo generate-lockfile` locally, review it, and commit it before release." },
    @{ id = "pnpm_lock"; path = $PnpmLockPath; detail = "pnpm-lock.yaml is committed for reproducible frontend dependency resolution."; fix = "Run `pnpm install --lockfile-only` locally, review it, and commit it before release." }
)) {
    if (Test-Path $lock.path -PathType Leaf) {
        Add-Result $lock.id "ready" $lock.detail $lock.path
    }
    else {
        Add-Result $lock.id "blocking" $lock.fix $lock.path
    }
}

if (Test-Path $IconPath -PathType Leaf) {
    Add-Result "windows_icon" "ready" "Windows release icon exists." $IconPath
}
else {
    Add-Result "windows_icon" "blocking" "Release icon is missing. Generate/approve a branded icon set and commit icons/icon.ico before release." $IconPath
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
    $installMode = Get-Property $nsis "installMode"
    $publisher = [string](Get-Property $windowsBundle "publisher")
    $signCommand = [string](Get-Property $nativeWindows "signCommand")

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

    $hasSignCommand = -not [string]::IsNullOrWhiteSpace($signCommand)
    if ($PublicRelease -and -not $hasSignCommand) {
        Add-Result "code_signing" "blocking" "Public Windows release requires a reviewed code-signing configuration."
    }
    elseif ($hasSignCommand) {
        Add-Result "code_signing" "ready" "A Windows signCommand is configured; verify the real signing identity before publishing."
    }
    else {
        Add-Result "code_signing" "info" "No Windows signCommand is committed. Local unsigned packaging is allowed; use -PublicRelease to enforce signing."
    }
}

if ($rootPackage) {
    $scripts = Get-Property $rootPackage "scripts"
    foreach ($scriptName in @("desktop:dev", "desktop:build", "desktop:release:verify", "desktop:release:build")) {
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
