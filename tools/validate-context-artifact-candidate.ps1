[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Push-Location $Root
try {
    $arguments = @('run', '--locked', '--quiet', '-p', 'xtask', '--', 'validate', 'context-artifact-candidate', '--root', $Root)
    if ($Json) { $arguments += '--json' }
    & cargo @arguments
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
