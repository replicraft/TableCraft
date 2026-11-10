param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$UpgradeInstaller,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$UpgradeVersion,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The native MSI upgrade fixture requires a disposable elevated Windows runner'
}
$windowsInstaller = New-Object -ComObject WindowsInstaller.Installer
$upgradeCode = '{F78632A2-14B8-5F28-B29D-760BDED29A9A}'
if (@($windowsInstaller.RelatedProducts($upgradeCode)).Count -ne 0) {
    throw 'A pre-existing TableCraft installation must never be changed by this fixture'
}
$original = (Resolve-Path -LiteralPath $Installer).ProviderPath
$upgrade = (Resolve-Path -LiteralPath $UpgradeInstaller).ProviderPath
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
if (Test-Path -LiteralPath $evidence) { throw 'Upgrade evidence directory must be new and test-owned' }
New-Item -ItemType Directory -Path $evidence | Out-Null
$fixtureReceiptPath = Join-Path (Split-Path -LiteralPath $upgrade) 'TEST-ONLY.json'
$fixtureReceipt = Get-Content -LiteralPath $fixtureReceiptPath -Raw | ConvertFrom-Json
if ($fixtureReceipt.fixture_only -ne $true -or $fixtureReceipt.release_asset -ne $false -or
    $fixtureReceipt.binary_workspace_version -ne $Version -or $fixtureReceipt.declared_fixture_version -ne $UpgradeVersion -or
    (Get-FileHash -LiteralPath $upgrade -Algorithm SHA256).Hash.ToLowerInvariant() -ne $fixtureReceipt.msi_sha256) {
    throw 'Disposable upgrade MSI lacks the exact TEST-ONLY fixture identity'
}
Copy-Item -LiteralPath $fixtureReceiptPath -Destination (Join-Path $evidence 'TEST-ONLY.fixture.json')
$msiexec = Join-Path $env:SystemRoot 'System32\msiexec.exe'
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$installSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../crates/platform/tablecraft-update/src/msi/install.ps1') -Raw
$installedDirectory = Join-Path $evidence 'Installed TableCraft'
$executable = Join-Path $installedDirectory 'tablecraft-desktop.exe'
$manifestPath = Join-Path $installedDirectory 'RELEASE.json'
$database = Join-Path $installedDirectory 'UserData.accdb'
$sentinel = 'A first-party user-owned database sentinel, never opened by the fixture.'
$activeParent = $null
$activeHelper = $null
$maximumWaitMilliseconds = 120000

function Run-Msi([string[]]$Arguments, [int[]]$Allowed = @(0)) {
    $process = Start-Process -FilePath $msiexec -ArgumentList $Arguments -Wait -PassThru
    if ($Allowed -notcontains $process.ExitCode) { throw "Native fixture msiexec returned $($process.ExitCode)" }
}

function Product-Code([string]$Path) {
    $database = $windowsInstaller.OpenDatabase($Path, 0)
    $view = $database.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductCode'")
    try {
        $view.Execute() | Out-Null
        return [string]$view.Fetch().StringData(1)
    } finally {
        $view.Close() | Out-Null
    }
}

function Hash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-Original {
    if ($windowsInstaller.ProductState($oldProduct) -ne 5 -or $windowsInstaller.ProductState($newProduct) -eq 5) {
        throw 'A refused or rolled-back helper changed native product registration'
    }
    foreach ($name in $baseline.Keys) {
        if ((Hash (Join-Path $installedDirectory $name)) -ne $baseline[$name]) {
            throw 'A refused or rolled-back helper changed an original payload file'
        }
    }
    if ([IO.File]::ReadAllText($database) -cne $sentinel) { throw 'A helper changed user-owned database bytes' }
}

