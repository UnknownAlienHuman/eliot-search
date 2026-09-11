[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

& cargo run --locked --quiet -p xtask -- validate p00-ticket-drafts @argsList
exit $LASTEXITCODE
