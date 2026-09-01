[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

python "$PSScriptRoot/validate-architecture-coverage.py" @argsList
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

python "$PSScriptRoot/validate-architecture-coverage-contracts.py" @argsList
exit $LASTEXITCODE
