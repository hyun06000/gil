# Read-only installer preflight. Never select from the shell process architecture:
# an x64 shell can run under emulation on an ARM64 OS.
Set-StrictMode -Version Latest

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

function Get-GilInstallTarget {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'GIL: this preflight is for Windows only. Nothing was installed.'
    }
    try {
        $processors = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop)
        $codes = @($processors | ForEach-Object { $_.Architecture })
    } catch {
        # Do not expose machine/user details from the underlying query.
        throw 'GIL: Windows CPU information could not be read. Nothing was installed.'
    }
    $arch = Resolve-GilWindowsArchitecture -Codes $codes
    [pscustomobject]@{
        platform = 'windows'
        architecture = $arch
        marketplace = "gil-preview-windows-$arch"
        plugin = 'gil-companion-prototype'
        # Detection is not release clearance or installation consent.
        installation_started = $false
    }
}

Export-ModuleMember -Function Get-GilInstallTarget
