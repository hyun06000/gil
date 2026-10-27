param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+-owner\.[1-9][0-9]*$')]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Split-Path -Parent $Here

if (-not $IsWindows) { throw 'Windows x64에서만 updater 시험 설치본을 만듭니다.' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'updater 시험 설치본은 Windows x64만 지원합니다.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust cargo가 없습니다.' }
foreach ($Name in @(
    'GIL_UPDATER_PUBLIC_KEY', 'GIL_UPDATER_ENDPOINT', 'TAURI_SIGNING_PRIVATE_KEY',
    'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
)) {
    if (-not (Test-Path "Env:$Name") -or [string]::IsNullOrWhiteSpace((Get-Item "Env:$Name").Value)) {
        throw "$Name 환경 변수가 없습니다. 검증 가능한 updater 설치본만 만듭니다."
    }
}
$UpdaterEndpoint = [uri]$env:GIL_UPDATER_ENDPOINT
if (-not $UpdaterEndpoint.IsAbsoluteUri -or $UpdaterEndpoint.Scheme -ne 'https' -or $UpdaterEndpoint.UserInfo) {
    throw 'GIL_UPDATER_ENDPOINT는 credentials가 없는 절대 HTTPS 주소여야 합니다.'
}
foreach ($Name in @('GIL_WINDOWS_CERTIFICATE_THUMBPRINT', 'GIL_WINDOWS_TIMESTAMP_URL', 'TAURI_CONFIG')) {
    Remove-Item "Env:$Name" -ErrorAction SilentlyContinue
}
$TauriVersion = (cargo tauri --version 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $TauriVersion -ne 'tauri-cli 2.11.5') {
    throw 'Tauri CLI 2.11.5가 필요합니다.'
}

Push-Location $Here
try {
    cargo build --locked --release -p gil --bin gil --bin gil-launcher --features product-safety
    if ($LASTEXITCODE -ne 0) { throw '시험 Core와 고정 런처 빌드가 실패했습니다.' }
    $Core = Join-Path $Root 'target\release\gil.exe'
    $Digest = (Get-FileHash -LiteralPath $Core -Algorithm SHA256).Hash.ToLowerInvariant()
    $Bootstrap = Join-Path $Root 'target\windows-bootstrap'
    New-Item -ItemType Directory -Force -Path $Bootstrap | Out-Null
    @{ schema = 1; version = $Version; sha256 = $Digest; architecture = 'x86_64'; cohort = 'gil-mcp-v1' } |
        ConvertTo-Json -Compress |
        Set-Content -LiteralPath (Join-Path $Bootstrap 'current.json') -Encoding utf8NoBOM

    $Config = Get-Content -Raw (Join-Path $Here 'tauri.unsigned-updater-test-windows.conf.json') | ConvertFrom-Json
    $Config | Add-Member -NotePropertyName version -NotePropertyValue $Version -Force
    # createUpdaterArtifacts is handled by Tauri's bundler, independently from the runtime updater
    # builder in updates.rs. Supply the same public trust material to both layers explicitly.
    $Config.plugins.updater | Add-Member -NotePropertyName pubkey -NotePropertyValue $env:GIL_UPDATER_PUBLIC_KEY -Force
    $Config.plugins.updater | Add-Member -NotePropertyName endpoints -NotePropertyValue @($UpdaterEndpoint.AbsoluteUri) -Force
    $ConfigJson = $Config | ConvertTo-Json -Depth 10 -Compress
    cargo tauri build --bundles nsis --config $ConfigJson
    if ($LASTEXITCODE -ne 0) { throw 'Windows updater 시험 설치본 빌드가 실패했습니다.' }
    $Signatures = @(Get-ChildItem (Join-Path $Root 'target\release\bundle\nsis') -File -Filter '*.sig')
    if ($Signatures.Count -ne 1) { throw "updater 서명은 하나여야 합니다: $($Signatures.Count)" }
    $SignedText = [Text.Encoding]::UTF8.GetString(
        [Convert]::FromBase64String((Get-Content -Raw -LiteralPath $Signatures[0].FullName).Trim())
    )
    if ($SignedText -notmatch [regex]::Escape("version:$Version")) {
        throw "updater 서명이 버전 $Version 을 결속하지 않았습니다."
    }
} finally {
    Pop-Location
}

$Bundle = Join-Path $Root 'target\release\bundle\nsis'
Write-Host "UNSIGNED PUBLISHER / SIGNED UPDATER TEST: $Bundle"
