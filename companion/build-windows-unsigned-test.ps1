$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Split-Path -Parent $Here

if (-not $IsWindows) { throw 'Windows x64에서만 내부 테스트 설치본을 만듭니다.' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw '내부 테스트 설치본은 Windows x64만 지원합니다.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust cargo가 없습니다.' }
$TauriVersion = (cargo tauri --version 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $TauriVersion -ne 'tauri-cli 2.10.1') {
    throw "Tauri CLI 2.10.1이 필요합니다."
}

# This path exists only for owner-operated acceptance on a disposable PC. It deliberately strips
# every release trust input so the output can neither masquerade as signed nor emit updater files.
$ReleaseVariables = @(
    'GIL_UPDATER_PUBLIC_KEY', 'GIL_UPDATER_ENDPOINT', 'TAURI_SIGNING_PRIVATE_KEY',
    'TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'GIL_WINDOWS_CERTIFICATE_THUMBPRINT',
    'GIL_WINDOWS_TIMESTAMP_URL', 'TAURI_CONFIG'
)
$Saved = @{}
foreach ($Name in $ReleaseVariables) {
    if (Test-Path "Env:$Name") { $Saved[$Name] = (Get-Item "Env:$Name").Value }
    Remove-Item "Env:$Name" -ErrorAction SilentlyContinue
}

Push-Location $Here
try {
    cargo build --locked --release -p gil --bin gil --bin gil-launcher --features product-safety
    if ($LASTEXITCODE -ne 0) { throw '테스트 Core와 고정 런처 빌드가 실패했습니다.' }
    $Core = Join-Path $Root 'target\release\gil.exe'
    $VersionLine = (& $Core --version).Trim()
    if ($VersionLine -notmatch '^gil ([0-9A-Za-z.+-]+)$') { throw 'Core 버전을 읽지 못했습니다.' }
    $CoreVersion = $Matches[1]
    $Configured = (Get-Content -Raw (Join-Path $Here 'tauri.conf.json') | ConvertFrom-Json).version
    if ($CoreVersion -ne $Configured) { throw "Core 버전($CoreVersion)과 Companion 버전($Configured)이 다릅니다." }

    $Bootstrap = Join-Path $Root 'target\windows-bootstrap'
    New-Item -ItemType Directory -Force -Path $Bootstrap | Out-Null
    $Digest = (Get-FileHash -LiteralPath $Core -Algorithm SHA256).Hash.ToLowerInvariant()
    # Owner candidates can change bytes several times before a public version is chosen. Give the
    # preserved runtime directory a content-specific prerelease version so an older test install can
    # remain immutable while this candidate is installed beside it.
    $RuntimeVersion = "$CoreVersion-owner.$($Digest.Substring(0, 12))"
    $env:TAURI_CONFIG = @{ version = $RuntimeVersion } | ConvertTo-Json -Compress
    @{ schema = 1; version = $RuntimeVersion; sha256 = $Digest; architecture = 'x86_64'; cohort = 'gil-mcp-v1' } |
        ConvertTo-Json -Compress |
        Set-Content -LiteralPath (Join-Path $Bootstrap 'current.json') -Encoding utf8NoBOM

    cargo tauri build --bundles nsis --config tauri.unsigned-test-windows.conf.json
    if ($LASTEXITCODE -ne 0) { throw 'Windows 무서명 내부 테스트 설치본 빌드가 실패했습니다.' }
} finally {
    Pop-Location
    foreach ($Name in $ReleaseVariables) {
        Remove-Item "Env:$Name" -ErrorAction SilentlyContinue
        if ($Saved.ContainsKey($Name)) { Set-Item "Env:$Name" $Saved[$Name] }
    }
}

$Bundle = Join-Path $Root 'target\release\bundle\nsis'
Write-Host "UNSIGNED INTERNAL TEST ONLY: $Bundle"
