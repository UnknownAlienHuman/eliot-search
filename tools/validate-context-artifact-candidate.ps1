[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json,
    [string]$Python = 'python'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$argsList = @((Join-Path $PSScriptRoot 'validate-context-artifact-candidate.py'), '--root', $Root)
if ($Json) { $argsList += '--json' }
& $Python @argsList
exit $LASTEXITCODE
