param(
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DesktopDir = Join-Path $RepoRoot "apps\desktop"
$TauriDir = Join-Path $DesktopDir "src-tauri"
$BundleIdentifier = "com.voduong.assisstantdesktop"
$AppLocalData = Join-Path $env:LOCALAPPDATA $BundleIdentifier
$IsWindowsHost = $env:OS -eq "Windows_NT"

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

function Get-CommandInfo {
    param(
        [string]$Name,
        [string[]]$VersionArgs,
        [bool]$Required
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        Add-Result $Name ($(if ($Required) { "blocking" } else { "optional" })) "Không tìm thấy command trong PATH."
        return $null
    }

    $version = ""
    try {
        $version = (& $Name @VersionArgs 2>&1 | Select-Object -First 1 | Out-String).Trim()
    }
    catch {
        $version = "Command tồn tại nhưng không đọc được version: $($_.Exception.Message)"
    }

    Add-Result $Name "ready" $version $command.Source
    return $command
}

if (-not $IsWindowsHost) {
    Add-Result "platform" "blocking" "Verification harness này chỉ hỗ trợ Windows."
}
else {
    Add-Result "platform" "ready" "Windows host detected."
}

if (-not (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    Add-Result "repo_root" "blocking" "Không tìm thấy Cargo.toml tại repo root." $RepoRoot
}
else {
    Add-Result "repo_root" "ready" "Repository root hợp lệ." $RepoRoot
}

$rustc = Get-CommandInfo "rustc" @("--version") $true
$cargo = Get-CommandInfo "cargo" @("--version") $true
$pnpm = Get-CommandInfo "pnpm" @("--version") $true
$agy = Get-CommandInfo "agy" @("--version") $true

$TargetTriple = $null
if ($rustc) {
    try {
        $TargetTriple = (& rustc --print host-tuple 2>&1 | Out-String).Trim()
        if ($TargetTriple -match "windows-msvc$") {
            Add-Result "rust_target" "ready" $TargetTriple
        }
        else {
            Add-Result "rust_target" "blocking" "Project cần native Windows MSVC target; host hiện tại: $TargetTriple"
        }
    }
    catch {
        Add-Result "rust_target" "blocking" "Không đọc được Rust host target: $($_.Exception.Message)"
    }
}

$cl = Get-Command "cl.exe" -ErrorAction SilentlyContinue
if ($cl) {
    Add-Result "msvc_cl" "ready" "MSVC cl.exe có trong PATH." $cl.Source
}
else {
    Add-Result "msvc_cl" "info" "Không thấy cl.exe trong PATH. Cargo MSVC vẫn có thể hoạt động nếu Visual Studio Build Tools được Rust toolchain tìm thấy; nếu linker lỗi, mở Developer PowerShell hoặc cài C++ Build Tools."
}

$WebViewCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft\EdgeWebView\Application"),
    (Join-Path $env:LOCALAPPDATA "Microsoft\EdgeWebView\Application")
) | Where-Object { $_ -and (Test-Path $_) }

if ($WebViewCandidates.Count -gt 0) {
    Add-Result "webview2" "ready" "Tìm thấy Microsoft Edge WebView2 runtime directory." $WebViewCandidates[0]
}
else {
    Add-Result "webview2" "info" "Không xác nhận được WebView2 bằng filesystem probe. Tauri/Windows có thể vẫn cung cấp runtime qua vị trí khác."
}

$DebugSidecar = Join-Path $RepoRoot "target\debug\assistant-mcp.exe"
$ReleaseSidecar = Join-Path $RepoRoot "target\release\assistant-mcp.exe"
if (Test-Path $DebugSidecar) {
    Add-Result "mcp_debug_binary" "ready" "Debug MCP binary đã tồn tại." $DebugSidecar
}
else {
    Add-Result "mcp_debug_binary" "info" "Debug MCP binary chưa được build." $DebugSidecar
}

if (Test-Path $ReleaseSidecar) {
    Add-Result "mcp_release_binary" "ready" "Release MCP binary đã tồn tại." $ReleaseSidecar
}
else {
    Add-Result "mcp_release_binary" "info" "Release MCP binary chưa được build." $ReleaseSidecar
}

if ($TargetTriple) {
    $StagedSidecar = Join-Path $TauriDir "binaries\assistant-mcp-$TargetTriple.exe"
    if (Test-Path $StagedSidecar) {
        Add-Result "tauri_staged_sidecar" "ready" "Tauri target-triple sidecar đã được stage." $StagedSidecar
    }
    else {
        Add-Result "tauri_staged_sidecar" "info" "Sidecar chưa được stage. `pnpm --dir apps/desktop sidecar:stage:dev` sẽ tạo file khi bạn chủ động chạy." $StagedSidecar
    }
}

$RuntimeDir = Join-Path $AppLocalData "runtime"
$GeneratedMcpConfig = Join-Path $RuntimeDir ".agents\mcp_config.json"
$ContextDir = Join-Path $AppLocalData "context"
$WhisperModel = Join-Path $AppLocalData "models\whisper\ggml-base.bin"
$WakeModelDir = Join-Path $AppLocalData "models\wake\sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01"
$WakeBpe = Join-Path $WakeModelDir "bpe.model"
$WakeTokens = Join-Path $WakeModelDir "tokens.txt"
$WakeKeywords = Join-Path $WakeModelDir "keywords.txt"
$WakeSettings = Join-Path $AppLocalData "settings\wake.json"
$PermissionPolicy = Join-Path $AppLocalData "permissions\policy.json"

if (Test-Path $GeneratedMcpConfig) {
    Add-Result "runtime_mcp_config" "ready" "Generated MCP config đã tồn tại sau một lần desktop startup." $GeneratedMcpConfig
}
else {
    Add-Result "runtime_mcp_config" "info" "Generated MCP config chưa tồn tại; desktop sẽ tạo file khi startup." $GeneratedMcpConfig
}

if (Test-Path $ContextDir) {
    Add-Result "context_dir" "ready" "Context app-local-data directory đã tồn tại." $ContextDir
}
else {
    Add-Result "context_dir" "info" "Context directory sẽ được desktop tạo khi startup." $ContextDir
}

if (Test-Path $WhisperModel) {
    Add-Result "whisper_model" "ready" "Whisper model mặc định đã tồn tại." $WhisperModel
}
else {
    Add-Result "whisper_model" "optional" "Chưa có Whisper model mặc định; text assistant vẫn hoạt động." $WhisperModel
}

if (Test-Path $WakeModelDir) {
    Add-Result "wake_models" "ready" "Wake model directory đã tồn tại." $WakeModelDir
}
else {
    Add-Result "wake_models" "optional" "Wake resources chưa được cài; wake word là optional." $WakeModelDir
}

foreach ($wakeResource in @(
    @{ id = "wake_tokens"; path = $WakeTokens; label = "tokens.txt" },
    @{ id = "wake_keywords"; path = $WakeKeywords; label = "keywords.txt" },
    @{ id = "wake_bpe"; path = $WakeBpe; label = "bpe.model (preparation-only)" }
)) {
    if (Test-Path $wakeResource.path) {
        Add-Result $wakeResource.id "ready" "$($wakeResource.label) đã tồn tại." $wakeResource.path
    }
    else {
        Add-Result $wakeResource.id "optional" "$($wakeResource.label) chưa tồn tại; wake vẫn là capability optional." $wakeResource.path
    }
}

if (Test-Path $WakeSettings) {
    try {
        $wakeSettingsJson = Get-Content $WakeSettings -Raw | ConvertFrom-Json
        Add-Result "wake_settings" "ready" "Wake settings parse được (enabled/phrase)." $WakeSettings
    }
    catch {
        Add-Result "wake_settings" "optional" "Wake settings tồn tại nhưng JSON lỗi; desktop sẽ fallback an toàn và báo detail: $($_.Exception.Message)" $WakeSettings
    }
}
else {
    Add-Result "wake_settings" "info" "Wake settings chưa tồn tại; desktop sẽ dùng mặc định/env override cho tới khi user thay đổi wake configuration." $WakeSettings
}

if (Test-Path $PermissionPolicy) {
    try {
        Get-Content $PermissionPolicy -Raw | ConvertFrom-Json | Out-Null
        Add-Result "permission_policy" "ready" "Runtime permission policy parse được." $PermissionPolicy
    }
    catch {
        Add-Result "permission_policy" "blocking" "Permission policy tồn tại nhưng JSON lỗi: $($_.Exception.Message)" $PermissionPolicy
    }
}
else {
    Add-Result "permission_policy" "info" "Chưa có runtime override policy; baseline policy sẽ được dùng." $PermissionPolicy
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
        }
        app_local_data = $AppLocalData
        results = $Results
    } | ConvertTo-Json -Depth 6
}
else {
    Write-Host ""
    Write-Host "Assisstant Desktop - Local Windows Verification" -ForegroundColor Cyan
    Write-Host "Repo: $RepoRoot"
    Write-Host "App local data: $AppLocalData"
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
    Write-Host "Suggested manual next commands (not executed by this script):" -ForegroundColor Cyan
    Write-Host "  pnpm install"
    Write-Host "  pnpm --dir apps/desktop sidecar:stage:dev"
    Write-Host "  pnpm --dir apps/desktop tauri dev"
}

if ($Blocking -gt 0) {
    exit 1
}
exit 0
