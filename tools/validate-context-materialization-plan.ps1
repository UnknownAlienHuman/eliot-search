[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $cargoArgs = @(
        "run", "--locked", "-p", "xtask", "--",
        "validate", "context-materialization-plan"
    )
    if ($Json) { $cargoArgs += "--json" }
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
