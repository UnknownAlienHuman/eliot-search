[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }
python "$PSScriptRoot/validate-w1-milestone-packets.py" @argsList
exit $LASTEXITCODE
