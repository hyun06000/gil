# Installer target detection: decision table, live sources, fault injection (CIM refused, cmdlet
# absent, registry refused, everything refused, contradiction) and emulation (x86 WOW host, any
# non-native pwsh found). The guidance block from DEV-CANDIDATE-CHECK.md is extracted and run
# under the same conditions so the published text is what is tested.
$ErrorActionPreference = 'Stop'
$native = $env:GIL_NATIVE_ARCH
if (-not $native) { throw 'GIL_NATIVE_ARCH is required' }
$modulePath = Join-Path $PSScriptRoot 'install-target.psm1'
Import-Module $modulePath -Force
$module = Get-Module install-target
$results = [ordered]@{ native = $native; passed = 0; hosts = @(); cases = @() }

# ── 1. Decision table (module internals, no OS access) ─────────────────────────────────────
foreach ($case in @(
    @{Codes=@(9); Expected='x64'}, @{Codes=@(12); Expected='arm64'},
    @{Codes=@(9,9); Expected='x64'}, @{Codes=@(12,12); Expected='arm64'}
)) {
    $actual = & $module { param($codes) Resolve-GilWindowsArchitecture -Codes $codes } $case.Codes
    if ($actual -ne $case.Expected) { throw 'Architecture selection regression' }
    $results.passed++
}
foreach ($case in @(@{Codes=@()}, @{Codes=@($null)}, @{Codes=@(0)}, @{Codes=@(5)}, @{Codes=@(9,12)}, @{Codes=@('unknown')})) {
    $refused = $false
    try { & $module { param($codes) Resolve-GilWindowsArchitecture -Codes $codes } $case.Codes | Out-Null } catch { $refused = $true }
    if (-not $refused) { throw 'Unsupported or ambiguous CPU accepted' }
    $results.passed++
}
function Sources([hashtable]$values) {
    $s = [ordered]@{}
    foreach ($k in 'cim','sysenv','cpuid') { $s[$k] = @{ value = $values[$k]; note = $(if ($values[$k]) { 'answered' } else { 'unavailable' }) } }
    $s
}
foreach ($case in @(
    @{S=@{cim='x64';sysenv='x64';cpuid='x64'}; Expected='x64'},
    @{S=@{cim=$null;sysenv='arm64';cpuid='arm64'}; Expected='arm64'},
    @{S=@{cim=$null;sysenv=$null;cpuid='x64'}; Expected='x64'},
    @{S=@{cim='arm64';sysenv=$null;cpuid=$null}; Expected='arm64'}
)) {
    $actual = & $module { param($s) Resolve-GilNativeArchitecture -Sources $s } (Sources $case.S)
    if ($actual -ne $case.Expected) { throw "Decision regression for $($case.S | ConvertTo-Json -Compress)" }
    $results.passed++
}
foreach ($case in @(
    @{S=@{cim=$null;sysenv=$null;cpuid=$null}; Needle='could not be detected'},
    @{S=@{cim='x64';sysenv='arm64';cpuid=$null}; Needle='inconsistent'},
    @{S=@{cim='x64';sysenv='x64';cpuid='arm64'}; Needle='inconsistent'}
)) {
    $message = $null
    try { & $module { param($s) Resolve-GilNativeArchitecture -Sources $s } (Sources $case.S) | Out-Null } catch { $message = $_.Exception.Message }
    if (-not $message -or $message -notmatch $case.Needle -or $message -notmatch 'ARM 기반 프로세서') { throw "Decision must refuse with the minimal question: $message" }
    $results.passed++
}

# ── 2. Live sources in this process, with each source pretended unavailable ────────────────
$target = Get-GilInstallTarget
if ($target.architecture -ne $native) { throw 'Native runner detection mismatch' }
if ($target.installation_started -ne $false) { throw 'Detection claimed installation' }
if (@($target.answered_by).Count -lt 2) { throw "Expected at least two live sources on the runner: $($target.sources -join ' ')" }
$results.live = @{ answered_by = @($target.answered_by); sources = @($target.sources); process = $target.process_architecture; emulated = $target.emulated }
$results.passed++
foreach ($skip in @(@('cim'), @('sysenv'), @('cpuid'), @('cim','sysenv'), @('cim','cpuid'), @('sysenv','cpuid'))) {
    $t = Get-GilInstallTarget -Skip $skip
    if ($t.architecture -ne $native) { throw "Detection without $($skip -join '+') mismatched" }
    if (@($t.answered_by | Where-Object { $skip -contains $_ }).Count) { throw 'A skipped source answered' }
    $results.passed++
}
$message = $null
try { Get-GilInstallTarget -Skip @('cim','sysenv','cpuid') | Out-Null } catch { $message = $_.Exception.Message }
if ($message -notmatch 'could not be detected' -or $message -notmatch 'ARM 기반 프로세서') { throw "All sources unavailable must ask, not guess: $message" }
$results.passed++

