$ErrorActionPreference = 'Stop'
$lock = $null
$exitCode = 1
$parentExitTimeoutMilliseconds = 300000
$stage = [IO.Path]::GetFullPath([string]$request.stage)
$msi = Join-Path $stage 'update.msi'
$installer = New-Object -ComObject WindowsInstaller.Installer
$upgradeCode = '{F78632A2-14B8-5F28-B29D-760BDED29A9A}'

function Read-Value($database, [string]$name) {
    if ($name -notmatch '^[A-Za-z]+$') { throw 'Invalid fixed MSI property' }
    $view = $database.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='$name'")
    try {
        $view.Execute() | Out-Null
        $record = $view.Fetch()
        if ($null -eq $record) { throw 'MSI property is missing' }
        return [string]$record.StringData(1)
    } finally {
        $view.Close() | Out-Null
    }
}

function Registered-Executable([string]$product) {
    if ($installer.ProductState($product) -ne 5) { throw 'MSI product is not installed' }
    if (@($installer.RelatedProducts($upgradeCode)) -notcontains $product) { throw 'MSI upgrade family differs' }
    $cached = $installer.ProductInfo($product, 'LocalPackage')
    $database = $installer.OpenDatabase($cached, 0)
    if ((Read-Value $database 'ProductCode') -ne $product) { throw 'Cached MSI product differs' }
    $view = $database.OpenView("SELECT ``ComponentId`` FROM ``Component`` WHERE ``Component``='DesktopApplication'")
    try {
        $view.Execute() | Out-Null
        $record = $view.Fetch()
        if ($null -eq $record) { throw 'Registered component is missing' }
        return [string]$installer.ComponentPath($product, $record.StringData(1))
    } finally {
        $view.Close() | Out-Null
    }
}

function Hash-Equals([string]$path, [string]$expected) {
    return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $expected
}

try {
    $parent = Get-Process -Id ([int]$request.parent)
    if ($parent.StartTime.ToUniversalTime().Ticks -ne $request.parent_start) { throw 'The parent process identity changed' }
    $registered = Registered-Executable ([string]$request.old_product)
    if ([IO.Path]::GetFullPath($registered) -ne [IO.Path]::GetFullPath([string]$request.executable)) { throw 'Registered destination changed' }
    $manifest = Join-Path (Split-Path -LiteralPath $registered) 'RELEASE.json'
    if (-not (Hash-Equals $registered $request.old_executable_hash) -or -not (Hash-Equals $manifest $request.old_manifest_hash)) { throw 'Installed source changed' }

    # Deny writes/deletion while this authenticated path is used by msiexec.
    $lock = New-Object IO.FileStream($msi, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = [BitConverter]::ToString($sha.ComputeHash($lock)).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
        $lock.Position = 0
    }
    if ($hash -ne $request.msi_hash) { throw 'Authenticated MSI changed before elevation' }
    $database = $installer.OpenDatabase($msi, 0)
    if ((Read-Value $database 'ProductCode') -ne $request.new_product -or
        (Read-Value $database 'UpgradeCode') -ne $upgradeCode -or
        (Read-Value $database 'ProductVersion') -ne $request.version -or
        (Read-Value $database 'ALLUSERS') -ne '1') { throw 'MSI identity changed' }
    [IO.File]::WriteAllText((Join-Path $stage 'READY'), [string]$request.msi_hash)
    if (-not $parent.WaitForExit($parentExitTimeoutMilliseconds)) { throw 'The document-safe shutdown did not finish' }

    $commit = Join-Path $stage 'COMMIT'
    if ((Test-Path -LiteralPath (Join-Path $stage 'ABORT')) -or
        -not (Test-Path -LiteralPath $commit) -or
        [IO.File]::ReadAllText($commit) -cne $request.msi_hash) { throw 'The update handoff was not committed' }

    if ((Registered-Executable ([string]$request.old_product)) -ne $registered -or
        -not (Hash-Equals $registered $request.old_executable_hash) -or
        -not (Hash-Equals $manifest $request.old_manifest_hash)) { throw 'Installed source changed during shutdown' }

    $arguments = @('/i', ('"' + $msi + '"'), '/qn', '/norestart', 'REBOOT=ReallySuppress',
        ('INSTALLFOLDER="' + (Split-Path -LiteralPath $registered) + '"'),
        '/l*v', ('"' + (Join-Path $stage 'install.log') + '"'))
    $process = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\msiexec.exe') `
        -ArgumentList $arguments -Wait -PassThru
    $exitCode = $process.ExitCode
    if ($exitCode -ne 0) { throw 'Windows Installer did not complete without a pending reboot' }
    if ((Registered-Executable ([string]$request.new_product)) -ne $registered -or
        -not (Hash-Equals $registered $request.executable_hash) -or
        -not (Hash-Equals $manifest $request.manifest_hash)) { throw 'Installed update did not match the prepared payload' }
    @{ state = 'installed'; product = $request.new_product; exit_code = 0 } |
        ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $stage 'RESULT.json') -Encoding UTF8
} catch {
    @{ state = 'installer-failed'; exit_code = $exitCode } |
        ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $stage 'FAILED.json') -Encoding UTF8
    exit 1
} finally {
    if ($null -ne $lock) { $lock.Dispose() }
}
