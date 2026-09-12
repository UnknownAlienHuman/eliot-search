[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Candidate,
    [string]$Bundle,
    [string]$Selection,
    [string]$OutputRoot = "artifacts/context-materialization-plans",
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Write,
    [switch]$RequireReady
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Push-Location $Root
try {
    $argsList = @(
        "run", "--locked", "--quiet", "-p", "xtask", "--",
        "build", "context-materialization-plan",
        "--root", $Root,
        "--candidate", $Candidate,
        "--output-root", $OutputRoot
    )
    if ($Bundle) { $argsList += @("--bundle", $Bundle) }
    if ($Selection) { $argsList += @("--selection", $Selection) }
    if ($Write) { $argsList += "--write" }
    if ($RequireReady) { $argsList += "--require-ready" }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
