[CmdletBinding()]
param(
    [string]$AssistantPath = "assistant",
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-AssistantExecutable {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ([System.IO.Path]::IsPathRooted($Value) -or $Value.Contains([System.IO.Path]::DirectorySeparatorChar)) {
        $candidate = [System.IO.Path]::GetFullPath($Value)
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            throw "Assistant executable not found: $candidate"
        }
        return $candidate
    }

    $command = Get-Command $Value -ErrorAction Stop
    if ([string]::IsNullOrWhiteSpace($command.Source)) {
        throw "Cannot resolve assistant executable from '$Value'."
    }
    return $command.Source
}

$assistantExe = Resolve-AssistantExecutable -Value $AssistantPath
$results = [System.Collections.Generic.List[object]]::new()

function Invoke-ReadOnlyCheck {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [bool]$Required = $true
    )

    $previousExitCode = $global:LASTEXITCODE
    $output = ""
    $exitCode = 1
    try {
        $captured = & $script:assistantExe @Arguments 2>&1
        $exitCode = if ($null -eq $global:LASTEXITCODE) { 0 } else { [int]$global:LASTEXITCODE }
        $output = ($captured | Out-String).Trim()
    }
    catch {
        $exitCode = 1
        $output = $_.Exception.Message
    }
    finally {
        $global:LASTEXITCODE = $previousExitCode
    }

    $passed = $exitCode -eq 0
    $result = [pscustomobject]@{
        id = $Id
        label = $Label
        required = $Required
        passed = $passed
        exit_code = $exitCode
        command = "assistant " + ($Arguments -join " ")
        output = $output
    }
    $script:results.Add($result)

    if (-not $script:Json) {
        $marker = if ($passed) { "OK" } elseif ($Required) { "FAIL" } else { "WARN" }
        Write-Host ("[{0,-4}] {1}" -f $marker, $Label)
        if (-not [string]::IsNullOrWhiteSpace($output)) {
            $preview = ($output -split "`r?`n" | Select-Object -First 3) -join " | "
            Write-Host "       $preview"
        }
    }
}

$manualChecks = @(
    [pscustomobject]@{
        id = "android_install"
        label = "Android build/install"
        instruction = "Build/install apps/android-satellite on the target phone and confirm the app opens normally."
    },
    [pscustomobject]@{
        id = "qr_pairing"
        label = "QR pairing"
        instruction = "Run assistant satellite pair --qr, scan it, explicitly connect, and confirm the phone becomes trusted."
    },
    [pscustomobject]@{
        id = "android_activation"
        label = "Android activation surfaces"
        instruction = "Verify in-app voice, Quick Settings Assistant Voice, and launcher shortcut Nói AI all enter the same safe activation flow."
    },
    [pscustomobject]@{
        id = "speech_quality"
        label = "Vietnamese/English recognition"
        instruction = "Validate vi-VN and en-US recognition on the target phone, including on-device -> system fallback and diagnostic metadata."
    },
    [pscustomobject]@{
        id = "desktop_tts"
        label = "Desktop VI/EN TTS"
        instruction = "Verify selected Vietnamese and English SAPI voices are audible and missing preferred voices fall back safely."
    },
    [pscustomobject]@{
        id = "cancel_barge_in"
        label = "Stop / interrupt-to-talk"
        instruction = "Verify Processing and Speaking can be cancelled from Android while Executing/Confirming remain fail-closed."
    },
    [pscustomobject]@{
        id = "conversation"
        label = "Conversational follow-up"
        instruction = "Verify follow-up listening begins only after desktop TTS completes; silence/NO_MATCH ends the session."
    },
    [pscustomobject]@{
        id = "satellite_ui_secret"
        label = "Desktop Satellite diagnostics"
        instruction = "Verify the Quick Satellite panel reports state/devices but never exposes the pairing token in UI/devtools payloads."
    },
    [pscustomobject]@{
        id = "tailscale_mobile"
        label = "Tailscale remote"
        instruction = "With LAN unavailable, validate Android over mobile data inside the intended tailnet; confirm no Funnel/public endpoint is created."
    },
    [pscustomobject]@{
        id = "fallback_voice"
        label = "Desktop fallback voice"
        instruction = "If fallback voice is part of the release, validate Zipformer/wake locally. Missing fallback resources alone must not invalidate Android-primary voice."
    },
    [pscustomobject]@{
        id = "windows_package"
        label = "Windows package/install/startup"
        instruction = "Run the existing release verification/build flow locally, install the NSIS package, and validate startup plus staged helper resolution."
    }
)

