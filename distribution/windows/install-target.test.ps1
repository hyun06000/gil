$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'install-target.psm1') -Force
$module = Get-Module install-target
$passed = 0
foreach ($case in @(
    @{Codes=@(9); Expected='x64'},
    @{Codes=@(12); Expected='arm64'},
    @{Codes=@(9,9); Expected='x64'},
    @{Codes=@(12,12); Expected='arm64'}
)) {
    $actual = & $module { param($codes) Resolve-GilWindowsArchitecture -Codes $codes } $case.Codes
    if ($actual -ne $case.Expected) { throw 'Architecture selection regression' }
    $passed++
}
foreach ($case in @(
    @{Codes=@()}, @{Codes=@($null)}, @{Codes=@(0)}, @{Codes=@(5)},
    @{Codes=@(9,12)}, @{Codes=@('unknown')}
)) {
    $refused = $false
    try { & $module { param($codes) Resolve-GilWindowsArchitecture -Codes $codes } $case.Codes | Out-Null }
    catch { $refused = $true }
    if (-not $refused) { throw 'Unsupported or ambiguous CPU accepted' }
    $passed++
}
$target = Get-GilInstallTarget
if ($target.architecture -ne $env:GIL_NATIVE_ARCH) { throw 'Native runner detection mismatch' }
if ($target.installation_started -ne $false) { throw 'Detection claimed installation' }
$passed++
Write-Output "Installer target checks: $passed passed; native $($target.architecture)"
