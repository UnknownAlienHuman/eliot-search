[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

python "$PSScriptRoot/validate-p00-ticket-drafts.py" @argsList
exit $LASTEXITCODE
