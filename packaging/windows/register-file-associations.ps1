<# Register an optional per-user handler. Does not change default associations. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Register', 'Unregister')]
    [string] $Action,
    [string] $Executable = (Join-Path $PSScriptRoot 'tablecraft.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This helper requires Windows.'
}
$Executable = [IO.Path]::GetFullPath($Executable)
if ($Executable.Contains('"') -or $Executable.Contains("`r") -or $Executable.Contains("`n")) {
    throw 'Invalid executable path.'
}
if ($Action -eq 'Register' -and -not [IO.File]::Exists($Executable)) {
    throw 'Executable must be an installed file.'
}

# Literal REG_SZ, with no shell or environment-variable expansion.
$Command = '"' + $Executable + '" open --wait -- "%1"'
$Records = @(
    @{ Path = 'Software\Classes\TableCraft.AccessDatabase'; Name = ''; Value = 'Access database (TableCraft Inspect)' },
    @{ Path = 'Software\Classes\TableCraft.AccessDatabase\shell\open\command'; Name = ''; Value = $Command },
    @{ Path = 'Software\Classes\.mdb\OpenWithProgids'; Name = 'TableCraft.AccessDatabase'; Value = '' },
    @{ Path = 'Software\Classes\.accdb\OpenWithProgids'; Name = 'TableCraft.AccessDatabase'; Value = '' },
    @{ Path = 'Software\TableCraft\Capabilities'; Name = 'ApplicationName'; Value = 'TableCraft Inspect' },
    @{ Path = 'Software\TableCraft\Capabilities'; Name = 'ApplicationDescription'; Value = 'Inspect Access databases without executing content' },
    @{ Path = 'Software\TableCraft\Capabilities\FileAssociations'; Name = '.mdb'; Value = 'TableCraft.AccessDatabase' },
    @{ Path = 'Software\TableCraft\Capabilities\FileAssociations'; Name = '.accdb'; Value = 'TableCraft.AccessDatabase' },
    @{ Path = 'Software\RegisteredApplications'; Name = 'TableCraft Inspect'; Value = 'Software\TableCraft\Capabilities' }
)
$ReceiptPath = 'Software\TableCraft\FileAssociations'
$Registry = [Microsoft.Win32.Registry]::CurrentUser
$ReceiptKey = $Registry.OpenSubKey($ReceiptPath)
$Owner = $null
try {
    if ($null -ne $ReceiptKey) {
        $Owner = $ReceiptKey.GetValue('Executable', $null)
    }
} finally {
    if ($null -ne $ReceiptKey) { $ReceiptKey.Dispose() }
}
if ($null -ne $Owner -and $Owner -cne $Executable) {
    throw 'Registration belongs to another installation.'
}

# Validate every owned value before mutating any key. Shared extension keys stay intact.
foreach ($Record in $Records) {
    $Key = $Registry.OpenSubKey($Record.Path)
    try {
        if ($null -ne $Key -and $Key.GetValueNames() -contains $Record.Name) {
            $Value = $Key.GetValue($Record.Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            if ($null -eq $Owner -or $Value -cne $Record.Value -or
                $Key.GetValueKind($Record.Name) -ne [Microsoft.Win32.RegistryValueKind]::String) {
                throw ('Refusing unowned or modified registry value: ' + $Record.Path + '\' + $Record.Name)
            }
        }
    } finally {
        if ($null -ne $Key) { $Key.Dispose() }
    }
}
if ($Action -eq 'Unregister' -and $null -eq $Owner) {
    Write-Output 'No TableCraft registration is owned by this installation.'
    return
}
if ($Action -eq 'Register') {
    # Retain ownership before writes so an interrupted registration is recoverable.
    $Key = $Registry.CreateSubKey($ReceiptPath)
    try { $Key.SetValue('Executable', $Executable, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $Key.Dispose() }
    foreach ($Record in $Records) {
        $Key = $Registry.CreateSubKey($Record.Path)
        try { $Key.SetValue($Record.Name, $Record.Value, [Microsoft.Win32.RegistryValueKind]::String) }
        finally { $Key.Dispose() }
    }
} else {
    foreach ($Record in $Records) {
        $Key = $Registry.OpenSubKey($Record.Path, $true)
        try {
            if ($null -ne $Key) { $Key.DeleteValue($Record.Name, $false) }
        } finally {
            if ($null -ne $Key) { $Key.Dispose() }
        }
    }
    $Key = $Registry.OpenSubKey($ReceiptPath, $true)
    try { $Key.DeleteValue('Executable', $false) }
    finally { $Key.Dispose() }
    # Remove only empty app-owned containers; shared extension keys stay intact.
    $Containers = @(
        'Software\Classes\TableCraft.AccessDatabase\shell\open\command',
        'Software\Classes\TableCraft.AccessDatabase\shell\open',
        'Software\Classes\TableCraft.AccessDatabase\shell',
        'Software\Classes\TableCraft.AccessDatabase',
        'Software\TableCraft\Capabilities\FileAssociations',
        'Software\TableCraft\Capabilities',
        $ReceiptPath
    )
    foreach ($Container in $Containers) {
        $Key = $Registry.OpenSubKey($Container)
        $Empty = $false
        try {
            $Empty = $null -ne $Key -and $Key.ValueCount -eq 0 -and $Key.SubKeyCount -eq 0
        } finally {
            if ($null -ne $Key) { $Key.Dispose() }
        }
        if ($Empty) { $Registry.DeleteSubKey($Container, $false) }
    }
}
if (-not ('TableCraftAssociationNotification' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TableCraftAssociationNotification {
    [DllImport("shell32.dll")]
    public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr first, IntPtr second);
}
'@
}
[TableCraftAssociationNotification]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
Write-Output 'TableCraft file-handler registration updated; defaults were not changed.'
