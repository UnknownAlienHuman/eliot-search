[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$errors = [System.Collections.Generic.List[string]]::new()
function Fail([string]$Message) { $script:errors.Add($Message) }
function Read-File([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Leaf)) { Fail "Missing file: $Path"; return '' }
    [IO.File]::ReadAllText($full)
}
function Value([string]$Text, [string]$Key, [bool]$Required = $true) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { if ($Required) { Fail "Missing string: $Key" }; return '' }
    $m.Groups[1].Value
}
function Number([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing integer: $Key"; return [int64]0 }
    [int64]$m.Groups[1].Value
}
function Flag([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $m.Success) { Fail "Missing boolean: $Key"; return $false }
    $m.Groups[1].Value -eq 'true'
}
function Array([string]$Text, [string]$Key) {
    $m = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $m.Success) { return @() }
    @([regex]::Matches($m.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Section([string]$Text, [string]$Name) {
    $m = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $m.Success) { Fail "Missing section: $Name"; return '' }
    $m.Groups[1].Value
}
function Same([object[]]$A, [object[]]$B) {
    $x = @($A | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    $y = @($B | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    if ($x.Count -ne $y.Count) { return $false }
    for ($i = 0; $i -lt $x.Count; $i++) { if ($x[$i] -cne $y[$i]) { return $false } }
    $true
}
function Empty-ControlDir([string]$Path) {
    $full = Join-Path $Root $Path
    if (-not (Test-Path $full -PathType Container)) { Fail "Missing directory: $Path"; return }
    foreach ($file in @(Get-ChildItem $full -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) { Fail "Premature control record: $($file.FullName.Substring($Root.Length + 1))" }
    }
}

$ticketManifest = Read-File 'swarm/ticket-drafts/manifest.toml'
$contextManifest = Read-File 'swarm/context-drafts/manifest.toml'
$orchestration = Read-File 'swarm/orchestration.toml'
$launch = Read-File 'swarm/launch-state.toml'

if ((Value $ticketManifest 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (Number $ticketManifest 'draft_count') -ne 3) { Fail 'Ticket draft manifest identity/count mismatch.' }
foreach ($zero in @('issued_ticket_count','active_lease_count','submission_count','accepted_review_count','package_handoff_count','wave_receipt_count')) { if ((Number $ticketManifest $zero) -ne 0) { Fail "$zero must be zero." } }
if ((Value $contextManifest 'status') -cne 'NON_CLAIMABLE_CONTEXT_DRAFTS' -or (Number $contextManifest 'draft_count') -ne 3 -or (Number $contextManifest 'materialized_context_count') -ne 0) { Fail 'Context draft manifest identity/count mismatch.' }
if ((Number $contextManifest 'writer_visible_artifact_count_per_context') -ne 1) { Fail 'Each materialized context must be one artifact.' }

$expected = [ordered]@{
    'search-contracts' = @{ Launch='AUTHORIZED'; Precondition='CURRENTLY_PRESENT'; Scope='crates/search-contracts/**'; Soft=8000; Handoffs=0 }
    'search-domain' = @{ Launch='CONDITIONAL'; Precondition='ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope='crates/search-domain/**'; Soft=7000; Handoffs=1 }
    'search-ports' = @{ Launch='CONDITIONAL'; Precondition='ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope='crates/search-ports/**'; Soft=5500; Handoffs=1 }
}

$manifestPackages = @([regex]::Matches($ticketManifest, '(?m)^package\s*=\s*"([^"]+)"\s*$') | ForEach-Object { $_.Groups[1].Value })
$contextPackages = @([regex]::Matches($contextManifest, '(?m)^package\s*=\s*"([^"]+)"\s*$') | ForEach-Object { $_.Groups[1].Value })
if (-not (Same $manifestPackages @($expected.Keys)) -or -not (Same $contextPackages @($expected.Keys))) { Fail 'Draft package set mismatch.' }

foreach ($entry in $expected.GetEnumerator()) {
    $package = $entry.Key; $spec = $entry.Value
    $ticketPath = "swarm/ticket-drafts/p00/$package.toml"
    $contextPath = "swarm/context-drafts/p00/$package.toml"
    $ticket = Read-File $ticketPath
    $context = Read-File $contextPath

    if ((Value $ticket 'record_kind') -cne 'assignment_ticket_draft' -or (Value $ticket 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') { Fail "$package ticket is not a draft." }
    foreach ($unsafe in @('claimable','authorizes_implementation','creates_lease','may_be_writer_acknowledged')) { if (Flag $ticket $unsafe) { Fail "$package ticket enables $unsafe." } }
    if ((Value $ticket 'package') -cne $package -or (Value $ticket 'stage') -cne 'W0' -or (Value $ticket 'phase') -cne 'P00' -or (Number $ticket 'wave') -ne 0) { Fail "$package ticket stage identity mismatch." }
    if ((Value $ticket 'launch_class') -cne $spec.Launch -or (Value $ticket 'launch_precondition') -cne $spec.Precondition) { Fail "$package launch classification mismatch." }

    $identity = Section $ticket 'unresolved_identity'
    foreach ($key in @('ticket_id','lease_id','writer','reviewer')) { if ((Value $identity $key) -cne 'UNASSIGNED') { Fail "$package $key is prematurely assigned." } }
    foreach ($key in @('base_commit','branch_or_worktree')) { if ((Value $identity $key) -cne 'UNSELECTED') { Fail "$package $key is prematurely selected." } }
    if ((Value $identity 'ticket_canonical_digest') -cne 'UNAVAILABLE') { Fail "$package ticket digest is prematurely available." }

    $fence = Section $ticket 'repository_fence'
    if ((Value $fence 'repository') -cne 'UnknownAlienHuman/eliot-search' -or (Value $fence 'write_scope') -cne $spec.Scope) { Fail "$package repository fence mismatch." }
    $limits = Section $ticket 'limits'
    if ((Number $limits 'soft_src_lines') -ne $spec.Soft -or (Number $limits 'hard_total_lines') -ne 10000 -or -not (Flag $limits 'one_active_writer')) { Fail "$package limits mismatch." }

    $deps = Section $ticket 'dependencies'
    $requiredDeps = @(Array $deps 'required_handoff_packages')
    $acceptedDeps = @(Array $deps 'accepted_handoff_refs')
    if ($spec.Handoffs -eq 0) {
        if ($requiredDeps.Count -ne 0 -or $acceptedDeps.Count -ne 0 -or (Value $deps 'status') -cne 'NOT_REQUIRED') { Fail "$package must have no dependency handoff." }
    } else {
        if (-not (Same $requiredDeps @('search-contracts')) -or $acceptedDeps.Count -ne 0 -or (Value $deps 'status') -cne 'UNAVAILABLE') { Fail "$package must remain blocked on search-contracts handoff." }
    }

    if ((Value $context 'record_kind') -cne 'writer_context_draft' -or (Value $context 'status') -cne 'UNMATERIALIZED_DRAFT') { Fail "$package context is not an unmaterialized draft." }
    foreach ($unsafe in @('claimable','authorizes_implementation')) { if (Flag $context $unsafe) { Fail "$package context enables $unsafe." } }
    if ((Value $context 'package') -cne $package -or (Value $context 'base_commit') -cne 'UNSELECTED' -or (Value $context 'materialized_context_ref') -cne 'UNAVAILABLE' -or (Value $context 'materialized_context_sha256') -cne 'UNAVAILABLE') { Fail "$package context is prematurely materialized." }
    if ((Value $context 'materialization_mode') -cne 'canonical_concatenated_bundle' -or (Number $context 'writer_visible_artifact_count') -ne 1) { Fail "$package context materialization contract mismatch." }

    $sources = @(Array $context 'source_files')
    $fragments = @(Array $context 'registry_fragments')
    $handoffs = @(Array $context 'accepted_handoff_slots')
    if ((Number $context 'source_file_count') -ne $sources.Count -or (Number $context 'registry_fragment_count') -ne $fragments.Count -or (Number $context 'accepted_handoff_slot_count') -ne $handoffs.Count) { Fail "$package context counts mismatch." }
    foreach ($required in @('AGENTS.md',"crates/$package/AGENTS.md",'docs/handoff/AUTHORITY_MAP.md','swarm/ASSIGNMENT_PROTOCOL.md',"swarm/assignments/$package.md",'docs/handoff/P00_BOOTSTRAP.md','docs/contracts/p00/manifest.toml')) { if ($sources -notcontains $required) { Fail "$package context omits $required." } }
    foreach ($source in $sources) {
        if (-not (Test-Path (Join-Path $Root $source) -PathType Leaf)) { Fail "$package context source missing: $source" }
        if ($source -like 'docs/architecture/*' -or $source -match '^(crates|bins)/.+/src/') { Fail "$package context includes forbidden source: $source" }
    }
    foreach ($selector in @("swarm/crates.toml::package[name=$package]","swarm/function-packets.toml::foundation[package=$package]",'swarm/stages.toml::stage[id=W0]')) { if ($fragments -notcontains $selector) { Fail "$package context omits selector $selector." } }
    if ($spec.Handoffs -eq 0) { if ($handoffs.Count -ne 0) { Fail 'search-contracts context must have no accepted handoff slot.' } }
    else { if (-not (Same $handoffs @('search-contracts::accepted_package_and_api_handoff'))) { Fail "$package handoff slot mismatch." } }
}

if ((Number $orchestration 'schema_version') -ne 4) { Fail 'Orchestration schema_version must be 4.' }
foreach ($pair in @(
    @('ticket_draft_manifest','swarm/ticket-drafts/manifest.toml'), @('context_draft_manifest','swarm/context-drafts/manifest.toml'),
    @('control_plane_schema_registry','swarm/control-plane-schema.toml'), @('control_plane_type_registry','swarm/schemas/types-v1.toml')
)) { if ((Value $orchestration $pair[0]) -cne $pair[1]) { Fail "Orchestration path mismatch: $($pair[0])" } }
if ((Value $launch 'active_stage') -cne 'P00' -or (Number $launch 'active_wave') -ne 0) { Fail 'Launch state must remain P00/W0.' }
if (-not (Same @(Array $launch 'authorized_packages') @('search-contracts'))) { Fail 'Only search-contracts may be authorized.' }
if (-not (Same @(Array $launch 'conditional_packages') @('search-domain','search-ports'))) { Fail 'Conditional package set mismatch.' }
foreach ($dir in @('swarm/tickets','swarm/context-manifests','swarm/leases','swarm/submissions','swarm/reviews','swarm/handoffs','swarm/supersessions','swarm/wave-receipts')) { Empty-ControlDir $dir }
foreach ($file in @(Get-ChildItem (Join-Path $Root '.github/workflows') -Filter '*.yml' -File)) {
    $body = [IO.File]::ReadAllText($file.FullName)
    if ($body -match '(?m)^\s*(pull_request|push|schedule|workflow_run|repository_dispatch|workflow_call):' -or $body.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) { Fail "Workflow is not manual-only: $($file.Name)" }
}

$result = [ordered]@{ ok=($errors.Count -eq 0); ticket_drafts=3; context_drafts=3; issued_tickets=0; active_leases=0; launch='P00/W0'; errors=@($errors) }
if ($Json) { $result | ConvertTo-Json -Depth 6 } else { Write-Host 'P00 draft control validation: drafts=3 contexts=3 issued=0 leases=0 launch=P00/W0'; foreach ($e in $errors) { Write-Host "ERROR: $e" -ForegroundColor Red }; if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green } }
if (-not $result.ok) { exit 1 }