# ── 3. Hosts: native and emulated shells; fault preludes; module and guidance block ─────────
function Get-PeMachine([string]$path) {
    # Defined before use; unreadable or non-PE candidates throw and are skipped.
    $fs = [System.IO.File]::OpenRead($path)
    try {
        $r = New-Object System.IO.BinaryReader($fs)
        $fs.Seek(0x3C, 'Begin') | Out-Null; $pe = $r.ReadInt32()
        $fs.Seek($pe + 4, 'Begin') | Out-Null
        switch ($r.ReadUInt16()) { 0x14c { 'x86' } 0x8664 { 'x64' } 0xAA64 { 'arm64' } default { 'unknown' } }
    } finally { $fs.Dispose() }
}
$hosts = @()
foreach ($candidate in @(
    (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'),
    (Join-Path $env:SystemRoot 'SysWOW64\WindowsPowerShell\v1.0\powershell.exe'),
    (Join-Path $env:ProgramFiles 'PowerShell\7\pwsh.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'PowerShell\7\pwsh.exe')
) + @(Get-Command pwsh.exe, powershell.exe -All -ErrorAction SilentlyContinue | ForEach-Object { $_.Source })) {
    if ($candidate -and (Test-Path -LiteralPath $candidate) -and ($hosts -notcontains $candidate)) {
        try { Get-PeMachine $candidate | Out-Null; $hosts += $candidate } catch { Write-Output "skipping unreadable host $candidate" }
    }
}
$guide = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot 'DEV-CANDIDATE-CHECK.md')
if ($guide -notmatch '(?s)<!-- gil-cpu-detect:begin -->\s*```powershell\r?\n(.*?)```\s*<!-- gil-cpu-detect:end -->') { throw 'guidance block not found' }
$block = ($Matches[1] -split "`r?`n" | ForEach-Object { $_ -replace '^   ', '' }) -join "`n"
function Invoke-Host([string]$exe, [string]$script) {
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
    $out = & $exe -NoProfile -NonInteractive -EncodedCommand $encoded 2>&1 | ForEach-Object { "$_" }
    @{ exit = $LASTEXITCODE; lines = @($out) }
}
$preludes = [ordered]@{
    none = ''
    cim_refused = "function Get-CimInstance { throw '클라이언트에서 CIM 리소스에 대한 액세스를 사용할 수 없습니다.' }"
    cim_absent = "Import-Module Microsoft.PowerShell.Management, Microsoft.PowerShell.Utility -ErrorAction SilentlyContinue; `$PSModuleAutoLoadingPreference = 'None'; Remove-Module CimCmdlets -Force -ErrorAction SilentlyContinue; if (Get-Command Get-CimInstance -ErrorAction SilentlyContinue) { throw 'fixture: Get-CimInstance still present' }"
    registry_refused = "function Get-ItemProperty { throw 'Requested registry access is not allowed.' }"
    all_refused = "function Get-CimInstance { throw 'refused' }; function Get-ItemProperty { throw 'refused' }"
    contradiction = "function Get-CimInstance { throw 'refused' }; function Get-ItemProperty { param(`$Path, `$Name, `$ErrorAction) if (`$Name -eq 'PROCESSOR_ARCHITECTURE') { [pscustomobject]@{ PROCESSOR_ARCHITECTURE = 'ARM64' } } else { [pscustomobject]@{ Identifier = 'Intel64 Family 6 Model 1' } } }"
}
$moduleScript = "Import-Module '$modulePath' -Force; try { Get-GilInstallTarget | ConvertTo-Json -Compress } catch { 'REFUSED: ' + `$_.Exception.Message; exit 3 }"
foreach ($exe in $hosts) {
    $machine = Get-PeMachine $exe
    $results.hosts += @{ exe = ($exe -replace [regex]::Escape($env:SystemDrive), '<sys>'); machine = $machine }
    foreach ($name in $preludes.Keys) {
        $prelude = $preludes[$name]
        # Module
        $run = Invoke-Host $exe ($prelude + "`n" + $moduleScript)
        $last = $run.lines[-1]
        $case = @{ host = $machine; prelude = $name; kind = 'module'; last = $last }
        switch ($name) {
            { $_ -in 'all_refused', 'contradiction' } {
                $needle = $(if ($name -eq 'contradiction') { 'inconsistent' } else { 'could not be detected' })
                if ($run.exit -ne 3 -or $last -notmatch "^REFUSED: .*$needle" -or $last -notmatch 'ARM 기반 프로세서') { throw "[$machine/$name/module] expected refusal with the minimal question, got: $last" }
            }
            default {
                if ($run.exit -ne 0) { throw "[$machine/$name/module] exit $($run.exit): $($run.lines -join ' | ')" }
                $t = $last | ConvertFrom-Json
                if ($t.architecture -ne $native) { throw "[$machine/$name/module] architecture $($t.architecture) != $native" }
                if ($t.process_architecture -ne $machine) { throw "[$machine/$name/module] process view $($t.process_architecture) != host $machine" }
                if ([bool]$t.emulated -ne ($machine -ne $native)) { throw "[$machine/$name/module] emulated flag wrong" }
                if ($name -like 'cim_*' -and (@($t.answered_by) -contains 'cim')) { throw "[$machine/$name/module] cim must not answer" }
                if ($name -eq 'registry_refused' -and ((@($t.answered_by) -join ',') -ne 'cim')) { throw "[$machine/$name/module] only cim may answer" }
                $case.answered_by = @($t.answered_by)
            }
        }
        $results.cases += $case; $results.passed++
        # Guidance block
        $run = Invoke-Host $exe ($prelude + "`n" + $block)
        $last = $run.lines[-1]; $sourcesLine = @($run.lines | Where-Object { $_ -like 'sources: *' })[-1]
        $case = @{ host = $machine; prelude = $name; kind = 'guidance'; last = $last }
        switch ($name) {
            'all_refused'   { if ($last -ne 'UNDETERMINED -> stop and ask') { throw "[$machine/$name/guidance] $last" } }
            'contradiction' { if ($last -ne 'CONTRADICTION: arm64,x64 -> stop and ask') { throw "[$machine/$name/guidance] $last" } }
            default {
                if ($last -ne "NATIVE CPU: $native") { throw "[$machine/$name/guidance] $last | $sourcesLine" }
                if ($sourcesLine -notmatch '^sources: ' -or $sourcesLine -notmatch "PROCESSOR_ARCHITECTURE=") { throw "[$machine/$name/guidance] sources line missing: $sourcesLine" }
                if ($name -like 'cim_*' -and $sourcesLine -notmatch 'cim=n/a' ) { throw "[$machine/$name/guidance] cim must be n/a: $sourcesLine" }
                if ($name -eq 'registry_refused' -and $sourcesLine -notmatch 'sysenv=n/a cpuid=n/a') { throw "[$machine/$name/guidance] registry must be n/a: $sourcesLine" }
            }
        }
        $results.cases += $case; $results.passed++
    }
}
$machines = @($results.hosts | ForEach-Object { $_.machine } | Select-Object -Unique)
if ($machines -notcontains $native) { throw 'no native host was exercised' }
$results.emulated_hosts_covered = @($machines | Where-Object { $_ -ne $native })
if ($results.emulated_hosts_covered.Count -eq 0) { throw 'no emulated (non-native) host was exercised' }
New-Item -ItemType Directory -Force -Path 'target' | Out-Null
$results | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 'target/cpu-detect-results.json'
Write-Output "Installer target checks: $($results.passed) passed; native $native; hosts $(($results.hosts | ForEach-Object { $_.machine }) -join ','); emulated covered $($results.emulated_hosts_covered -join ',')"
Get-Content 'target/cpu-detect-results.json'
