[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }
python "$PSScriptRoot/validate-w3-milestone-packets.py" @argsList
exit $LASTEXITCODE
