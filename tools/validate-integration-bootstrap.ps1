[CmdletBinding()]
param(
    [switch]$AllowMissingLock,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$args = @("$PSScriptRoot/validate-integration-bootstrap.py", "--root", $root)
if ($AllowMissingLock) { $args += "--allow-missing-lock" }
if ($Json) { $args += "--json" }
python @args
exit $LASTEXITCODE
