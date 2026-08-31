[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
& python tools/validate-accepted-evidence-digest.py
exit $LASTEXITCODE
