[CmdletBinding()]
param(
    [switch]$AllowMissingLock,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $cargoArgs = @('run')
    if (-not $AllowMissingLock) { $cargoArgs += '--locked' }
    $cargoArgs += @('-p', 'xtask', '--', 'validate', 'integration-bootstrap', '--root', $root)
    if ($AllowMissingLock) { $cargoArgs += '--allow-missing-lock' }
    if ($Json) { $cargoArgs += '--json' }
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
