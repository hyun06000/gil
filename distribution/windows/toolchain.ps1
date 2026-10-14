# Maintainer CI only. Select one stable MSVC installation; do not print its local paths.
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installations = @(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0 -or $installations.Count -ne 1) { throw 'One stable MSVC installation required' }
$vs = $installations[0]
if ($vs.isPrerelease -or -not $vs.isComplete) { throw 'Incomplete or preview MSVC refused' }
$devshell = Join-Path $vs.installationPath 'Common7/Tools/Launch-VsDevShell.ps1'
# Use Microsoft's PowerShell entry point; never round-trip a spaced path through cmd quoting.
& $devshell -SkipAutomaticLocation -Arch amd64 -HostArch amd64 6>$null | Out-Null
$selected = @{}
Get-ChildItem Env: | ForEach-Object { $selected[$_.Name] = $_.Value }
$link = Join-Path $selected.VCToolsInstallDir 'bin/Hostx64/x64/link.exe'
$vcVersion = $selected.VCToolsVersion.TrimEnd('\')
$sdkVersion = $selected.WindowsSDKVersion.TrimEnd('\')
if ($vcVersion -notmatch '^\d+\.\d+\.\d+$' -or $sdkVersion -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Unknown toolchain version shape' }
$files = @(
  @{name='link.exe'; path=$link},
  @{name='libcmt.lib'; path=(Join-Path $selected.VCToolsInstallDir 'lib/x64/libcmt.lib')},
  @{name='libvcruntime.lib'; path=(Join-Path $selected.VCToolsInstallDir 'lib/x64/libvcruntime.lib')},
  @{name='libucrt.lib'; path=(Join-Path $selected.WindowsSdkDir "Lib/$sdkVersion/ucrt/x64/libucrt.lib")}
)
$hashes = @($files | ForEach-Object { @{name=$_.name; sha256=(Get-FileHash -LiteralPath $_.path -Algorithm SHA256).Hash.ToLowerInvariant()} })
# Paths are returned privately to the build driver, never included in the public receipt.
@{
  environment=@{PATH=$selected.PATH; LIB=$selected.LIB; INCLUDE=$selected.INCLUDE; CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=$link};
  receipt=@{schema=1; visual_studio=$vs.installationVersion; product=$vs.productId; prerelease=$false;
    msvc=$vcVersion; windows_sdk=$sdkVersion; files=$hashes;
    runner_image=$env:ImageOS; runner_version=$env:ImageVersion}
} | ConvertTo-Json -Depth 6 -Compress
