# Read-only installer preflight. Never select from the shell process architecture:
# an x86 or x64 shell can run under emulation on an ARM64 OS.
#
# The native OS architecture is read from up to three independent read-only sources, none of
# which needs administrator rights, a setting change or a working CIM/WMI client:
#   cim    - Win32_Processor.Architecture (9 = x64, 12 = ARM64); may be refused by CIM policy
#   sysenv - HKLM\SYSTEM\...\Session Manager\Environment\PROCESSOR_ARCHITECTURE (the machine
#            value; not WOW64-redirected; readable by standard users)
#   cpuid  - HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0\Identifier (kernel-populated;
#            only its family prefix is used, the string itself is never returned)
# One or more agreeing sources decide. Disagreement or no source stops with a minimal question
# for the person; nothing is ever guessed from the process view.
Set-StrictMode -Version Latest

$script:MinimalQuestion = 'Windows 설정 > 시스템 > 정보의 "시스템 종류"에 "ARM 기반 프로세서"라고 적혀 있는지만 알려 주세요.'

function Resolve-GilWindowsArchitecture {
    param([object[]]$Codes)
    if ($null -eq $Codes -or $Codes.Count -eq 0) {
        throw 'GIL: CPU type could not be detected. Nothing was installed.'
    }
    $values = @($Codes | ForEach-Object {
        if ($null -eq $_ -or "$_" -notmatch '^(9|12)$') {
            throw 'GIL: this CPU type is not supported. Nothing was installed.'
        }
        [int]$_
    } | Select-Object -Unique)
    if ($values.Count -ne 1) {
        throw 'GIL: CPU information is inconsistent. Nothing was installed.'
    }
    if ($values[0] -eq 12) { return 'arm64' }
    return 'x64'
}

# Each source answers 'x64', 'arm64' or $null plus a generic note (no machine details).
function Get-GilCpuSources {
    param([string[]]$Skip = @())
    $sources = [ordered]@{}
    if ($Skip -contains 'cim') {
        $sources.cim = @{ value = $null; note = 'skipped' }
    } else {
        try {
            $codes = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | ForEach-Object { $_.Architecture })
            $sources.cim = @{ value = (Resolve-GilWindowsArchitecture -Codes $codes); note = 'answered' }
        } catch {
            # Refused CIM access, a missing cmdlet and an unsupported code all count as no answer.
            $sources.cim = @{ value = $null; note = 'unavailable' }
        }
    }
    if ($Skip -contains 'sysenv') {
        $sources.sysenv = @{ value = $null; note = 'skipped' }
    } else {
        try {
            $machine = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name PROCESSOR_ARCHITECTURE -ErrorAction Stop).PROCESSOR_ARCHITECTURE
            $value = switch ("$machine") { 'AMD64' { 'x64' } 'ARM64' { 'arm64' } default { $null } }
            $sources.sysenv = @{ value = $value; note = $(if ($value) { 'answered' } else { 'unsupported value' }) }
        } catch {
            $sources.sysenv = @{ value = $null; note = 'unavailable' }
        }
    }
    if ($Skip -contains 'cpuid') {
        $sources.cpuid = @{ value = $null; note = 'skipped' }
    } else {
        try {
            $identifier = (Get-ItemProperty -Path 'HKLM:\HARDWARE\DESCRIPTION\System\CentralProcessor\0' -Name Identifier -ErrorAction Stop).Identifier
            $value = if ("$identifier" -match '^ARM') { 'arm64' } elseif ("$identifier" -match '^(Intel64|AMD64|EM64T)') { 'x64' } else { $null }
            $sources.cpuid = @{ value = $value; note = $(if ($value) { 'answered' } else { 'unsupported value' }) }
        } catch {
            $sources.cpuid = @{ value = $null; note = 'unavailable' }
        }
    }
    return $sources
}

# Decide from the sources only. Disagreement and silence are refusals, never guesses.
function Resolve-GilNativeArchitecture {
    param([System.Collections.IDictionary]$Sources)
    $answers = @($Sources.Keys | ForEach-Object { $Sources[$_].value } | Where-Object { $_ } | Select-Object -Unique)
    if ($answers.Count -gt 1) {
        throw "GIL: CPU information is inconsistent ($($answers -join ', ')). Nothing was installed. $script:MinimalQuestion"
    }
    if ($answers.Count -eq 0) {
        throw "GIL: CPU type could not be detected without administrator rights. Nothing was installed. $script:MinimalQuestion"
    }
    return $answers[0]
}

function Get-GilInstallTarget {
    # -Skip is fault injection for tests only: pretend the named sources are unavailable.
    param([string[]]$Skip = @())
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'GIL: this preflight is for Windows only. Nothing was installed.'
    }
    $sources = Get-GilCpuSources -Skip $Skip
    $arch = Resolve-GilNativeArchitecture -Sources $sources
    # The process view is reported, never used to decide: PROCESSOR_ARCHITECTURE describes this
    # shell (x86 or AMD64 under emulation on ARM64), not the machine.
    $process = switch ("$env:PROCESSOR_ARCHITECTURE") { 'AMD64' { 'x64' } 'ARM64' { 'arm64' } 'x86' { 'x86' } default { 'unknown' } }
    [pscustomobject]@{
        platform = 'windows'
        architecture = $arch
        answered_by = @($sources.Keys | Where-Object { $sources[$_].value })
        sources = @($sources.Keys | ForEach-Object { "$($_)=$($sources[$_].note)" })
        process_architecture = $process
        emulated = ($process -ne $arch)
        marketplace = "gil-preview-windows-$arch"
        plugin = 'gil-companion-prototype'
        # Detection is not release clearance or installation consent.
        installation_started = $false
    }
}

Export-ModuleMember -Function Get-GilInstallTarget
