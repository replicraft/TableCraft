$ErrorActionPreference = 'Stop'
$parentProcess = [int]$env:TABLECRAFT_UPDATE_PARENT
$deadline = (Get-Date).AddMinutes(5)

while (Get-Process -Id $parentProcess -ErrorAction SilentlyContinue) {
    if ((Get-Date) -ge $deadline) {
        exit 1
    }
    Start-Sleep -Seconds 1
}

$backup = $env:TABLECRAFT_UPDATE_BACKUP
$ready = Join-Path -Path $backup -ChildPath 'READY'
if (-not (Test-Path -LiteralPath $ready -PathType Leaf)) {
    exit 1
}

try {
    $application = Start-Process -FilePath $env:TABLECRAFT_UPDATE_LAUNCH `
        -WorkingDirectory $env:TABLECRAFT_UPDATE_WORKING_DIRECTORY -PassThru
    Start-Sleep -Seconds 1
    if ($application.HasExited) {
        exit 1
    }
} catch {
    exit 1
}

# The exact private backup is passed by the parent, never read from release data.
Remove-Item -LiteralPath $backup -Recurse -Force
