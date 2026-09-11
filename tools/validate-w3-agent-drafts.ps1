[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $argsList = @("run", "--locked", "-p", "xtask", "--", "validate", "w3-agent-drafts")
    if ($Json) { $argsList += "--json" }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
