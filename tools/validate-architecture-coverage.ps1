[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    foreach ($target in @("architecture-coverage", "architecture-coverage-contracts")) {
        $cargoArgs = @(
            "run", "--locked", "--quiet", "-p", "xtask", "--",
            "validate", $target
        )
        if ($Json) { $cargoArgs += "--json" }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
}
finally {
    Pop-Location
}
exit 0
