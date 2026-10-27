$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Split-Path -Parent $Here

if (-not $IsWindows) {
    throw 'Windows x64에서만 설치본을 만듭니다.'
}
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw '첫 제품판은 Windows x64만 지원합니다.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust cargo가 없습니다.'
}
foreach ($Name in @(
    'GIL_UPDATER_PUBLIC_KEY',
    'GIL_UPDATER_ENDPOINT',
    'TAURI_SIGNING_PRIVATE_KEY',
    'GIL_WINDOWS_CERTIFICATE_THUMBPRINT',
    'GIL_WINDOWS_TIMESTAMP_URL'
)) {
    if (-not (Test-Path "Env:$Name") -or [string]::IsNullOrWhiteSpace((Get-Item "Env:$Name").Value)) {
        throw "$Name 환경 변수가 없습니다. 미서명 제품 설치본은 만들지 않습니다."
    }
}
$Thumbprint = $env:GIL_WINDOWS_CERTIFICATE_THUMBPRINT -replace '\s', ''
if ($Thumbprint -notmatch '^[0-9A-Fa-f]{40,64}$') {
    throw 'GIL_WINDOWS_CERTIFICATE_THUMBPRINT가 인증서 thumbprint 형식이 아닙니다.'
}
$UpdaterEndpoint = [uri]$env:GIL_UPDATER_ENDPOINT
if (-not $UpdaterEndpoint.IsAbsoluteUri -or $UpdaterEndpoint.Scheme -ne 'https' -or $UpdaterEndpoint.UserInfo) {
    throw 'GIL_UPDATER_ENDPOINT는 credentials가 없는 절대 HTTPS 주소여야 합니다.'
}
$Timestamp = [uri]$env:GIL_WINDOWS_TIMESTAMP_URL
if ($Timestamp.Scheme -notin @('http', 'https') -or -not $Timestamp.IsAbsoluteUri) {
    throw 'GIL_WINDOWS_TIMESTAMP_URL이 유효한 HTTP(S) 주소가 아닙니다.'
}
# Keep private certificate material in the Windows certificate store. Only the public thumbprint
# and timestamp service are merged into Tauri's build configuration.
$env:TAURI_CONFIG = @{
    bundle = @{ windows = @{
        certificateThumbprint = $Thumbprint
        digestAlgorithm = 'sha256'
        timestampUrl = $Timestamp.AbsoluteUri
        tsp = $true
    }}
    plugins = @{ updater = @{
        pubkey = $env:GIL_UPDATER_PUBLIC_KEY
        endpoints = @($UpdaterEndpoint.AbsoluteUri)
    }}
} | ConvertTo-Json -Depth 4 -Compress
$TauriVersion = (cargo tauri --version 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $TauriVersion -ne 'tauri-cli 2.10.1') {
    throw "Tauri CLI 2.10.1이 필요합니다. 'cargo install tauri-cli --version 2.10.1 --locked'를 실행하세요."
}

Push-Location $Here
try {
    cargo build --locked --release -p gil --bin gil --bin gil-launcher --features product-safety
    if ($LASTEXITCODE -ne 0) {
        throw '제품 Core와 고정 런처 빌드가 실패했습니다.'
    }
    $Core = Join-Path $Root 'target\release\gil.exe'
    $VersionLine = (& $Core --version).Trim()
    if ($VersionLine -notmatch '^gil ([0-9A-Za-z.+-]+)$') {
        throw 'Core 버전을 읽지 못했습니다.'
    }
    $Version = $Matches[1]
    $Configured = (Get-Content -Raw (Join-Path $Here 'tauri.conf.json') | ConvertFrom-Json).version
    if ($Version -ne $Configured) {
        throw "Core 버전($Version)과 Companion 버전($Configured)이 다릅니다."
    }
    $Bootstrap = Join-Path $Root 'target\windows-bootstrap'
    New-Item -ItemType Directory -Force -Path $Bootstrap | Out-Null
    $Digest = (Get-FileHash -LiteralPath $Core -Algorithm SHA256).Hash.ToLowerInvariant()
    @{ schema = 1; version = $Version; sha256 = $Digest; architecture = 'x86_64'; cohort = 'gil-mcp-v1' } |
        ConvertTo-Json -Compress |
        Set-Content -LiteralPath (Join-Path $Bootstrap 'current.json') -Encoding utf8NoBOM

    cargo tauri build --bundles nsis --config tauri.product-windows.conf.json
    if ($LASTEXITCODE -ne 0) {
        throw 'Windows NSIS 설치본 빌드가 실패했습니다.'
    }
} finally {
    Pop-Location
}

$Bundle = Join-Path $Root 'target\release\bundle\nsis'
Write-Host "Windows 설치본: $Bundle"
