# Original engineering recipe. Requires Windows and a matching-bitness native
# ACE DAO installation. Never opens an existing application/database.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Directory)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$dbVersion120 = 128
$dbOpenDynaset = 2
$dbOpenSnapshot = 4
$dbReadOnly = 4
$dbFailOnError = 128
$daoByte = 2
$daoInteger = 3
$daoLong = 4
$daoCurrency = 5
$daoSingle = 6
$daoDouble = 7
$daoBigInt = 16
$daoNumeric = 19
$daoDecimal = 20
$maxRows = 1
$expectedInputs = 4
$expectedCases = 100
$headerPrefixBytes = 128
$format = 'tablecraft-native-aggregates-v1'
$recipeSha256 = '2744eab6f5d791fbf59ccaee52b64a00d35e39e88c3f34e97044f77c9fbbe15e'
$utf8 = New-Object System.Text.UTF8Encoding($false)

function ConvertTo-Cell($Field) {
    $value = $Field.Value
    if ($null -eq $value -or $value -is [DBNull]) {
        return @{ kind = 'null' }
    }

    $culture = [Globalization.CultureInfo]::InvariantCulture
    switch ([int]$Field.Type) {
        { $_ -in $daoByte, $daoInteger, $daoLong, $daoBigInt } {
            return @{
                kind = 'integer'
                value = $value.ToString($culture)
            }
        }
        { $_ -eq $daoSingle } {
            $bits = [BitConverter]::ToUInt32([BitConverter]::GetBytes([single]$value), 0)
            return @{
                kind = 'single'
                value = $bits.ToString('x8', $culture)
            }
        }
        { $_ -eq $daoDouble } {
            $bits = [BitConverter]::ToUInt64([BitConverter]::GetBytes([double]$value), 0)
            return @{
                kind = 'double'
                value = $bits.ToString('x16', $culture)
            }
        }
        { $_ -in $daoCurrency, $daoNumeric, $daoDecimal } {
            $bits = [decimal]::GetBits([decimal]$value)
            $hex = (($bits | ForEach-Object { $_.ToString('x8', $culture) }) -join '')
            return @{
                kind = 'decimal'
                value = $hex
            }
        }
        default {
            throw 'Unexpected native aggregate result carrier'
        }
    }
}

$root = (Resolve-Path -LiteralPath $Directory).Path
$requestPath = Join-Path $root 'request.json'
if ((Get-FileHash -LiteralPath $requestPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $recipeSha256) {
    throw 'Request does not match the original reviewed SQL recipe'
}
$request = [IO.File]::ReadAllText($requestPath) | ConvertFrom-Json
if ($request.format -ne $format -or $request.max_rows -ne $maxRows) {
    throw 'Unknown probe format/limits'
}
if (@($request.values).Count -ne $expectedInputs -or @($request.cases).Count -ne $expectedCases) {
    throw 'Invalid probe counts'
}

$capturePath = Join-Path $root 'capture'
# Refuse reuse, including partial prior captures. Do not overwrite user evidence.
New-Item -Path $capturePath -ItemType Directory -ErrorAction Stop | Out-Null
$engine = $null
$db = $null
try {
    $engine = New-Object -ComObject DAO.DBEngine.120
    $comType = [type]::GetTypeFromProgID('DAO.DBEngine.120', $true)
    $key = [Microsoft.Win32.Registry]::ClassesRoot.OpenSubKey("CLSID\{$($comType.GUID)}\InprocServer32")
    if ($null -eq $key) {
        throw 'Cannot identify native ACE DLL build'
    }
    try {
        $dll = [Environment]::ExpandEnvironmentVariables(([string]$key.GetValue('')).Trim('"'))
    } finally {
        $key.Close()
    }
    $dllVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($dll).FileVersion
    if ([string]::IsNullOrEmpty($dllVersion)) {
        throw 'Missing native ACE DLL build'
    }

    $windows = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $report = [ordered]@{
        format = $format
        request_sha256 = (Get-FileHash -LiteralPath $requestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        runner_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
        environment = [ordered]@{
            engine_version = [string]$engine.Version
            engine_dll_version = $dllVersion
            windows_version = ([string]$windows.CurrentBuildNumber) + '.' + ([string]$windows.UBR)
            powershell_version = $PSVersionTable.PSVersion.ToString()
            process_bits = [IntPtr]::Size * 8
        }
        database = $null
        results = @()
    }
    $path = Join-Path $capturePath 'aggregates.accdb'
    $db = $engine.CreateDatabase($path, [string]$request.locale, $dbVersion120)
    $db.Execute('CREATE TABLE ProbeData (ID LONG NOT NULL, V LONG)', $dbFailOnError)
    $records = $db.OpenRecordset('ProbeData', $dbOpenDynaset)
    try {
        for ($id = 0; $id -lt $request.values.Count; $id++) {
            $records.AddNew()
            $records.Fields.Item('ID').Value = $id
            if ($null -ne $request.values[$id]) {
                $records.Fields.Item('V').Value = [int]$request.values[$id]
            }
            $records.Update()
        }
    } finally {
        $records.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($records)
    }
    $db.Close()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
    $db = $null
    $before = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    $db = $engine.OpenDatabase($path, $false, $true)
    foreach ($case in $request.cases) {
        $query = $null
        $records = $null
        try {
            $query = $db.CreateQueryDef('', [string]$case.sql)
            $records = $query.OpenRecordset($dbOpenSnapshot, $dbReadOnly)
            $fields = @($records.Fields | ForEach-Object {
                @{
                    name = [string]$_.Name
                    dao_type = [int]$_.Type
                    size = [int]$_.Size
                }
            })
            $rows = [Collections.Generic.List[object]]::new()
            while (-not $records.EOF) {
                if ($rows.Count -ge $request.max_rows) {
                    throw 'Native probe row limit exceeded'
                }
                $cells = @($records.Fields | ForEach-Object { ConvertTo-Cell $_ })
                $rows.Add($cells)
                $records.MoveNext()
            }
            $report.results += @{
                id = $case.id
                status = 'ok'
                fields = $fields
                rows = @($rows.ToArray())
            }
        } catch {
            # Query failures are observations, never passing compatibility.
            $cause = $_.Exception
            while ($null -ne $cause.InnerException) {
                $cause = $cause.InnerException
            }
            if ($cause -isnot [Runtime.InteropServices.COMException]) {
                throw
            }
            $report.results += @{
                id = $case.id
                status = 'error'
                hresult = [int]$cause.HResult
                dao_numbers = @($engine.Errors | ForEach-Object { [int]$_.Number })
            }
        } finally {
            if ($null -ne $records) {
                $records.Close()
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($records)
            }
            if ($null -ne $query) {
                $query.Close()
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($query)
            }
        }
    }
    $db.Close()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
    $db = $null
    $after = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($before -ne $after) { throw 'Native read changed fixture bytes' }
    $bytes = [IO.File]::ReadAllBytes($path)
    $report.database = @{
        bytes = $bytes.Length
        sha256 = $after
        header_hex = (($bytes[0..($headerPrefixBytes - 1)] | ForEach-Object { '{0:x2}' -f $_ }) -join '')
    }
    [IO.File]::WriteAllText((Join-Path $capturePath 'results.json'), ($report | ConvertTo-Json -Depth 12), $utf8)
    Write-Output 'Captured native ACE aggregate observations. Run the portable checker; TableCraft comparison remains separate.'
} finally {
    if ($null -ne $db) {
        $db.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
    }
    if ($null -ne $engine) {
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($engine)
    }
}
