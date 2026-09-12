[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

# The broad graph/type/schema validator remains Python-owned for the next slice.
python "$PSScriptRoot/validate-architecture-coverage.py" @argsList
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $cargoArgs = @(
        "run", "--locked", "--quiet", "-p", "xtask", "--",
        "validate", "architecture-coverage-contracts"
    )
    if ($Json) { $cargoArgs += "--json" }
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
