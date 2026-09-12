[CmdletBinding()]
param([switch]$Json)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $argsList = @("run", "--locked", "--quiet", "-p", "xtask", "--", "validate", "package-maps")
    if ($Json) { $argsList += "--json" }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
