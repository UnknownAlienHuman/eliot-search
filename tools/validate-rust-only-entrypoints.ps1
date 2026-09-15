[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    & cargo test --locked -p xtask --test tooling_runtime_boundary --test tooling_runtime_inventory
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
