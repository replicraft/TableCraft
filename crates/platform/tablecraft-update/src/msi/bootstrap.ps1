$ErrorActionPreference = 'Stop'
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
$parent = Get-Process -Id ([int]$request.parent)
$request | Add-Member -NotePropertyName parent_start -NotePropertyValue $parent.StartTime.ToUniversalTime().Ticks
$configuration = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($request | ConvertTo-Json -Compress)))
$source = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__TABLECRAFT_INSTALLER_SOURCE__'))
$source = '$request = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(''' + $configuration + ''')) | ConvertFrom-Json' + "`n" + $source
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'

try {
    # Elevate only the fixed installer helper. The desktop relaunch below stays
    # in this original unelevated process, including after a successful upgrade.
    $helper = Start-Process -FilePath $powershell -Verb RunAs -WindowStyle Hidden `
        -ArgumentList @('-NoLogo', '-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded) -PassThru
    $helper.WaitForExit()
    $resultPath = Join-Path $request.stage 'RESULT.json'
    $result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    if ($helper.ExitCode -ne 0 -or $result.state -ne 'installed' -or $result.product -ne $request.new_product) {
        throw 'Windows Installer did not confirm the requested update'
    }
    if ((Get-FileHash -LiteralPath $request.executable -Algorithm SHA256).Hash.ToLowerInvariant() -ne $request.executable_hash) {
        throw 'Updated executable identity differs'
    }
    $manifest = Join-Path (Split-Path -LiteralPath $request.executable) 'RELEASE.json'
    if ((Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash.ToLowerInvariant() -ne $request.manifest_hash) {
        throw 'Updated manifest identity differs'
    }
    Start-Process -FilePath $request.executable -WorkingDirectory (Split-Path -LiteralPath $request.executable) | Out-Null
} catch {
    # Preserve the exact private stage and native logs. No speculative file
    # replacement or forced uninstall is attempted after an installer failure.
    $failure = Join-Path $request.stage 'FAILED.json'
    if (-not (Test-Path -LiteralPath $failure)) {
        @{ state = 'handoff-failed'; phase = 'bootstrap' } | ConvertTo-Json -Compress | Set-Content -LiteralPath $failure -Encoding UTF8
    }
    exit 1
}
