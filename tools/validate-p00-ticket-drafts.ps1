[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
function Add-Error([string]$Message) { $script:errors.Add($Message) }
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
        return ''
    }
    [IO.File]::ReadAllText($path)
}
function TStr([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ''
    }
    $match.Groups[1].Value
}
function TInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
}
function TBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}
function TArray([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Section([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $match.Success) {
        Add-Error "Missing TOML section [$Name]."
        return ''
    }
    $match.Groups[1].Value
}
function Same-Set([string[]]$Left, [string[]]$Right) {
    $a = @($Left | Sort-Object -Unique)
    $b = @($Right | Sort-Object -Unique)
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) {
        if ($a[$i] -cne $b[$i]) { return $false }
    }
    $true
}
function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$Path is missing required token: $token"
        }
    }
}
function Assert-ControlDirectoryEmpty([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Container)) {
        Add-Error "Missing control directory: $RelativePath"
        return
    }
    foreach ($file in @(Get-ChildItem $path -Recurse -File)) {
        if ($file.Name -notin @('README.md', '.gitkeep')) {
            $relative = $file.FullName.Substring($Root.Length + 1)
            Add-Error "Claimable control directory contains a record before issuance: $relative"
        }
    }
}

$paths = [ordered]@{
    ticket_manifest = 'swarm/ticket-drafts/manifest.toml'
    context_manifest = 'swarm/context-drafts/manifest.toml'
    orchestration = 'swarm/orchestration.toml'
    launch = 'swarm/launch-state.toml'
    p00_manifest = 'docs/contracts/p00/manifest.toml'
    p00 = 'docs/handoff/P00_BOOTSTRAP.md'
    control_plane = 'docs/handoff/P00_DRAFT_CONTROL_PLANE.md'
    swarm_readme = 'swarm/README.md'
    handoff_readme = 'docs/handoff/README.md'
    tools_readme = 'tools/README.md'
    workflow = '.github/workflows/p00-ticket-drafts.yml'
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Required $entry.Value }