function Wait-ForReady($Helper, [string]$Stage, [string]$Digest) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($maximumWaitMilliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $ready = Join-Path $Stage 'READY'
        if ((Test-Path -LiteralPath $ready) -and [IO.File]::ReadAllText($ready) -ceq $Digest) { return }
        if ($Helper.HasExited -or (Test-Path -LiteralPath (Join-Path $Stage 'FAILED.json'))) {
            throw 'The fixed installer helper failed before READY; inspect retained stage evidence'
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'The fixed installer helper did not reach READY'
}

function Start-Helper([string]$Name, [string]$Package, [bool]$StaleParent = $false) {
    $stage = Join-Path $evidence $Name
    New-Item -ItemType Directory -Path $stage | Out-Null
    $packagePath = Join-Path $stage 'update.msi'
    Copy-Item -LiteralPath $Package -Destination $packagePath
    $parent = Start-Process -FilePath $powershell -WindowStyle Hidden -PassThru `
        -ArgumentList @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', 'Start-Sleep -Seconds 300')
    $start = $parent.StartTime.ToUniversalTime().Ticks
    if ($StaleParent) { $start++ }
    $request = @{
        parent = $parent.Id
        parent_start = $start
        stage = $stage
        executable = $executable
        old_product = $oldProduct
        new_product = $newProduct
        version = $UpgradeVersion
        msi_hash = Hash $packagePath
        old_executable_hash = $baseline['tablecraft-desktop.exe']
        old_manifest_hash = $baseline['RELEASE.json']
        executable_hash = Hash $expectedExecutable
        manifest_hash = Hash $expectedManifest
    }
    $configuration = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($request | ConvertTo-Json -Compress)))
    $source = '$request = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(''' + $configuration + ''')) | ConvertFrom-Json' + "`n" + $installSource
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
    # CI already runs elevated. Invoke the actual fixed installer helper without
    # the UAC/bootstrap/relaunch path; no desktop window is opened by this test.
    $helper = Start-Process -FilePath $powershell -WindowStyle Hidden -PassThru `
        -ArgumentList @('-NoLogo', '-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded)
    return @{ Stage = $stage; Parent = $parent; Helper = $helper; Digest = $request.msi_hash }
}

function Finish-Helper($Case) {
    Stop-Process -Id $Case.Parent.Id -ErrorAction SilentlyContinue
    if (-not $Case.Helper.WaitForExit($maximumWaitMilliseconds)) { throw 'Native installer helper timed out' }
}

try {
    $oldProduct = Product-Code $original
    $newProduct = Product-Code $upgrade
    if ($oldProduct -eq $newProduct) { throw 'Upgrade fixture must have a distinct ProductCode' }
    Run-Msi @('/i', ('"' + $original + '"'), '/qn', '/norestart',
        ('INSTALLFOLDER="' + $installedDirectory + '"'), '/l*v', ('"' + (Join-Path $evidence 'source-install.log') + '"'))
    $oldManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($oldManifest.version -ne $Version) { throw 'Source fixture manifest differs' }
    $baseline = @{}
    foreach ($name in @('tablecraft-desktop.exe', 'LICENSE', 'ThirdPartyLicenses.txt', 'RELEASE.json')) {
        $baseline[$name] = Hash (Join-Path $installedDirectory $name)
    }
    [IO.File]::WriteAllText($database, $sentinel)

    $image = Join-Path $evidence 'UpgradeImage'
    Run-Msi @('/a', ('"' + $upgrade + '"'), '/qn', '/norestart', ('TARGETDIR="' + $image + '"'))
    $manifests = @(Get-ChildItem -LiteralPath $image -Recurse -File -Filter RELEASE.json)
    $executables = @(Get-ChildItem -LiteralPath $image -Recurse -File -Filter tablecraft-desktop.exe)
    if ($manifests.Count -ne 1 -or $executables.Count -ne 1) { throw 'Fixture administrative image is ambiguous' }
    $expectedManifest = $manifests[0].FullName
    $expectedExecutable = $executables[0].FullName
    if ((Get-Content -LiteralPath $expectedManifest -Raw | ConvertFrom-Json).version -ne $UpgradeVersion) {
        throw 'Upgrade fixture manifest differs'
    }

    $case = Start-Helper 'stale-parent' $upgrade $true
    $activeParent = $case.Parent
    $activeHelper = $case.Helper
    if (-not $case.Helper.WaitForExit($maximumWaitMilliseconds) -or $case.Helper.ExitCode -eq 0 -or
        (Test-Path -LiteralPath (Join-Path $case.Stage 'READY'))) { throw 'A stale parent instance was accepted' }
    Stop-Process -Id $case.Parent.Id -ErrorAction SilentlyContinue
    Assert-Original

    $case = Start-Helper 'uncommitted' $upgrade
    $activeParent = $case.Parent
    $activeHelper = $case.Helper
    Wait-ForReady $case.Helper $case.Stage $case.Digest
    Assert-Original
    Finish-Helper $case
    if ($case.Helper.ExitCode -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $case.Stage 'FAILED.json'))) {
        throw 'A missing COMMIT was not refused with retained evidence'
    }
    Assert-Original

    # Deliberately corrupt only a disposable fixture's embedded CAB. Production
    # preparation would reject it; this directly tests native MSI rollback after
    # a storage/install failure, without adding any production fault switches.
    $corrupt = Join-Path $evidence 'corrupt-upgrade.msi'
    Copy-Item -LiteralPath $upgrade -Destination $corrupt
    $invalidCabinet = Join-Path $evidence 'invalid-cabinet.bin'
    [IO.File]::WriteAllText($invalidCabinet, 'Deliberately invalid first-party rollback fixture')
    $editable = $windowsInstaller.OpenDatabase($corrupt, 1)
    $media = $editable.OpenView('SELECT `Cabinet` FROM `Media`')
    $media.Execute() | Out-Null
    $cabinetName = ([string]$media.Fetch().StringData(1)).Substring(1)
    $media.Close() | Out-Null
    if ($cabinetName -notmatch '^[A-Za-z0-9_.-]+$') { throw 'Fixture cabinet name is unsafe' }
    $view = $editable.OpenView("UPDATE ``_Streams`` SET ``Data``=? WHERE ``Name``='$cabinetName'")
    $record = $windowsInstaller.CreateRecord(1)
    $record.SetStream(1, $invalidCabinet) | Out-Null
    $view.Execute($record) | Out-Null
    $view.Close() | Out-Null
    $editable.Commit() | Out-Null
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) | Out-Null
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($view) | Out-Null
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($media) | Out-Null
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($editable) | Out-Null
    $case = Start-Helper 'native-rollback' $corrupt
    $activeParent = $case.Parent
    $activeHelper = $case.Helper
    Wait-ForReady $case.Helper $case.Stage $case.Digest
    [IO.File]::WriteAllText((Join-Path $case.Stage 'COMMIT'), $case.Digest)
    Finish-Helper $case
    if ($case.Helper.ExitCode -eq 0) { throw 'A corrupt native cabinet was installed successfully' }
    Assert-Original
    $rollbackLog = Get-Content -LiteralPath (Join-Path $case.Stage 'install.log') -Raw
    if ($rollbackLog -notmatch 'RemoveExistingProducts' -or $rollbackLog -notmatch 'Rollback:') {
        throw 'Native rollback evidence did not capture the major-upgrade removal and rollback phases'
    }

    $case = Start-Helper 'successful-upgrade' $upgrade
    $activeParent = $case.Parent
    $activeHelper = $case.Helper
    Wait-ForReady $case.Helper $case.Stage $case.Digest
    [IO.File]::WriteAllText((Join-Path $case.Stage 'COMMIT'), $case.Digest)
    Assert-Original
    Finish-Helper $case
    $result = Get-Content -LiteralPath (Join-Path $case.Stage 'RESULT.json') -Raw | ConvertFrom-Json
    if ($case.Helper.ExitCode -ne 0 -or $result.state -ne 'installed' -or $result.product -ne $newProduct -or
        $windowsInstaller.ProductState($newProduct) -ne 5 -or $windowsInstaller.ProductState($oldProduct) -eq 5 -or
        (Hash $executable) -ne (Hash $expectedExecutable) -or (Hash $manifestPath) -ne (Hash $expectedManifest)) {
        throw 'The native helper did not install the exact requested fixture'
    }
    if ([IO.File]::ReadAllText($database) -cne $sentinel) { throw 'Upgrade removed user-owned database bytes' }
    Run-Msi @('/x', ('"' + $upgrade + '"'), '/qn', '/norestart', '/l*v', ('"' + (Join-Path $evidence 'fixture-uninstall.log') + '"'))
    if ([IO.File]::ReadAllText($database) -cne $sentinel) { throw 'Fixture uninstall removed user-owned database bytes' }
    @{
        result = 'passed'
        original_version = $Version
        fixture_version = $UpgradeVersion
        same_executable_payload = ((Hash $expectedExecutable) -eq $baseline['tablecraft-desktop.exe'])
        parent_instance_gate = $true
        commit_gate = $true
        native_rollback = $true
        native_upgrade = $true
        database_preserved = $true
        gui_opened = $false
        uac_bootstrap_qualified = $false
        relaunch_qualified = $false
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'SUMMARY.json') -Encoding UTF8
} finally {
    if ($null -ne $activeParent -and -not $activeParent.HasExited) { Stop-Process -Id $activeParent.Id -ErrorAction SilentlyContinue }
    if ($null -ne $activeHelper -and -not $activeHelper.HasExited) {
        if (-not $activeHelper.WaitForExit($maximumWaitMilliseconds)) { Stop-Process -Id $activeHelper.Id -ErrorAction SilentlyContinue }
    }
    foreach ($package in @($upgrade, $original)) {
        Run-Msi -Arguments @('/x', ('"' + $package + '"'), '/qn', '/norestart') -Allowed @(0, 1605, 1614)
    }
    Write-Output "Native MSI fixture evidence: $evidence"
}
