[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
& cargo run --locked --quiet -p xtask -- validate accepted-evidence-digest
exit $LASTEXITCODE
