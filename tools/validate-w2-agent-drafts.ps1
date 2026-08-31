[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }
python "$PSScriptRoot/validate-w2-agent-drafts.py" @argsList
exit $LASTEXITCODE
