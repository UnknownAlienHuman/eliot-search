[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [Parameter(Mandatory = $true)]
    [string]$BaseCommit,
    [string[]]$AcceptedHandoff = @(),
    [string]$OutputRoot = 'artifacts/context-artifact-candidates',
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [string]$Python = 'python',
    [switch]$PrintResult
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$argsList = @(
    (Join-Path $PSScriptRoot 'build-context-artifact-candidate.py'),
    '--root', $Root,
    '--package', $Package,
    '--base-commit', $BaseCommit,
    '--output-root', $OutputRoot
)
foreach ($handoff in $AcceptedHandoff) {
    $argsList += @('--accepted-handoff', $handoff)
}
if ($PrintResult) { $argsList += '--print-result' }
& $Python @argsList
exit $LASTEXITCODE
