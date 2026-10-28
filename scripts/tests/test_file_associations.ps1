# Parse the shipped helper without evaluating it or opening any registry key.
[CmdletBinding()]
param(
    [string] $Script = (Join-Path $PSScriptRoot '../../packaging/windows/register-file-associations.ps1')
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Tokens = $null
$ParseErrors = $null
$Tree = [Management.Automation.Language.Parser]::ParseFile(
    [IO.Path]::GetFullPath($Script), [ref] $Tokens, [ref] $ParseErrors
)
if ($ParseErrors.Count -ne 0) {
    throw ($ParseErrors | Out-String)
}
$Parameters = @($Tree.ParamBlock.Parameters | ForEach-Object { $_.Name.VariablePath.UserPath })
if (($Parameters -join ',') -cne 'Action,Executable') {
    throw 'Registration helper parameter contract changed.'
}
$Forbidden = @($Tree.FindAll({
    param($Node)
    $Node -is [Management.Automation.Language.CommandAst] -and
        $Node.GetCommandName() -in @('Invoke-Expression', 'iex', 'Start-Process', 'reg.exe', 'cmd.exe')
}, $true))
if ($Forbidden.Count -ne 0) {
    throw 'Registration helper unexpectedly invokes a shell or command interpreter.'
}
Write-Output 'Registration PowerShell parser contract passed; no registration executed.'
