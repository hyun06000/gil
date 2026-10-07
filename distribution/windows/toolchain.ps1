# Maintainer CI only. Select one stable MSVC installation; do not print its local paths.
param([ValidateSet('x64', 'arm64')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$component = if ($Architecture -eq 'arm64') { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
$devArch = if ($Architecture -eq 'arm64') { 'arm64' } else { 'amd64' }
$hostFolder = if ($Architecture -eq 'arm64') { 'Hostarm64' } else { 'Hostx64' }
$linkerKey = if ($Architecture -eq 'arm64') { 'CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_LINKER' } else { 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER' }
$machine = if ($Architecture -eq 'arm64') { 'ARM64' } else { 'AMD64' }
if ($env:PROCESSOR_ARCHITECTURE -ne $machine) { throw 'Native toolchain host required' }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installations = @(& $vswhere -latest -products '*' -requires $component -format json | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0 -or $installations.Count -ne 1) { throw 'One stable MSVC installation required' }
$vs = $installations[0]
if ($vs.isPrerelease -or -not $vs.isComplete) { throw 'Incomplete or preview MSVC refused' }
$devshell = Join-Path $vs.installationPath 'Common7/Tools/Launch-VsDevShell.ps1'
# Use Microsoft's PowerShell entry point; never round-trip a spaced path through cmd quoting.
& $devshell -SkipAutomaticLocation -Arch $devArch -HostArch $devArch 6>$null | Out-Null
$selected = @{}
Get-ChildItem Env: | ForEach-Object { $selected[$_.Name] = $_.Value }
$link = Join-Path $selected.VCToolsInstallDir "bin/$hostFolder/$Architecture/link.exe"
$vcVersion = $selected.VCToolsVersion.TrimEnd('\')
$sdkVersion = $selected.WindowsSDKVersion.TrimEnd('\')
if ($vcVersion -notmatch '^\d+\.\d+\.\d+$' -or $sdkVersion -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Unknown toolchain version shape' }
$files = @(
  @{name='link.exe'; path=$link},
  @{name='libcmt.lib'; path=(Join-Path $selected.VCToolsInstallDir "lib/$Architecture/libcmt.lib")},
  @{name='libvcruntime.lib'; path=(Join-Path $selected.VCToolsInstallDir "lib/$Architecture/libvcruntime.lib")},
  @{name='libucrt.lib'; path=(Join-Path $selected.WindowsSdkDir "Lib/$sdkVersion/ucrt/$Architecture/libucrt.lib")}
)
$hashes = @($files | ForEach-Object { @{name=$_.name; sha256=(Get-FileHash -LiteralPath $_.path -Algorithm SHA256).Hash.ToLowerInvariant()} })
# Paths are returned privately to the build driver, never included in the public receipt.
$buildEnvironment = @{PATH=$selected.PATH; LIB=$selected.LIB; INCLUDE=$selected.INCLUDE}
$buildEnvironment[$linkerKey] = $link
@{
  environment=$buildEnvironment;
  receipt=@{schema=1; architecture=$Architecture; visual_studio=$vs.installationVersion; product=$vs.productId; prerelease=$false;
    msvc=$vcVersion; windows_sdk=$sdkVersion; files=$hashes;
    runner_image=$env:ImageOS; runner_version=$env:ImageVersion}
} | ConvertTo-Json -Depth 6 -Compress
