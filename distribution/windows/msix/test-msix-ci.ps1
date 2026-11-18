# Maintainer/CI only: disposable Windows account with no Claude/Codex or other GIL package.
# Requires PowerShell 7, Node, Windows SDK and permission to trust a temporary certificate.
# Does not submit, publish or install a production signing identity.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $StartExe,
    [Parameter(Mandatory)] [string] $Version,
    # PR binaries carry a -dev suffix; MSIX versions must be numeric. Release callers omit this.
    [string] $ExpectedBinaryVersion = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'MSIX installation checks require Windows' }
if (Get-AppxPackage -Name 'GIL.Dev') { throw 'Existing GIL.Dev package: use a clean disposable account' }
if (Test-Path -LiteralPath (Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps\gil.exe')) {
    throw 'An existing gil.exe execution alias would conflict; use a clean disposable account'
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'MSIX check requires a numeric X.Y.Z version' }
if (-not $ExpectedBinaryVersion) { $ExpectedBinaryVersion = $Version }
$Here = $PSScriptRoot
$work = Join-Path ([IO.Path]::GetTempPath()) ('gil-msix-check-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$cert = $null
$native = Join-Path $env:LOCALAPPDATA 'Programs\gil\gil.exe'
$nativeBefore = if (Test-Path -LiteralPath $native) { (Get-FileHash -LiteralPath $native).Hash } else { $null }
try {
    $cert = New-SelfSignedCertificate -Type Custom -Subject 'CN=GIL Dev' -KeyUsage DigitalSignature `
        -FriendlyName 'GIL temporary MSIX check' -CertStoreLocation Cert:\CurrentUser\My `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
    $pfx = Join-Path $work 'dev.pfx'
    $password = [guid]::NewGuid().ToString('N')
    $secure = ConvertTo-SecureString $password -AsPlainText -Force
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $secure | Out-Null
    Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\LocalMachine\TrustedPeople -Password $secure | Out-Null
    $msix = Join-Path $work 'gil-dev.msix'
    & "$Here/build-msix.ps1" -Exe $Exe -StartExe $StartExe -Version $Version -IdentityName 'GIL.Dev' `
        -Publisher 'CN=GIL Dev' -PublisherDisplayName 'GIL Dev' -Out $msix -SignWithPfx $pfx -PfxPassword $password
    Add-AppxPackage -Path $msix
    $package = Get-AppxPackage -Name 'GIL.Dev'
    if (-not $package -or $package.Version.ToString() -ne "$Version.0") { throw 'Installed package version mismatch' }
    $manifest = Get-AppxPackageManifest -Package $package.PackageFullName
    $ns = [Xml.XmlNamespaceManager]::new($manifest.NameTable)
    $ns.AddNamespace('f', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $ns.AddNamespace('uap', 'http://schemas.microsoft.com/appx/manifest/uap/windows10')
    $ns.AddNamespace('desktop6', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10/6')
    if ($manifest.SelectSingleNode('//uap:VisualElements', $ns).AppListEntry -ne 'default') { throw 'GIL must be visible in Start' }
    if ($manifest.SelectSingleNode('//f:TargetDeviceFamily', $ns).MinVersion -ne '10.0.18362.0') { throw 'Unexpected minimum OS' }
    if ($manifest.SelectSingleNode('//desktop6:RegistryWriteVirtualization', $ns)) { throw 'Unexpected registry write exception' }
    $application = $manifest.SelectSingleNode('//f:Application', $ns)
    if ($application.Executable -ne 'gil-start.exe') { throw 'Start must launch the guide' }
    $start = Join-Path $package.InstallLocation 'gil-start.exe'
    # Activate the installed app by AUMID, then read and interact with its actual console.
    & pwsh -NoProfile -STA -File "$Here/test-start-menu.ps1" -AppId "$($package.PackageFamilyName)!GIL" -ExpectedExe $start
    if ($LASTEXITCODE -ne 0) { throw 'Installed Start-menu activation/guide check failed' }
    $alias = Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps\gil.exe'
    if (-not (Test-Path -LiteralPath $alias)) { throw 'App execution alias missing' }
    # Invoke the registered alias, never the unpacked exe or a PATH-selected native installation.
    node "$Here/smoke.mjs" $alias $ExpectedBinaryVersion
    if ($LASTEXITCODE -ne 0) { throw 'Packaged CLI/MCP smoke failed' }
    & $alias install --no-wait
    if ($LASTEXITCODE -ne 0) { throw 'Packaged gil install failed' }
    $doctor = (& $alias doctor) -join "`n"
    if ($LASTEXITCODE -ne 0 -or $doctor -notmatch 'binary: OK .*\[store\]' -or
        $doctor -notmatch 'claude-desktop: NOT_INSTALLED' -or $doctor -notmatch 'codex: NOT_INSTALLED') {
        throw 'Packaged doctor failed, or this is not a clean account without Hosts'
    }
    $nativeAfter = if (Test-Path -LiteralPath $native) { (Get-FileHash -LiteralPath $native).Hash } else { $null }
    if ($nativeAfter -ne $nativeBefore) { throw 'Packaged install created or replaced a native gil.exe' }
    Write-Host 'PASS: installed manifest, Start activation and bilingual console, CLI/MCP alias, install and doctor; actual Host UI and WACK not tested'
} finally {
    # The preflight rejected an existing GIL.Dev. Remove only this check's package/certificates.
    try { Get-AppxPackage -Name 'GIL.Dev' | Remove-AppxPackage } finally {
        if ($cert) {
            Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath "Cert:\LocalMachine\TrustedPeople\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $work -Recurse -Force
    }
}
