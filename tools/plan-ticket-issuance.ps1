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
    [switch]$RequireReady,
    [string]$Python = 'python'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$planner = Join-Path $Root 'tools/plan-ticket-issuance.py'
if (-not (Test-Path $planner -PathType Leaf)) {
    throw "Missing planner: $planner"
}

$arguments = [System.Collections.Generic.List[string]]::new()
$arguments.Add($planner)
$arguments.Add('--root')
$arguments.Add($Root)
$arguments.Add('--package')
$arguments.Add($Package)
$arguments.Add('--output')
$arguments.Add($Output)

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

& $Python @arguments
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
