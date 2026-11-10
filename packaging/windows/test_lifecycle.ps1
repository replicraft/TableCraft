param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Target,
    [string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path -LiteralPath $Installer).ProviderPath
$msiexec = Join-Path $env:SystemRoot 'System32\msiexec.exe'
$shortcut = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'TableCraft.lnk'
$existing = @(
    Get-ItemProperty 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -eq 'TableCraft' }
)
if ($existing.Count -ne 0 -or (Test-Path -LiteralPath $shortcut)) {
    throw 'Installer lifecycle checks require a clean disposable Windows runner'
}
foreach ($extension in @('mdb', 'accdb')) {
    if (Test-Path -LiteralPath "HKLM:\Software\Classes\TableCraft.Database.$extension") {
        throw 'Existing TableCraft registration must not be changed by a lifecycle test'
    }
}

function Extension-Snapshot([string]$Extension) {
    $path = "HKLM:\Software\Classes\.$Extension"
    $key = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
    $default = if ($null -eq $key) { $null } else { $key.GetValue('') }
    $openWith = Get-Item -LiteralPath "$path\OpenWithProgids" -ErrorAction SilentlyContinue
    $other = [ordered]@{}
    if ($null -ne $openWith) {
        foreach ($name in ($openWith.GetValueNames() | Sort-Object)) {
            if ($name -ne "TableCraft.Database.$Extension") {
                $other[$name] = $openWith.GetValue($name)
            }
        }
    }
    return @{ Default = $default; Other = ($other | ConvertTo-Json -Compress) }
}

$before = @{}
foreach ($extension in @('mdb', 'accdb')) {
    $before[$extension] = Extension-Snapshot $extension
}
$temporary = if ($EvidenceDirectory) {
    [IO.Path]::GetFullPath($EvidenceDirectory)
} else {
    Join-Path ([IO.Path]::GetTempPath()) ('tablecraft-msi-lifecycle-' + [guid]::NewGuid())
}
if (Test-Path -LiteralPath $temporary) {
    throw 'Lifecycle evidence directory must be new and test-owned'
}
$install = Join-Path $temporary 'Installed TableCraft'
New-Item -ItemType Directory -Path $temporary | Out-Null
$installed = $false
$attempted = $false

function Run-Msi([string[]]$Arguments, [int[]]$AllowedExitCodes = @(0)) {
    $process = Start-Process -FilePath $msiexec -ArgumentList $Arguments -Wait -PassThru
    if ($AllowedExitCodes -notcontains $process.ExitCode) {
        throw "Windows Installer returned $($process.ExitCode); inspect lifecycle logs at $temporary"
    }
}

try {
    $attempted = $true
    Run-Msi @('/i', ('"' + $installerPath + '"'), '/qn', '/norestart',
              ('INSTALLFOLDER="' + $install + '"'), '/l*v', ('"' + (Join-Path $temporary 'install.log') + '"'))
    $installed = $true
    $manifestPath = Join-Path $install 'RELEASE.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.product -ne 'tablecraft-desktop' -or $manifest.version -ne $Version -or $manifest.target -ne $Target) {
        throw 'Installed manifest identity differs from the requested release'
    }
    foreach ($property in $manifest.files.PSObject.Properties) {
        $file = Join-Path $install $property.Name
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $property.Value) {
            throw 'Installed payload hash differs from its release manifest'
        }
    }
    $executable = Join-Path $install 'tablecraft-desktop.exe'
    $reported = & $executable --version
    if ($LASTEXITCODE -ne 0 -or $reported -ne "tablecraft-desktop $Version") {
        throw 'Installed executable failed its headless version check'
    }
    foreach ($extension in @('mdb', 'accdb')) {
        $progid = "TableCraft.Database.$extension"
        $command = (Get-Item -LiteralPath "HKLM:\Software\Classes\$progid\shell\open\command").GetValue('')
        if ($command -ne ('"' + $executable + '" -- "%1"')) {
            throw 'Installed database handler does not preserve one literal filename'
        }
        $openWith = Get-Item -LiteralPath "HKLM:\Software\Classes\.$extension\OpenWithProgids"
        if ($openWith.GetValueNames() -notcontains $progid) {
            throw 'Installed Open With registration is missing'
        }
        $after = Extension-Snapshot $extension
        if ($after.Default -ne $before[$extension].Default -or $after.Other -ne $before[$extension].Other) {
            throw 'Installation changed an existing default or another application registration'
        }
    }
    if (-not (Test-Path -LiteralPath $shortcut)) {
        throw 'Installed Start Menu shortcut is missing'
    }
    $shell = New-Object -ComObject WScript.Shell
    if ($shell.CreateShortcut($shortcut).TargetPath -ne $executable) {
        throw 'Installed shortcut targets another executable'
    }
    $privateDatabase = Join-Path $install 'UserData.accdb'
    [IO.File]::WriteAllText($privateDatabase, 'User-owned database sentinel; never remove with the application')
    Run-Msi @('/x', ('"' + $installerPath + '"'), '/qn', '/norestart',
              '/l*v', ('"' + (Join-Path $temporary 'uninstall.log') + '"'))
    $installed = $false
    $attempted = $false
    foreach ($name in @('tablecraft-desktop.exe', 'LICENSE', 'ThirdPartyLicenses.txt', 'RELEASE.json')) {
        if (Test-Path -LiteralPath (Join-Path $install $name)) {
            throw 'Uninstallation left an application-owned payload file'
        }
    }
    if (-not (Test-Path -LiteralPath $privateDatabase)) {
        throw 'Uninstallation removed a user-owned database'
    }
    if (Test-Path -LiteralPath $shortcut) {
        throw 'Uninstallation left the owned Start Menu shortcut'
    }
    foreach ($extension in @('mdb', 'accdb')) {
        $progid = "TableCraft.Database.$extension"
        if (Test-Path -LiteralPath "HKLM:\Software\Classes\$progid\shell\open\command") {
            throw 'Uninstallation left the owned open command'
        }
        $openWith = Get-Item -LiteralPath "HKLM:\Software\Classes\.$extension\OpenWithProgids" -ErrorAction SilentlyContinue
        if ($null -ne $openWith -and $openWith.GetValueNames() -contains $progid) {
            throw 'Uninstallation left the owned Open With registration'
        }
        $after = Extension-Snapshot $extension
        if ($after.Default -ne $before[$extension].Default -or $after.Other -ne $before[$extension].Other) {
            throw 'Uninstallation changed another application registration'
        }
    }
    Write-Output 'TableCraft MSI install/uninstall, payload and handler checks passed without opening the GUI.'
} finally {
    if ($installed -or $attempted) {
        Run-Msi -Arguments @('/x', ('"' + $installerPath + '"'), '/qn', '/norestart',
                  '/l*v', ('"' + (Join-Path $temporary 'cleanup.log') + '"')) -AllowedExitCodes @(0, 1605)
    }
    # Retain logs for CI artifacts, including on failed checks. All files are
    # test-owned; the ordinary application uninstall never recursively removes data.
    Write-Output "Lifecycle evidence directory: $temporary"
}
