[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $argsList = @("run", "--locked", "-p", "xtask", "--", "validate", "w1-milestone-packets")
    if ($Json) { $argsList += "--json" }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
