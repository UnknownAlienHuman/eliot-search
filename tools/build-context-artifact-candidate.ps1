[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [Parameter(Mandatory = $true)]
    [string]$BaseCommit,
    [string[]]$AcceptedHandoff = @(),
    [string]$OutputRoot = 'artifacts/context-artifact-candidates',
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$PrintResult
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Push-Location $Root
try {
    $argsList = @(
        'run', '--locked', '--quiet', '-p', 'xtask', '--',
        'build', 'context-artifact-candidate',
        '--root', $Root,
        '--package', $Package,
        '--base-commit', $BaseCommit,
        '--output-root', $OutputRoot
    )
    foreach ($handoff in $AcceptedHandoff) {
        $argsList += @('--accepted-handoff', $handoff)
    }
    if ($PrintResult) { $argsList += '--print-result' }
    & cargo @argsList
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
