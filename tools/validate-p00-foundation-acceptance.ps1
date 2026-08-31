[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json,
    [string]$Python = 'python'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$validator = Join-Path $Root 'tools/validate-p00-foundation-acceptance.py'
if (-not (Test-Path $validator -PathType Leaf)) {
    throw "Missing validator: $validator"
}

$arguments = [System.Collections.Generic.List[string]]::new()
$arguments.Add($validator)
$arguments.Add('--root')
$arguments.Add($Root)
if ($Json) {
    $arguments.Add('--json')
}

& $Python @arguments
exit $LASTEXITCODE
