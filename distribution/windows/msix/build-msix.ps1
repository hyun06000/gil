# Package the single static gil.exe as an MSIX for the Microsoft Store (unsigned upload) or,
# with -SignWithPfx, as a self-signed package for a sideload check. Windows only; needs the
# Windows SDK (makeappx.exe, signtool.exe). Never installs anything by itself.
#
#   ./build-msix.ps1 -Exe target\x86_64-pc-windows-msvc\release\gil.exe -Version 0.3.0 `
#       -IdentityName 12345Publisher.GIL -Publisher 'CN=...' -PublisherDisplayName 'Publisher' `
#       -Out target\msix\gil-store.msix
#
# Store identity values come from Partner Center (Product identity page). Store signs the
# uploaded package; do not sign it yourself. For the sideload check pass -SignWithPfx and the
# matching dev identity (Publisher must equal the certificate subject).
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $IdentityName,
    [Parameter(Mandatory)] [string] $Publisher,
    [Parameter(Mandatory)] [string] $PublisherDisplayName,
    [Parameter(Mandatory)] [string] $Out,
    [string] $SignWithPfx = '',
    [string] $PfxPassword = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $IsWindows) { throw 'MSIX packaging runs on Windows (Windows SDK makeappx.exe).' }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must be X.Y.Z (Store packages use X.Y.Z.0): $Version" }
if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { throw "executable not found: $Exe" }
if ($Publisher -notmatch '^CN=') { throw "Publisher must be the distinguished name from Partner Center (CN=...): $Publisher" }

function Find-SdkTool([string] $Name) {
    $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "$env:ProgramFiles\Windows Kits\10\bin")
    $tool = Get-ChildItem -Path $roots -Recurse -Filter $Name -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName | Select-Object -Last 1
    if (-not $tool) { throw "$Name not found under the Windows SDK; install the Windows 10/11 SDK" }
    $tool.FullName
}

$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Layout = Join-Path ([IO.Path]::GetTempPath()) ("gil-msix-layout-" + [guid]::NewGuid().ToString('N'))
$OutDir = Split-Path -Parent $Out
if ($OutDir) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
New-Item -ItemType Directory -Force -Path $Layout | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $Layout 'Assets') | Out-Null

# Layout: gil.exe + Assets + the manifest with the placeholders filled in. No other files.
Copy-Item -LiteralPath $Exe -Destination (Join-Path $Layout 'gil.exe')
Get-ChildItem -LiteralPath (Join-Path $Here 'Assets') -Filter '*.png' | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $Layout "Assets\$($_.Name)")
}
$manifest = Get-Content -Raw -LiteralPath (Join-Path $Here 'AppxManifest.xml')
foreach ($pair in @(
    @('{{IDENTITY_NAME}}', $IdentityName), @('{{PUBLISHER}}', $Publisher),
    @('{{PUBLISHER_DISPLAY_NAME}}', $PublisherDisplayName), @('{{VERSION}}', "$Version.0")
)) {
    if (-not $manifest.Contains($pair[0])) { throw "placeholder missing from template: $($pair[0])" }
    $manifest = $manifest.Replace($pair[0], [System.Security.SecurityElement]::Escape($pair[1]))
}
if ($manifest -match '\{\{[A-Z_]+\}\}') { throw "unfilled placeholder remains: $($Matches[0])" }
[IO.File]::WriteAllText((Join-Path $Layout 'AppxManifest.xml'), $manifest, [Text.UTF8Encoding]::new($false))

$makeappx = Find-SdkTool 'makeappx.exe'
if (Test-Path -LiteralPath $Out) { Remove-Item -LiteralPath $Out -Force }
& $makeappx pack /o /d $Layout /p $Out
if ($LASTEXITCODE -ne 0) { throw "makeappx failed with $LASTEXITCODE" }

if ($SignWithPfx) {
    $signtool = Find-SdkTool 'signtool.exe'
    & $signtool sign /fd SHA256 /f $SignWithPfx /p $PfxPassword $Out
    if ($LASTEXITCODE -ne 0) { throw "signtool failed with $LASTEXITCODE" }
}

Remove-Item -LiteralPath $Layout -Recurse -Force
$hash = (Get-FileHash -LiteralPath $Out -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "MSIX: $Out ($((Get-Item -LiteralPath $Out).Length) bytes, SHA-256 $hash, signed=$([bool]$SignWithPfx))"
