[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Candidate,
    [string]$Bundle,
    [string]$Selection,
    [string]$OutputRoot = "artifacts/context-materialization-plans",
    [switch]$Write,
    [switch]$RequireReady
)
$ErrorActionPreference = "Stop"
$args = @("tools/plan-context-materialization.py", "--candidate", $Candidate, "--output-root", $OutputRoot)
if ($Bundle) { $args += @("--bundle", $Bundle) }
if ($Selection) { $args += @("--selection", $Selection) }
if ($Write) { $args += "--write" }
if ($RequireReady) { $args += "--require-ready" }
& python @args
exit $LASTEXITCODE
