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
$format = 'tablecraft-native-text-v1'
$recipeSha256 = 'f8d3cf62e513d1973b9f64558a5e1d263066feeb32435cd0bbd79e4deab55685'
$utf8 = New-Object System.Text.UTF8Encoding($false)

function ConvertFrom-Units([string]$Hex) {
    if ($Hex -notmatch '^(?:[0-9a-f]{4})*$') { throw 'Invalid UTF-16 units' }
    $units = [char[]]::new($Hex.Length / 4)
    for ($i = 0; $i -lt $units.Length; $i++) {
        $units[$i] = [char][Convert]::ToUInt16($Hex.Substring(4 * $i, 4), 16)
    }
    return [string]::new($units)
}

function ConvertTo-Units([string]$Text) {
    return (($Text.ToCharArray() | ForEach-Object { '{0:x4}' -f [int]$_ }) -join '')
}

function ConvertTo-Cell($Field) {
    $value = $Field.Value
    if ($null -eq $value -or $value -is [DBNull]) { return @{ kind = 'null' } }
    switch ([int]$Field.Type) {
        1 { return @{ kind = 'boolean'; value = [bool]$value } }
        { $_ -in 2, 3, 4 } { return @{ kind = 'integer'; value = $value.ToString([Globalization.CultureInfo]::InvariantCulture) } }
        { $_ -in 10, 12 } { return @{ kind = 'text'; value = (ConvertTo-Units ([string]$value)) } }
        default { throw 'Unexpected native result carrier' }
    }
}

$root = (Resolve-Path -LiteralPath $Directory).Path
$requestPath = Join-Path $root 'request.json'
if ((Get-FileHash -LiteralPath $requestPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $recipeSha256) {
    throw 'Request does not match the original reviewed SQL recipe'
}
$request = [IO.File]::ReadAllText($requestPath) | ConvertFrom-Json
if ($request.format -ne $format -or $request.max_rows -ne 4096) { throw 'Unknown probe format/limits' }
if (@($request.tables).Count -ne 2 -or @($request.cases).Count -gt 128) { throw 'Invalid probe counts' }
$capturePath = Join-Path $root 'capture'
# Refuse reuse, including partial prior captures. Do not overwrite user evidence.
New-Item -Path $capturePath -ItemType Directory -ErrorAction Stop | Out-Null
$engine = $null
$db = $null
try {
    $engine = New-Object -ComObject DAO.DBEngine.120
    $comType = [type]::GetTypeFromProgID('DAO.DBEngine.120', $true)
    $key = [Microsoft.Win32.Registry]::ClassesRoot.OpenSubKey("CLSID\{$($comType.GUID)}\InprocServer32")
    if ($null -eq $key) { throw 'Cannot identify native ACE DLL build' }
    try { $dll = [Environment]::ExpandEnvironmentVariables(([string]$key.GetValue('')).Trim('"')) }
    finally { $key.Close() }
    $dllVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($dll).FileVersion
    if ([string]::IsNullOrEmpty($dllVersion)) { throw 'Missing native ACE DLL build' }
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
        databases = @()
    }
    $plainPath = Join-Path $capturePath 'plain.accdb'
    $db = $engine.CreateDatabase($plainPath, [string]$request.locale, $dbVersion120)
    foreach ($table in $request.tables) {
        if ($table.name -notin 'ShortValues', 'LongValues') { throw 'Unexpected table name' }
        $nativeType = if ($table.kind -eq 'short') { 'TEXT(255)' } else { 'MEMO' }
        $db.Execute("CREATE TABLE [$($table.name)] (ID LONG NOT NULL, V $nativeType)", $dbFailOnError)
        $db.TableDefs.Item($table.name).Fields.Item('V').AllowZeroLength = $true
        $records = $db.OpenRecordset([string]$table.name, $dbOpenDynaset)
        try {
            for ($id = 0; $id -lt $table.values.Count; $id++) {
                $records.AddNew()
                $records.Fields.Item('ID').Value = $id
                if ($null -ne $table.values[$id]) {
                    $records.Fields.Item('V').Value = ConvertFrom-Units $table.values[$id]
                }
                $records.Update()
            }
        } finally { $records.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($records) }
    }
    $db.Close()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
    $db = $null
    $indexedPath = Join-Path $capturePath 'indexed.accdb'
    [IO.File]::Copy($plainPath, $indexedPath, $false)
    $db = $engine.OpenDatabase($indexedPath, $false, $false)
    $db.Execute('CREATE INDEX VIndex ON ShortValues (V)', $dbFailOnError)
    $db.Close()
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
    $db = $null
    foreach ($phase in 'plain', 'indexed') {
        $path = Join-Path $capturePath ($phase + '.accdb')
        $before = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        $db = $engine.OpenDatabase($path, $false, $true)
        $orders = [ordered]@{}
        foreach ($table in $request.tables) {
            $orders[$table.name] = [int]$db.TableDefs.Item($table.name).Fields.Item('V').CollatingOrder
        }
        $captured = [ordered]@{
            id = $phase; database_collating_order = [int]$db.CollatingOrder
            field_collating_orders = $orders; results = @()
        }
        foreach ($case in $request.cases) {
            $query = $null
            $records = $null
            try {
                $query = $db.CreateQueryDef('', [string]$case.sql)
                if ($null -ne $case.PSObject.Properties['parameter']) {
                    $query.Parameters.Item('P').Value = ConvertFrom-Units $case.parameter
                }
                $records = $query.OpenRecordset($dbOpenSnapshot, $dbReadOnly)
                $columns = @($records.Fields | ForEach-Object { [string]$_.Name })
                $rows = [Collections.Generic.List[object]]::new()
                while (-not $records.EOF) {
                    if ($rows.Count -ge $request.max_rows) { throw 'Native probe row limit exceeded' }
                    $cells = @($records.Fields | ForEach-Object { ConvertTo-Cell $_ })
                    $rows.Add($cells)
                    $records.MoveNext()
                }
                $captured.results += @{ id = $case.id; status = 'ok'; columns = $columns; rows = @($rows.ToArray()) }
            } catch {
                # Query failures are observations, never passing compatibility.
                $cause = $_.Exception
                while ($null -ne $cause.InnerException) { $cause = $cause.InnerException }
                if ($cause -isnot [Runtime.InteropServices.COMException]) { throw }
                $captured.results += @{ id = $case.id; status = 'error'; hresult = [int]$cause.HResult; dao_numbers = @($engine.Errors | ForEach-Object { [int]$_.Number }) }
            } finally {
                if ($null -ne $records) { $records.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($records) }
                if ($null -ne $query) { $query.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($query) }
            }
        }
        $db.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
        $db = $null
        $after = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($before -ne $after) { throw 'Native read changed fixture bytes' }
        $bytes = [IO.File]::ReadAllBytes($path)
        $captured['bytes'] = $bytes.Length
        $captured['sha256'] = $after
        $captured['header_hex'] = (($bytes[0..127] | ForEach-Object { '{0:x2}' -f $_ }) -join '')
        $report.databases += $captured
    }
    [IO.File]::WriteAllText((Join-Path $capturePath 'results.json'), ($report | ConvertTo-Json -Depth 12), $utf8)
    Write-Output 'Captured native ACE observations. Run the portable checker; TableCraft comparison remains separate.'
} finally {
    if ($null -ne $db) { $db.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db) }
    if ($null -ne $engine) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($engine) }
}
