[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
& python tools/validate-context-materialization-plan.py
exit $LASTEXITCODE
