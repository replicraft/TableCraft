$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
$installer = New-Object -ComObject WindowsInstaller.Installer
$upgradeCode = '{F78632A2-14B8-5F28-B29D-760BDED29A9A}'
$maximumTableRows = 256
$maximumFieldCharacters = 16384
$maximumRelatedProducts = 16
$maximumCabinetBytes = 536870912
$streamChunkBytes = 65536

function Read-Rows($database, [string]$query, [string[]]$columns) {
    $view = $database.OpenView($query)
    try {
        $view.Execute() | Out-Null
        $rows = @()
        while ($record = $view.Fetch()) {
            if ($rows.Count -ge $maximumTableRows) { throw 'MSI table exceeds its bound' }
            $row = [ordered]@{}
            for ($index = 0; $index -lt $columns.Count; $index++) {
                $value = $record.StringData($index + 1)
                if ($value.Length -gt $maximumFieldCharacters) { throw 'MSI field exceeds its bound' }
                $row[$columns[$index]] = $value
            }
            $rows += [pscustomobject]$row
        }
        return $rows
    } finally {
        $view.Close() | Out-Null
    }
}

function Inspect-Package([string]$path) {
    $database = $installer.OpenDatabase($path, 0)
    $summary = $installer.SummaryInformation($path, 0)
    $tables = @(Read-Rows $database 'SELECT `Name` FROM `_Tables`' @('name'))
    $actions = @()
    if ($tables.name -contains 'CustomAction') {
        $actions = @(Read-Rows $database 'SELECT `Action` FROM `CustomAction`' @('name'))
    }
    return @{
        template = $summary.Property(7)
        properties = @(Read-Rows $database 'SELECT `Property`, `Value` FROM `Property`' @('name', 'value'))
        files = @(Read-Rows $database 'SELECT `File`, `Component_`, `FileName`, `FileSize`, `Sequence` FROM `File`' @('id', 'component', 'name', 'size', 'sequence'))
        components = @(Read-Rows $database 'SELECT `Component`, `ComponentId`, `Directory_`, `Attributes`, `KeyPath` FROM `Component`' @('id', 'guid', 'directory', 'attributes', 'keypath'))
        upgrades = @(Read-Rows $database 'SELECT `UpgradeCode`, `ActionProperty` FROM `Upgrade`' @('code', 'action'))
        custom_actions = $actions
        storages = @(Read-Rows $database 'SELECT `Name` FROM `_Storages`' @('name'))
        media = @(Read-Rows $database 'SELECT `DiskId`, `LastSequence`, `Cabinet` FROM `Media`' @('disk', 'last_sequence', 'cabinet'))
    }
}

if ($request.operation -eq 'hint') {
    $registered = $false
    $count = 0
    foreach ($product in $installer.RelatedProducts($upgradeCode)) {
        $count++
        if ($count -gt $maximumRelatedProducts) { throw 'Too many related products' }
        if ($installer.ProductState($product) -ne 5) { continue }
        $cached = $installer.ProductInfo($product, 'LocalPackage')
        $database = $installer.OpenDatabase($cached, 0)
        $components = @(Read-Rows $database "SELECT ``ComponentId`` FROM ``Component`` WHERE ``Component``='DesktopApplication'" @('guid'))
        if ($components.Count -ne 1) { throw 'Registered MSI component is ambiguous' }
        $path = [string]$installer.ComponentPath($product, $components[0].guid)
        if ([IO.Path]::GetFullPath($path) -eq [IO.Path]::GetFullPath([string]$request.executable)) { $registered = $true }
    }
    @{ registered = $registered } | ConvertTo-Json -Compress
} elseif ($request.operation -eq 'detect') {
    $matches = @()
    foreach ($product in $installer.RelatedProducts($upgradeCode)) {
        if ($matches.Count -ge $maximumRelatedProducts) { throw 'Too many related MSI products' }
        if ($installer.ProductState($product) -ne 5) { continue }
        $cached = $installer.ProductInfo($product, 'LocalPackage')
        $report = Inspect-Package $cached
        $component = @($report.components | Where-Object { $_.id -eq 'DesktopApplication' })
        if ($component.Count -ne 1) { throw 'Installed component ownership is ambiguous' }
        $executable = $installer.ComponentPath($product, $component[0].guid)
        $matches += @{
            product = $product
            executable = $executable
            cached = $cached
            transforms = [string]$installer.ProductInfo($product, 'Transforms')
            package = $report
        }
    }
    @{ products = $matches } | ConvertTo-Json -Depth 10 -Compress
} elseif ($request.operation -eq 'inspect') {
    Inspect-Package ([string]$request.path) | ConvertTo-Json -Depth 10 -Compress
} elseif ($request.operation -eq 'cabinet') {
    $database = $installer.OpenDatabase([string]$request.path, 0)
    $media = @(Read-Rows $database 'SELECT `Cabinet` FROM `Media`' @('name'))
    if ($media.Count -ne 1 -or $media[0].name -notmatch '^#[A-Za-z0-9_.-]{1,127}$') { throw 'External or multiple cabinets are unsupported' }
    $name = $media[0].name.Substring(1)
    $view = $database.OpenView("SELECT ``Data`` FROM ``_Streams`` WHERE ``Name``='$name'")
    $output = $null
    try {
        $view.Execute() | Out-Null
        $record = $view.Fetch()
        if ($null -eq $record) { throw 'Embedded cabinet is missing' }
        $output = New-Object IO.FileStream([string]$request.destination, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
        $encoding = [Text.Encoding]::GetEncoding('iso-8859-1')
        $total = 0
        while ($true) {
            $text = [string]$record.ReadStream(1, $streamChunkBytes, 1)
            if ($text.Length -eq 0) { break }
            $bytes = $encoding.GetBytes($text)
            $total += $bytes.Length
            if ($total -gt $maximumCabinetBytes) { throw 'Embedded cabinet exceeds its bound' }
            $output.Write($bytes, 0, $bytes.Length)
        }
        $output.Flush($true)
        @{ cabinet_bytes = $total } | ConvertTo-Json -Compress
    } finally {
        if ($null -ne $output) { $output.Dispose() }
        $view.Close() | Out-Null
    }
} else {
    throw 'Unknown MSI operation' 
}
