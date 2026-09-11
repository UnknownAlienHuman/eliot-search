[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    & cargo test --locked -p xtask --test tooling_runtime_boundary
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
