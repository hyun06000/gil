param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$CodexExe,
    [Parameter(Mandatory = $true)][string]$Evidence,
    [ValidateSet('Valid', 'NotSigned')][string]$ExpectedSignature = 'Valid',
    [string]$DisplayName = 'GIL Companion',
    [switch]$SeedLegacyRuntime
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Installer = (Resolve-Path -LiteralPath $Installer).Path
$CodexExe = (Resolve-Path -LiteralPath $CodexExe).Path
$Runtime = Join-Path $env:LOCALAPPDATA 'GIL\runtime'
if (Test-Path -LiteralPath $Runtime) { throw "Runner is not clean: $Runtime already exists" }
$LegacyCore = $null
$LegacyHash = $null
if ($SeedLegacyRuntime) {
    $LegacyCore = Join-Path $Runtime 'versions\0.1.0\gil.exe'
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $LegacyCore) | Out-Null
    Set-Content -LiteralPath $LegacyCore -Value 'preserved legacy owner-test Core' -Encoding utf8NoBOM
    $LegacyHash = (Get-FileHash -LiteralPath $LegacyCore -Algorithm SHA256).Hash.ToLowerInvariant()
    [ordered]@{ schema = 1; version = '0.1.0'; sha256 = $LegacyHash; architecture = 'x86_64'; cohort = 'gil-mcp-v1' } |
        ConvertTo-Json -Compress |
        Set-Content -LiteralPath (Join-Path $Runtime 'current.json') -Encoding utf8NoBOM
}

$process = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "NSIS installer exited $($process.ExitCode)" }

$Launcher = Join-Path $Runtime 'gil-launcher.exe'
$ReceiptPath = Join-Path $Runtime 'current.json'
if (-not (Test-Path -LiteralPath $Launcher -PathType Leaf)) { throw 'Stable launcher was not installed' }
if (-not (Test-Path -LiteralPath $ReceiptPath -PathType Leaf)) { throw 'Runtime receipt was not installed' }
$Receipt = Get-Content -Raw -LiteralPath $ReceiptPath | ConvertFrom-Json
if ($Receipt.schema -ne 1 -or $Receipt.architecture -ne 'x86_64' -or $Receipt.cohort -ne 'gil-mcp-v1') {
    throw "Unexpected runtime receipt: $(Get-Content -Raw -LiteralPath $ReceiptPath)"
}
$Core = Join-Path $Runtime "versions\$($Receipt.version)\gil.exe"
if (-not (Test-Path -LiteralPath $Core -PathType Leaf)) { throw 'Selected Core was not installed' }
$CoreHash = (Get-FileHash -LiteralPath $Core -Algorithm SHA256).Hash.ToLowerInvariant()
if ($CoreHash -ne $Receipt.sha256) { throw 'Installed Core digest differs from the receipt' }
if ($SeedLegacyRuntime) {
    if ($Receipt.version -eq '0.1.0') { throw 'Owner candidate reused the legacy runtime directory' }
    if (-not (Test-Path -LiteralPath $LegacyCore -PathType Leaf) -or
        (Get-FileHash -LiteralPath $LegacyCore -Algorithm SHA256).Hash.ToLowerInvariant() -ne $LegacyHash) {
        throw 'Installation changed or removed the preserved legacy runtime'
    }
}

# Re-running the exact same installer is a normal repair action. It must preserve immutable Core
# bytes, refresh the Companion files and receipt, and exit successfully.
$repair = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($repair.ExitCode -ne 0) { throw "NSIS repair install exited $($repair.ExitCode)" }
$RepairReceipt = Get-Content -Raw -LiteralPath $ReceiptPath | ConvertFrom-Json
if ($RepairReceipt.version -ne $Receipt.version -or
    (Get-FileHash -LiteralPath $Core -Algorithm SHA256).Hash.ToLowerInvariant() -ne $CoreHash) {
    throw 'Repair install changed the selected immutable Core'
}

$entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Get-ItemProperty |
    Where-Object DisplayName -eq $DisplayName)
