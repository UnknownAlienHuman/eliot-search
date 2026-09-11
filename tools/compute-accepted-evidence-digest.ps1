[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Record,
    [switch]$JsonArray
)
$ErrorActionPreference = "Stop"
$args = @("run", "--locked", "--quiet", "-p", "xtask", "--", "compute", "accepted-evidence-digest", $Record)
if ($JsonArray) { $args += "--json-array" }
& cargo @args
exit $LASTEXITCODE
