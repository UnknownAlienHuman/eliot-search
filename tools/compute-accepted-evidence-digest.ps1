[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Record,
    [switch]$JsonArray
)
$ErrorActionPreference = "Stop"
$args = @("tools/compute-accepted-evidence-digest.py", $Record)
if ($JsonArray) { $args += "--json-array" }
& python @args
exit $LASTEXITCODE