if ($entries.Count -ne 1) { throw "Expected one GIL Companion uninstall entry, found $($entries.Count)" }
$uninstallText = $entries[0].UninstallString
if ($uninstallText -match '^"([^\"]+\.exe)"(?:\s.*)?$') {
    $Uninstaller = $Matches[1]
} elseif ($uninstallText -match '^([^\r\n]+\.exe)$') {
    $Uninstaller = $Matches[1]
} else {
    throw "Unexpected uninstall command: $uninstallText"
}
$Uninstaller = (Resolve-Path -LiteralPath $Uninstaller).Path

# Installing bytes is not product acceptance: a person must get a visible window by launching the
# installed application. Capture startup output so a release-subsystem crash is diagnosable instead
# of looking like a double-click that did nothing.
$InstallDir = Split-Path -Parent $Uninstaller
$Companions = @(Get-ChildItem -LiteralPath $InstallDir -File -Filter '*.exe' |
    Where-Object FullName -ne $Uninstaller)
if ($Companions.Count -ne 1) { throw "Expected one installed Companion executable, found $($Companions.Count)" }
$GuiStdout = Join-Path $env:RUNNER_TEMP 'gil-companion-gui.stdout.log'
$GuiStderr = Join-Path $env:RUNNER_TEMP 'gil-companion-gui.stderr.log'
$gui = Start-Process -FilePath $Companions[0].FullName -PassThru `
    -RedirectStandardOutput $GuiStdout -RedirectStandardError $GuiStderr
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 200
        $gui.Refresh()
        if ($gui.HasExited) {
            $stdout = if (Test-Path -LiteralPath $GuiStdout) { Get-Content -Raw -LiteralPath $GuiStdout } else { '' }
            $stderr = if (Test-Path -LiteralPath $GuiStderr) { Get-Content -Raw -LiteralPath $GuiStderr } else { '' }
            throw "Companion exited before showing a window (exit $($gui.ExitCode)). stdout: $stdout stderr: $stderr"
        }
    } while ($gui.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)
    if ($gui.MainWindowHandle -eq 0) {
        throw 'Companion stayed alive but did not show a top-level window within 20 seconds'
    }
} finally {
    $gui.Refresh()
    if (-not $gui.HasExited) {
        Stop-Process -Id $gui.Id -Force
        $gui.WaitForExit()
    }
}

# Model a real side-by-side update without rebuilding an untrusted second installer. The selected
# previous-version directory contains verified Core bytes and stays mapped by the live MCP process;
# the installer must select its own immutable version beside it without stopping that connection.
$PreviousVersion = "$($Receipt.version)-previous-test"
$PreviousCore = Join-Path $Runtime "versions\$PreviousVersion\gil.exe"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PreviousCore) | Out-Null
Copy-Item -LiteralPath $Core -Destination $PreviousCore
[ordered]@{ schema = 1; version = $PreviousVersion; sha256 = $CoreHash; architecture = 'x86_64'; cohort = 'gil-mcp-v1' } |
    ConvertTo-Json -Compress |
    Set-Content -LiteralPath $ReceiptPath -Encoding utf8NoBOM

node distribution/windows/product-launcher-smoke.mjs $Launcher $PreviousCore $Installer $ReceiptPath $Receipt.version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$UpdatedReceipt = Get-Content -Raw -LiteralPath $ReceiptPath | ConvertFrom-Json
if ($UpdatedReceipt.version -ne $Receipt.version) { throw 'Installer did not select the new Core for the next MCP connection' }
if ((Get-FileHash -LiteralPath $PreviousCore -Algorithm SHA256).Hash.ToLowerInvariant() -ne $CoreHash) {
    throw 'Update changed the Core used by the existing MCP connection'
}

# Exercise the same official CLI command shape used by the Companion button, but isolate all test
# configuration under RUNNER_TEMP. The test never reads or changes the runner's normal Codex home.
$CodexHome = Join-Path $env:RUNNER_TEMP 'gil-product-codex-home'
New-Item -ItemType Directory -Path $CodexHome | Out-Null
$savedCodexHome = $env:CODEX_HOME
try {
    $env:CODEX_HOME = $CodexHome
    & $CodexExe mcp add gil -- $Launcher mcp --serve
    if ($LASTEXITCODE -ne 0) { throw "codex mcp add exited $LASTEXITCODE" }
    $listed = (& $CodexExe mcp list | Out-String)
    if ($LASTEXITCODE -ne 0 -or $listed -notmatch '(?m)^gil\s') { throw "Codex did not list gil: $listed" }
    $Config = (Get-Content -Raw -LiteralPath (Join-Path $CodexHome 'config.toml')).Replace('\\', '\')
    if ($Config -notmatch [regex]::Escape($Launcher) -or $Config -notmatch 'mcp' -or $Config -notmatch '--serve') {
        throw 'Codex registration does not point to the installed launcher'
    }
    & $CodexExe mcp remove gil
    if ($LASTEXITCODE -ne 0) { throw "codex mcp remove exited $LASTEXITCODE" }
    $afterRemove = (& $CodexExe mcp list | Out-String)
    if ($LASTEXITCODE -ne 0 -or $afterRemove -match '(?m)^gil\s') { throw "Codex still lists gil: $afterRemove" }
} finally {
    $env:CODEX_HOME = $savedCodexHome
}

$Signature = Get-AuthenticodeSignature -LiteralPath $Installer
if ($Signature.Status.ToString() -ne $ExpectedSignature) {
    throw "Installer signature is $($Signature.Status), expected ${ExpectedSignature}: $($Signature.StatusMessage)"
}

# Tauri owns Companion removal. The external runtime is deliberately preserved because another
# already-open Codex conversation may still be using it; Project directories are never in scope.
$entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Get-ItemProperty |
    Where-Object DisplayName -eq $DisplayName)
if ($entries.Count -ne 1) { throw "Expected one GIL Companion uninstall entry, found $($entries.Count)" }
$uninstallText = $entries[0].UninstallString
if ($uninstallText -match '^"([^"]+\.exe)"(?:\s.*)?$') {
    $Uninstaller = $Matches[1]
} elseif ($uninstallText -match '^([^\r\n]+\.exe)$') {
    $Uninstaller = $Matches[1]
} else {
    throw "Unexpected uninstall command: $uninstallText"
}
$Uninstaller = (Resolve-Path -LiteralPath $Uninstaller).Path
$removed = Start-Process -FilePath $Uninstaller -ArgumentList '/S' -Wait -PassThru
if ($removed.ExitCode -ne 0) { throw "NSIS uninstaller exited $($removed.ExitCode)" }
if (-not (Test-Path -LiteralPath $Launcher -PathType Leaf) -or -not (Test-Path -LiteralPath $Core -PathType Leaf)) {
    throw 'Uninstall removed the preserved runtime'
}
$left = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Get-ItemProperty |
    Where-Object DisplayName -eq $DisplayName)
if ($left.Count -ne 0) { throw 'Companion uninstall registration remains' }

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Evidence) | Out-Null
$SignerSubject = if ($Signature.SignerCertificate) { $Signature.SignerCertificate.Subject } else { $null }
$SignerThumbprint = if ($Signature.SignerCertificate) { $Signature.SignerCertificate.Thumbprint.ToLowerInvariant() } else { $null }
[ordered]@{
    schema = 1
    source = $env:GITHUB_SHA
    installer = [ordered]@{
        file = Split-Path -Leaf $Installer
        bytes = (Get-Item -LiteralPath $Installer).Length
        sha256 = (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
        signer_subject = $SignerSubject
        signer_thumbprint = $SignerThumbprint
        signature_status = $Signature.Status.ToString()
    }
    runtime = [ordered]@{ version = $Receipt.version; architecture = $Receipt.architecture; cohort = $Receipt.cohort; core_sha256 = $CoreHash }
    acceptance = [ordered]@{ silent_current_user_install = $true; idempotent_repair_install = $true; version_switch_while_mcp_connected = $true; old_connection_survived_update = $true; new_core_selected_for_next_connection = $true; legacy_runtime_preserved = [bool]$SeedLegacyRuntime; visible_companion_window = $true; launcher_mcp = $true; running_core_write_refused = $true; codex_cli_registration = $true; codex_cli_disconnect = $true; companion_uninstall = $true; runtime_preserved = $true }
    exclusions = @('Companion UI contents', 'Codex Desktop conversation', 'complete runtime cleanup', 'SmartScreen reputation')
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Evidence -Encoding utf8NoBOM
Get-Content -Raw -LiteralPath $Evidence