if (-not $Json) {
    Write-Host "Assisstant Desktop release readiness"
    Write-Host "Executable: $assistantExe"
    Write-Host "Mode: read-only automated checks + manual hardware acceptance"
    Write-Host ""
}

# These commands are intentionally read-only. This runner does not pair/revoke,
# install firewall rules, mutate Tailscale, start microphone capture, install
# models, build packages, or run test suites.
Invoke-ReadOnlyCheck -Id "version" -Label "Canonical CLI" -Arguments @("version")
Invoke-ReadOnlyCheck -Id "status" -Label "Local status snapshot" -Arguments @("status", "--json")
Invoke-ReadOnlyCheck -Id "runtime_ping" -Label "Authenticated runtime IPC" -Arguments @("runtime", "ping")
Invoke-ReadOnlyCheck -Id "startup" -Label "Windows startup state" -Arguments @("startup", "show")
Invoke-ReadOnlyCheck -Id "permissions" -Label "Permission policy" -Arguments @("permissions", "list")
Invoke-ReadOnlyCheck -Id "satellite_doctor" -Label "Satellite configuration/security" -Arguments @("satellite", "doctor")
Invoke-ReadOnlyCheck -Id "satellite_devices" -Label "Satellite trusted-device registry" -Arguments @("satellite", "devices")
Invoke-ReadOnlyCheck -Id "tts_voices" -Label "Installed SAPI voices" -Arguments @("tts", "voices", "--json")
Invoke-ReadOnlyCheck -Id "tts_preferences" -Label "SAPI voice preferences" -Arguments @("tts", "show", "--json")

# Network-mode checks are advisory because a release can be validated in LAN
# mode, Tailscale mode, or both. Their failures are printed but do not change
# the automated blocking result.
Invoke-ReadOnlyCheck -Id "firewall" -Label "Managed LAN firewall rule" -Arguments @("satellite", "firewall", "show") -Required $false
Invoke-ReadOnlyCheck -Id "tailscale" -Label "Managed Tailscale remote state" -Arguments @("satellite", "remote", "tailscale", "show") -Required $false

$blockingFailures = @($results | Where-Object { $_.required -and -not $_.passed })
$advisories = @($results | Where-Object { -not $_.required -and -not $_.passed })

if ($Json) {
    [pscustomobject]@{
        assistant = $assistantExe
        automated = @($results)
        blocking_failures = $blockingFailures.Count
        advisory_failures = $advisories.Count
        manual = $manualChecks
        ready_for_manual_acceptance = $blockingFailures.Count -eq 0
    } | ConvertTo-Json -Depth 6
}
else {
    Write-Host ""
    Write-Host "Manual target-hardware acceptance"
    foreach ($check in $manualChecks) {
        Write-Host ("[MANUAL] {0}" -f $check.label)
        Write-Host ("         {0}" -f $check.instruction)
    }

    Write-Host ""
    if ($blockingFailures.Count -eq 0) {
        Write-Host "Automated result: PASS — continue with the manual acceptance items above."
        if ($advisories.Count -gt 0) {
            Write-Host "Advisory: $($advisories.Count) optional network-mode check(s) were unavailable or not configured."
        }
    }
    else {
        Write-Host "Automated result: FAIL — $($blockingFailures.Count) required read-only check(s) failed."
    }
    Write-Host "This command does not declare release readiness until every applicable manual hardware gate is validated."
}

if ($blockingFailures.Count -gt 0) {
    exit 1
}
exit 0
