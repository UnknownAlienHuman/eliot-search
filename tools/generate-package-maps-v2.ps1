[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$Json
)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $argsList = @("run", "--locked", "--quiet", "-p", "xtask", "--", "generate", "package-maps")
    if ($Check) { $argsList += "--check" }
    if ($Json) { $argsList += "--json" }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
