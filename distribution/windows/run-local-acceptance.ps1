param(
    [ValidateSet('Contract', 'Signed')][string]$Mode = 'Contract',
    [string]$CodexExe,
    [switch]$AllowInstallTest
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) { throw 'This acceptance runner must run on Windows.' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'The first product candidate supports Windows x64 only.'
}

foreach ($command in @('node', 'cargo', 'rustup')) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "Required command is missing: $command"
    }
}

$savedToolchain = $env:RUSTUP_TOOLCHAIN
$env:RUSTUP_TOOLCHAIN = '1.97.1'
Push-Location $Root
try {
    rustup toolchain install 1.97.1 --profile minimal --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "rustup exited $LASTEXITCODE" }
    cargo fetch --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "cargo fetch exited $LASTEXITCODE" }

    node --test distribution/windows/product-install.test.mjs
    if ($LASTEXITCODE -ne 0) { throw "product contract tests exited $LASTEXITCODE" }
    cargo +1.97.1 test --locked -p gil --bin gil-launcher
    if ($LASTEXITCODE -ne 0) { throw "launcher tests exited $LASTEXITCODE" }
    cargo +1.97.1 check --locked --target x86_64-pc-windows-msvc -p gil --bin gil-launcher --features product-safety
    if ($LASTEXITCODE -ne 0) { throw "Windows launcher check exited $LASTEXITCODE" }

    if ($Mode -eq 'Contract') {
        Write-Host 'PASS: Windows x64 product contract and launcher checks.'
        return
    }

    if (-not $AllowInstallTest) {
        throw 'Signed mode installs and removes Companion. Re-run with -AllowInstallTest on a clean test PC.'
    }
    if ([string]::IsNullOrWhiteSpace($CodexExe)) {
        $found = Get-Command codex.exe -ErrorAction SilentlyContinue
        if ($found) { $CodexExe = $found.Source }
    }
    if ([string]::IsNullOrWhiteSpace($CodexExe)) {
        throw 'Signed mode requires -CodexExe or codex.exe on PATH.'
    }
    $CodexExe = (Resolve-Path -LiteralPath $CodexExe).Path

    foreach ($name in @(
        'GIL_UPDATER_PUBLIC_KEY',
        'GIL_UPDATER_ENDPOINT',
        'TAURI_SIGNING_PRIVATE_KEY',
        'GIL_WINDOWS_CERTIFICATE_THUMBPRINT',
        'GIL_WINDOWS_TIMESTAMP_URL'
    )) {
        if (-not (Test-Path "Env:$name") -or [string]::IsNullOrWhiteSpace((Get-Item "Env:$name").Value)) {
            throw "Signed mode requires environment variable: $name"
        }
    }
    $thumbprint = $env:GIL_WINDOWS_CERTIFICATE_THUMBPRINT -replace '\s', ''
    $certificate = Get-Item "Cert:\CurrentUser\My\$thumbprint" -ErrorAction SilentlyContinue
    if (-not $certificate -or -not $certificate.HasPrivateKey) {
        throw 'The selected CurrentUser signing certificate with a private key was not found.'
    }
    if ($certificate.NotBefore -gt (Get-Date) -or $certificate.NotAfter -le (Get-Date)) {
        throw 'The selected signing certificate is not currently valid.'
    }
    if (@($certificate.EnhancedKeyUsageList | ForEach-Object { $_.ObjectId.Value }) -notcontains '1.3.6.1.5.5.7.3.3') {
        throw 'The selected certificate is not valid for code signing.'
    }
    $runtime = Join-Path $env:LOCALAPPDATA 'GIL\runtime'
    if (Test-Path -LiteralPath $runtime) {
        throw "Install acceptance requires a clean test PC; runtime already exists: $runtime"
    }
    if (-not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
        cargo install tauri-cli --version 2.10.1 --locked
        if ($LASTEXITCODE -ne 0) { throw "tauri-cli installation exited $LASTEXITCODE" }
    }

    & (Join-Path $Root 'companion\build-windows.ps1')
    if ($LASTEXITCODE -ne 0) { throw "signed build exited $LASTEXITCODE" }
    $installers = @(Get-ChildItem (Join-Path $Root 'target\release\bundle\nsis') -File -Filter '*.exe')
    if ($installers.Count -ne 1) { throw "Expected one NSIS installer, found $($installers.Count)" }
    $evidence = Join-Path $Root 'target\windows-product-evidence\acceptance.json'
    & (Join-Path $Root 'distribution\windows\product-e2e.ps1') `
        -Installer $installers[0].FullName -CodexExe $CodexExe -Evidence $evidence
    if ($LASTEXITCODE -ne 0) { throw "product acceptance exited $LASTEXITCODE" }
    Write-Host "PASS: signed Windows product acceptance. Evidence: $evidence"
} finally {
    Pop-Location
    $env:RUSTUP_TOOLCHAIN = $savedToolchain
}
