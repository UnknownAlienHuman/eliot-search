[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [string]$BaseCommit = '',
    [string]$Writer = '',
    [string]$Reviewer = '',
    [string[]]$AcceptedHandoff = @(),
    [string]$Output = '-',
    [switch]$RequireReady
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Push-Location $Root
try {
    $arguments = [System.Collections.Generic.List[string]]::new()
    foreach ($value in @(
        'run', '--locked', '--quiet', '-p', 'xtask', '--',
        'build', 'ticket-issuance-plan',
        '--root', $Root,
        '--package', $Package,
        '--output', $Output
    )) {
        $arguments.Add($value)
    }
    if ($BaseCommit) {
        $arguments.Add('--base-commit')
        $arguments.Add($BaseCommit)
    }
    if ($Writer) {
        $arguments.Add('--writer')
        $arguments.Add($Writer)
    }
    if ($Reviewer) {
        $arguments.Add('--reviewer')
        $arguments.Add($Reviewer)
    }
    foreach ($handoff in $AcceptedHandoff) {
        $arguments.Add('--accepted-handoff')
        $arguments.Add($handoff)
    }
    if ($RequireReady) {
        $arguments.Add('--require-ready')
    }
    & cargo @arguments
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
