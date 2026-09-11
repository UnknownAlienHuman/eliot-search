[CmdletBinding()]
param([switch]$Json)

$ErrorActionPreference = "Stop"
$argsList = @()
if ($Json) { $argsList += "--json" }

& cargo run --locked --quiet -p xtask -- validate p00-foundation-acceptance @argsList
exit $LASTEXITCODE