$expected = [ordered]@{
    'search-contracts' = [ordered]@{
        Launch = 'AUTHORIZED'; Precondition = 'CURRENTLY_PRESENT'; Scope = 'crates/search-contracts/**'; Soft = 8000
        Ticket = 'swarm/ticket-drafts/p00/search-contracts.toml'; Context = 'swarm/context-drafts/p00/search-contracts.toml'; Handoffs = 0
    }
    'search-domain' = [ordered]@{
        Launch = 'CONDITIONAL'; Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope = 'crates/search-domain/**'; Soft = 7000
        Ticket = 'swarm/ticket-drafts/p00/search-domain.toml'; Context = 'swarm/context-drafts/p00/search-domain.toml'; Handoffs = 1
    }
    'search-ports' = [ordered]@{
        Launch = 'CONDITIONAL'; Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'; Scope = 'crates/search-ports/**'; Soft = 5500
        Ticket = 'swarm/ticket-drafts/p00/search-ports.toml'; Context = 'swarm/context-drafts/p00/search-ports.toml'; Handoffs = 1
    }
}

# Manifests must remain non-claimable and empty of real records.
if ((TStr $text.ticket_manifest 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (TInt $text.ticket_manifest 'draft_count') -ne 3) {
    Add-Error 'Ticket draft manifest status/count is invalid.'
}
foreach ($key in @('issued_ticket_count', 'active_lease_count', 'submission_count', 'accepted_review_count', 'package_handoff_count', 'wave_receipt_count')) {
    if ((TInt $text.ticket_manifest $key) -ne 0) { Add-Error "$key must remain zero." }
}
if ((TStr $text.context_manifest 'status') -cne 'NON_CLAIMABLE_CONTEXT_DRAFTS' -or (TInt $text.context_manifest 'draft_count') -ne 3 -or (TInt $text.context_manifest 'materialized_context_count') -ne 0) {
    Add-Error 'Context draft manifest status/count is invalid.'
}
if ((TInt $text.context_manifest 'writer_visible_artifact_count_per_context') -ne 1) {
    Add-Error 'Each materialized context must remain one writer-visible artifact.'
}

$ticketBlocks = [regex]::Split($text.ticket_manifest, '(?m)^\[\[draft\]\]\s*$')
$ticketPackages = @()
for ($i = 1; $i -lt $ticketBlocks.Count; $i++) {
    $package = TStr $ticketBlocks[$i] 'package'
    if ($package) { $ticketPackages += $package }
    if ((TStr $ticketBlocks[$i] 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (TBool $ticketBlocks[$i] 'claimable')) {
        Add-Error "Ticket manifest entry $package is claimable or has wrong status."
    }
}
if (-not (Same-Set $ticketPackages @($expected.Keys))) { Add-Error 'Ticket draft package set is invalid.' }

$contextBlocks = [regex]::Split($text.context_manifest, '(?m)^\[\[draft\]\]\s*$')
$contextPackages = @()
for ($i = 1; $i -lt $contextBlocks.Count; $i++) {
    $package = TStr $contextBlocks[$i] 'package'
    if ($package) { $contextPackages += $package }
    if ((TStr $contextBlocks[$i] 'status') -cne 'UNMATERIALIZED_DRAFT' -or (TBool $contextBlocks[$i] 'claimable')) {
        Add-Error "Context manifest entry $package is claimable or has wrong status."
    }
}
if (-not (Same-Set $contextPackages @($expected.Keys))) { Add-Error 'Context draft package set is invalid.' }

$p00Required = @(TArray $text.p00_manifest 'required_files' | ForEach-Object { "docs/contracts/p00/$_" })

foreach ($entry in $expected.GetEnumerator()) {
    $package = $entry.Key
    $spec = $entry.Value
    $ticket = Read-Required $spec.Ticket
    $context = Read-Required $spec.Context

    if ((TStr $ticket 'record_kind') -cne 'assignment_ticket_draft' -or (TStr $ticket 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') {
        Add-Error "$package ticket identity/status is invalid."
    }
    foreach ($flag in @('claimable', 'authorizes_implementation', 'creates_lease', 'may_be_writer_acknowledged')) {
        if (TBool $ticket $flag) { Add-Error "$package ticket illegally enables $flag." }
    }
    if ((TStr $ticket 'package') -cne $package -or (TStr $ticket 'stage') -cne 'W0' -or (TStr $ticket 'phase') -cne 'P00' -or (TInt $ticket 'wave') -ne 0) {
        Add-Error "$package ticket stage/package identity is invalid."
    }
    if ((TStr $ticket 'launch_class') -cne $spec.Launch -or (TStr $ticket 'launch_precondition') -cne $spec.Precondition) {
        Add-Error "$package ticket launch classification is invalid."
    }

    $unresolved = Section $ticket 'unresolved_identity'
    foreach ($key in @('ticket_id', 'lease_id', 'writer', 'reviewer')) {
        if ((TStr $unresolved $key) -cne 'UNASSIGNED') { Add-Error "$package $key must remain UNASSIGNED." }
    }
    foreach ($key in @('base_commit', 'branch_or_worktree')) {
        if ((TStr $unresolved $key) -cne 'UNSELECTED') { Add-Error "$package $key must remain UNSELECTED." }
    }
    if ((TStr $unresolved 'ticket_canonical_digest') -cne 'UNAVAILABLE' -or (TStr $unresolved 'issued_at') -ne '' -or (TStr $unresolved 'integration_signature_ref') -ne '') {
        Add-Error "$package contains premature ticket issuance metadata."
    }

    $repositoryFence = Section $ticket 'repository_fence'
    if ((TStr $repositoryFence 'repository') -cne 'UnknownAlienHuman/eliot-search' -or (TStr $repositoryFence 'write_scope') -cne $spec.Scope) {
        Add-Error "$package repository/write-scope fence is invalid."
    }
    foreach ($key in @('package_registry_path', 'function_registry_path', 'stage_registry_path', 'launch_state_path')) {
        $relative = TStr $repositoryFence $key
        if (-not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "$package references missing registry $relative." }
    }
    if ((TStr $repositoryFence 'registry_digests') -cne 'UNRESOLVED_AT_ISSUANCE') { Add-Error "$package registry digests are prematurely resolved." }

    $ticketContext = Section $ticket 'context'
    if ((TStr $ticketContext 'context_draft') -cne $spec.Context -or (TInt $ticketContext 'writer_visible_artifact_count') -ne 1 -or (TStr $ticketContext 'architecture_access') -cne 'exception-only') {
        Add-Error "$package ticket context identity/ceiling is invalid."
    }
    foreach ($key in @('context_manifest_ref', 'context_artifact_ref', 'context_artifact_sha256')) {
        if ((TStr $ticketContext $key) -cne 'UNAVAILABLE') { Add-Error "$package $key must remain UNAVAILABLE." }
    }

    $dependencies = Section $ticket 'dependencies'
    $requiredDeps = @(TArray $dependencies 'required_handoff_packages')
    $acceptedDeps = @(TArray $dependencies 'accepted_handoff_refs')
    if ($spec.Handoffs -eq 0) {
        if ($requiredDeps.Count -ne 0 -or $acceptedDeps.Count -ne 0 -or (TStr $dependencies 'status') -cne 'NOT_REQUIRED') {
            Add-Error "$package must require no dependency handoff."
        }
    } else {
        if (-not (Same-Set $requiredDeps @('search-contracts')) -or $acceptedDeps.Count -ne 0 -or (TStr $dependencies 'status') -cne 'UNAVAILABLE') {
            Add-Error "$package dependency handoff must remain contracts-only and unavailable."
        }
        if ((TStr $dependencies 'required_contract_commit') -cne 'UNSELECTED' -or (TStr $dependencies 'required_contract_api_schema_digest') -cne 'UNAVAILABLE') {
            Add-Error "$package contracts dependency identity is prematurely selected."
        }
    }

    $limits = Section $ticket 'limits'
    if ((TInt $limits 'soft_src_lines') -ne $spec.Soft -or (TInt $limits 'split_review_total_lines') -ne 8500 -or (TInt $limits 'hard_total_lines') -ne 10000 -or -not (TBool $limits 'one_active_writer')) {
        Add-Error "$package line/lease limits are invalid."
    }
    foreach ($name in @('required_outputs', 'required_evidence', 'issuance_requirements')) {
        if ((TArray $ticket $name).Count -eq 0) { Add-Error "$package ticket lacks $name." }
    }

    if ((TStr $context 'record_kind') -cne 'writer_context_draft' -or (TStr $context 'status') -cne 'UNMATERIALIZED_DRAFT') {
        Add-Error "$package context identity/status is invalid."
    }
    foreach ($flag in @('claimable', 'authorizes_implementation')) {
        if (TBool $context $flag) { Add-Error "$package context illegally enables $flag." }
    }
    if ((TStr $context 'package') -cne $package -or (TStr $context 'base_commit') -cne 'UNSELECTED' -or (TStr $context 'materialized_context_ref') -cne 'UNAVAILABLE' -or (TStr $context 'materialized_context_sha256') -cne 'UNAVAILABLE') {
        Add-Error "$package context is prematurely materialized or misbound."
    }
    if ((TStr $context 'materialization_mode') -cne 'canonical_concatenated_bundle' -or (TInt $context 'writer_visible_artifact_count') -ne 1) {
        Add-Error "$package context materialization mode/count is invalid."
    }

    $canonical = Section $context 'canonicalization'
    if ((TStr $canonical 'encoding') -cne 'UTF-8' -or (TStr $canonical 'line_endings') -cne 'LF') { Add-Error "$package context encoding is invalid." }
    foreach ($flag in @('preserve_declared_order', 'record_source_sha256', 'record_fragment_sha256')) {
        if (-not (TBool $canonical $flag)) { Add-Error "$package context disables $flag." }
    }

    $sources = @(TArray $context 'source_files')
    $fragments = @(TArray $context 'registry_fragments')
    $handoffs = @(TArray $context 'accepted_handoff_slots')
    if ((TInt $context 'source_file_count') -ne $sources.Count -or (TInt $context 'registry_fragment_count') -ne $fragments.Count -or (TInt $context 'accepted_handoff_slot_count') -ne $handoffs.Count) {
        Add-Error "$package context declared counts differ from arrays."
    }
    if ($sources.Count -gt 20 -or $fragments.Count -gt 6 -or $handoffs.Count -ne $spec.Handoffs) {
        Add-Error "$package context exceeds a declared static/dynamic ceiling."
    }

    $requiredSources = @('AGENTS.md', "crates/$package/AGENTS.md", 'docs/handoff/AUTHORITY_MAP.md', 'swarm/ASSIGNMENT_PROTOCOL.md', "swarm/assignments/$package.md", 'docs/handoff/P00_BOOTSTRAP.md', 'docs/contracts/p00/manifest.toml')
    foreach ($required in $requiredSources) {
        if ($sources -notcontains $required) { Add-Error "$package context omits required source $required." }
    }
    if ($package -eq 'search-contracts') {
        foreach ($required in $p00Required) {
            if ($sources -notcontains $required) { Add-Error "search-contracts context omits P00 pack file $required." }
        }
    }

    $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($source in $sources) {
        if (-not $seen.Add($source)) { Add-Error "$package context duplicates $source." }
        if (-not (Test-Path (Join-Path $Root $source) -PathType Leaf)) { Add-Error "$package context source is missing: $source" }
        if ($source.StartsWith('docs/architecture/', [StringComparison]::Ordinal)) { Add-Error "$package context includes architecture master material." }
        if ($source -match '^(crates|bins)/.+/src/') { Add-Error "$package context includes implementation source: $source" }
    }
    foreach ($fragment in $fragments) {
        $parts = $fragment -split '::', 2
        if ($parts.Count -ne 2 -or -not (Test-Path (Join-Path $Root $parts[0]) -PathType Leaf)) {
            Add-Error "$package context has invalid registry fragment $fragment."
        }
    }
    if ($fragments -notcontains "swarm/crates.toml::package[name=$package]" -or $fragments -notcontains "swarm/function-packets.toml::foundation[package=$package]" -or $fragments -notcontains 'swarm/stages.toml::stage[id=W0]') {
        Add-Error "$package context lacks exact package/function/stage registry selectors."
    }
    if ($package -eq 'search-contracts') {
        if ($fragments -notcontains 'swarm/launch-state.toml::authorized_packages[search-contracts]' -or $handoffs.Count -ne 0) { Add-Error 'search-contracts context launch/handoff selectors are invalid.' }
    } else {
        if ($fragments -notcontains "swarm/launch-state.toml::conditional_packages[$package]" -or $fragments -notcontains "swarm/launch-state.toml::conditional_activation.$package" -or -not (Same-Set $handoffs @('search-contracts::accepted_package_and_api_handoff'))) {
            Add-Error "$package context conditional selectors/handoff slot are invalid."
        }
    }
    if ((TArray $context 'forbidden_paths') -notcontains 'docs/architecture/**' -or (TArray $context 'required_unavailable_checks').Count -eq 0) {
        Add-Error "$package context lacks forbidden-path or unavailable-check declarations."
    }
}

# Orchestration and launch authority.
if ((TInt $text.orchestration 'schema_version') -ne 2) { Add-Error 'Orchestration schema_version must be 2.' }
$requiredPaths = [ordered]@{
    ticket_draft_manifest = $paths.ticket_manifest
    context_draft_manifest = $paths.context_manifest
    writer_lease_template = 'swarm/WRITER_LEASE_TEMPLATE.md'
    context_manifest_template = 'swarm/CONTEXT_MANIFEST_TEMPLATE.md'
    submission_template = 'swarm/SUBMISSION_TEMPLATE.md'
    review_receipt_template = 'swarm/REVIEW_RECEIPT_TEMPLATE.md'
}
foreach ($entry in $requiredPaths.GetEnumerator()) {
    if ((TStr $text.orchestration $entry.Key) -cne $entry.Value) { Add-Error "Orchestration $($entry.Key) path mismatch." }
    if (-not (Test-Path (Join-Path $Root $entry.Value) -PathType Leaf)) { Add-Error "Orchestration references missing file $($entry.Value)." }
}
$states = @(TArray $text.orchestration 'states')
foreach ($draftState in @('DRAFT_ONLY_NOT_ISSUED', 'UNMATERIALIZED_DRAFT')) {
    if ($states -contains $draftState) { Add-Error "Draft state $draftState illegally appears in orchestration states." }
}
$drafts = Section $text.orchestration 'drafts'
foreach ($flag in @('drafts_are_orchestration_states', 'draft_presence_authorizes_work', 'draft_presence_creates_lease', 'draft_may_be_writer_acknowledged', 'draft_may_be_copied_verbatim_to_issued_ticket', 'architecture_master_in_ordinary_context', 'dependency_implementation_source_in_context')) {
    if (TBool $drafts $flag) { Add-Error "Unsafe orchestration draft flag enabled: $flag" }
}
foreach ($flag in @('issued_ticket_requires_new_immutable_record', 'issued_ticket_requires_exact_base_commit', 'issued_ticket_requires_materialized_context_manifest', 'issued_ticket_requires_writer_and_independent_reviewer', 'issued_ticket_requires_instruction_and_registry_digests', 'conditional_ticket_requires_accepted_dependency_handoffs', 'materialized_context_is_append_only')) {
    if (-not (TBool $drafts $flag)) { Add-Error "Required orchestration draft flag disabled: $flag" }
}
if ((TInt $drafts 'materialized_context_artifact_count') -ne 1) { Add-Error 'Materialized context artifact count must be one.' }

if ((TStr $text.launch 'active_stage') -cne 'P00' -or (TInt $text.launch 'active_wave') -ne 0) { Add-Error 'Launch authority must remain P00/W0.' }
if (-not (Same-Set @(TArray $text.launch 'authorized_packages') @('search-contracts'))) { Add-Error 'Only search-contracts may remain authorized.' }
if (-not (Same-Set @(TArray $text.launch 'conditional_packages') @('search-domain', 'search-ports'))) { Add-Error 'Conditional P00 package set is invalid.' }

foreach ($directory in @('swarm/tickets', 'swarm/context-manifests', 'swarm/leases', 'swarm/submissions', 'swarm/reviews', 'swarm/handoffs', 'swarm/wave-receipts')) {
    Assert-ControlDirectoryEmpty $directory
}

Require-Tokens $paths.control_plane $text.control_plane @('three precise pre-issuance drafts', 'DRAFT_ONLY_NOT_ISSUED', 'materialized contexts:   0', 'active writer leases:    0')
Require-Tokens $paths.p00 $text.p00 @('non-claimable', 'materialized contexts: 0', 'issued tickets:        0')
Require-Tokens $paths.swarm_readme $text.swarm_readme @('ticket-drafts', 'context-drafts', 'writer lease')
Require-Tokens $paths.handoff_readme $text.handoff_readme @('P00_DRAFT_CONTROL_PLANE.md')
Require-Tokens $paths.tools_readme $text.tools_readme @('validate-p00-ticket-drafts.ps1')

$workflowFiles = @(Get-ChildItem (Join-Path $Root '.github/workflows') -Filter '*.yml' -File)
foreach ($file in $workflowFiles) {
    $workflowText = [IO.File]::ReadAllText($file.FullName)
    if ($workflowText -match '(?m)^\s*(pull_request|push|schedule|workflow_run|repository_dispatch|workflow_call):') { Add-Error "Automatic workflow trigger found in $($file.Name)." }
    if ($workflowText.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) { Add-Error "Workflow $($file.Name) lacks workflow_dispatch." }
}
Require-Tokens $paths.workflow $text.workflow @('contents: read', 'persist-credentials: false', 'validate-p00-ticket-drafts.ps1')

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    ticket_drafts = 3
    context_drafts = 3
    materialized_contexts = 0
    issued_tickets = 0
    active_leases = 0
    submissions = 0
    accepted_reviews = 0
    package_handoffs = 0
    workflows = $workflowFiles.Count
    launch_stage = TStr $text.launch 'active_stage'
    launch_wave = TInt $text.launch 'active_wave'
    authorized = @(TArray $text.launch 'authorized_packages')
    conditional = @(TArray $text.launch 'conditional_packages')
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search P00 draft ticket/control validation'
    Write-Host "drafts=$($result.ticket_drafts) contexts=$($result.context_drafts) issued=$($result.issued_tickets) leases=$($result.active_leases) launch=$($result.launch_stage)/W$($result.launch_wave)"
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
