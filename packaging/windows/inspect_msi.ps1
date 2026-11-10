param([Parameter(Mandatory = $true)][string]$Path)
$ErrorActionPreference = 'Stop'
$installer = New-Object -ComObject WindowsInstaller.Installer
$resolved = (Resolve-Path -LiteralPath $Path).ProviderPath
$database = $installer.OpenDatabase($resolved, 0)

function Read-Table([string]$Query, [string[]]$Columns) {
    $view = $database.OpenView($Query)
    try {
        $view.Execute()
        $rows = @()
        while ($record = $view.Fetch()) {
            if ($rows.Count -ge 256) {
                throw 'Unexpectedly large installer table'
            }
            $row = [ordered]@{}
            for ($column = 0; $column -lt $Columns.Count; $column++) {
                $row[$Columns[$column]] = $record.StringData($column + 1)
            }
            $rows += [pscustomobject]$row
        }
        return $rows
    } finally {
        $view.Close()
    }
}

$summary = $installer.SummaryInformation($resolved, 0)
$result = [ordered]@{
    template = $summary.Property(7)
    properties = @(Read-Table 'SELECT `Property`, `Value` FROM `Property`' @('name', 'value'))
    files = @(Read-Table 'SELECT `File`, `Component_`, `FileName`, `FileSize` FROM `File`' @('id', 'component', 'name', 'size'))
    directories = @(Read-Table 'SELECT `Directory`, `Directory_Parent`, `DefaultDir` FROM `Directory`' @('id', 'parent', 'name'))
    components = @(Read-Table 'SELECT `Component`, `Directory_`, `Attributes`, `KeyPath` FROM `Component`' @('id', 'directory', 'attributes', 'keypath'))
    shortcuts = @(Read-Table 'SELECT `Shortcut`, `Directory_`, `Name`, `Component_`, `Target`, `Icon_` FROM `Shortcut`' @('id', 'directory', 'name', 'component', 'target', 'icon'))
    registry = @(Read-Table 'SELECT `Registry`, `Root`, `Key`, `Name`, `Value`, `Component_` FROM `Registry`' @('id', 'root', 'key', 'name', 'value', 'component'))
    upgrades = @(Read-Table 'SELECT `UpgradeCode`, `VersionMin`, `VersionMax`, `Attributes`, `ActionProperty` FROM `Upgrade`' @('code', 'minimum', 'maximum', 'attributes', 'action'))
    icons = @(Read-Table 'SELECT `Name` FROM `Icon`' @('name'))
}
$tables = @(Read-Table 'SELECT `Name` FROM `_Tables`' @('name'))
$result.customActions = @()
if ($tables.name -contains 'CustomAction') {
    $result.customActions = @(Read-Table 'SELECT `Action`, `Type`, `Source`, `Target` FROM `CustomAction`' @('action', 'type', 'source', 'target'))
}
$result | ConvertTo-Json -Depth 8 -Compress
