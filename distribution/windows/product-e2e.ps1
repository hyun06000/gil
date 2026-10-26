param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$CodexExe,
    [Parameter(Mandatory = $true)][string]$Evidence
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Installer = (Resolve-Path -LiteralPath $Installer).Path
$CodexExe = (Resolve-Path -LiteralPath $CodexExe).Path
$Runtime = Join-Path $env:LOCALAPPDATA 'GIL\runtime'
if (Test-Path -LiteralPath $Runtime) { throw "Runner is not clean: $Runtime already exists" }

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

node distribution/windows/product-launcher-smoke.mjs $Launcher $Core
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

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
if ($Signature.Status -ne 'Valid') { throw "Installer signature is $($Signature.Status): $($Signature.StatusMessage)" }

# Tauri owns Companion removal. The external runtime is deliberately preserved because another
# already-open Codex conversation may still be using it; Project directories are never in scope.
$entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Get-ItemProperty |
    Where-Object DisplayName -eq 'GIL Companion')
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
    Where-Object DisplayName -eq 'GIL Companion')
if ($left.Count -ne 0) { throw 'Companion uninstall registration remains' }

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Evidence) | Out-Null
[ordered]@{
    schema = 1
    source = $env:GITHUB_SHA
    installer = [ordered]@{
        file = Split-Path -Leaf $Installer
        bytes = (Get-Item -LiteralPath $Installer).Length
        sha256 = (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
        signer_subject = $Signature.SignerCertificate.Subject
        signer_thumbprint = $Signature.SignerCertificate.Thumbprint.ToLowerInvariant()
        signature_status = $Signature.Status.ToString()
    }
    runtime = [ordered]@{ version = $Receipt.version; architecture = $Receipt.architecture; cohort = $Receipt.cohort; core_sha256 = $CoreHash }
    acceptance = [ordered]@{ silent_current_user_install = $true; launcher_mcp = $true; running_core_write_refused = $true; codex_cli_registration = $true; codex_cli_disconnect = $true; companion_uninstall = $true; runtime_preserved = $true }
    exclusions = @('visible Companion UI', 'Codex Desktop conversation', 'complete runtime cleanup', 'repair', 'SmartScreen reputation')
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Evidence -Encoding utf8NoBOM
Get-Content -Raw -LiteralPath $Evidence
