[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

python "$PSScriptRoot/validate-architecture-coverage.py" @argsList
exit $LASTEXITCODE
