[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()
function Add-Error([string]$Message) { $script:errors.Add($Message) }
function Add-Warning([string]$Message) { $script:warnings.Add($Message) }
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
function Same-Sequence([string[]]$Left, [string[]]$Right) {
    $a = @($Left)
    $b = @($Right)
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
function Assert-EmptyControlDirectory([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Container)) {
        Add-Error "Missing control directory: $RelativePath"
        return
    }
    $files = @(Get-ChildItem $path -Recurse -File)
    foreach ($file in $files) {
        if ($file.Name -notin @('README.md', '.gitkeep')) {
            Add-Error "Claimable control directory contains a real record before issuance: $($file.FullName.Substring($Root.Length + 1))"
        }
    }
}

$paths = [ordered]@{
    ticket_manifest = 'swarm/ticket-drafts/manifest.toml'
    context_manifest = 'swarm/context-drafts/manifest.toml'
    orchestration = 'swarm/orchestration.toml'
    launch = 'swarm/launch-state.toml'
    stages = 'swarm/stages.toml'
    functions = 'swarm/function-packets.toml'
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
        Launch = 'AUTHORIZED'
        Precondition = 'CURRENTLY_PRESENT'
        IssueStatus = 'BLOCKED_ON_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-contracts/**'
        Soft = 8000
        Context = 'swarm/context-drafts/p00/search-contracts.toml'
        Ticket = 'swarm/ticket-drafts/p00/search-contracts.toml'
        Handoffs = @()
        Sources = @(
          'AGENTS.md',
          'crates/search-contracts/AGENTS.md',
          'docs/handoff/AUTHORITY_MAP.md',
          'swarm/ASSIGNMENT_PROTOCOL.md',
          'swarm/assignments/search-contracts.md',
          'docs/handoff/P00_BOOTSTRAP.md',
          'docs/contracts/p00/README.md',
          'docs/contracts/p00/manifest.toml',
          'docs/contracts/p00/CANONICAL_TYPES.md',
          'docs/contracts/p00/TYPE_REGISTRY.md',
          'docs/contracts/p00/SUPPORT_SCHEMAS.md',
          'docs/contracts/p00/CONTRACT_CHALLENGES.md',
          'docs/contracts/p00/SOURCE_GRAPH.md',
          'docs/contracts/p00/RECIPES.md',
          'docs/contracts/p00/QUERY_AND_RESULTS.md',
          'docs/contracts/p00/RECIPE_RESULTS.md',
          'docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md',
          'docs/contracts/p00/REASON_CODES.md',
          'docs/contracts/p00/PORT_OPERATIONS.md'
        )
        Fragments = @(
          'swarm/crates.toml::package[name=search-contracts]',
          'swarm/function-packets.toml::foundation[package=search-contracts]',
          'swarm/stages.toml::stage[id=W0]',
          'swarm/launch-state.toml::authorized_packages[search-contracts]'
        )
    }
    'search-domain' = [ordered]@{
        Launch = 'CONDITIONAL'
        Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'
        IssueStatus = 'BLOCKED_ON_CONTRACTS_HANDOFF_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-domain/**'
        Soft = 7000
        Context = 'swarm/context-drafts/p00/search-domain.toml'
        Ticket = 'swarm/ticket-drafts/p00/search-domain.toml'
        Handoffs = @('search-contracts::accepted_package_and_api_handoff')
        Sources = @(
          'AGENTS.md',
          'crates/search-domain/AGENTS.md',
          'docs/handoff/AUTHORITY_MAP.md',
          'swarm/ASSIGNMENT_PROTOCOL.md',
          'swarm/assignments/search-domain.md',
          'docs/handoff/P00_BOOTSTRAP.md',
          'docs/contracts/p00/manifest.toml',
          'docs/contracts/p00/CANONICAL_TYPES.md',
          'docs/contracts/p00/SUPPORT_SCHEMAS.md',
          'docs/contracts/p00/CONTRACT_CHALLENGES.md',
          'docs/contracts/p00/SOURCE_GRAPH.md',
          'docs/contracts/p00/QUERY_AND_RESULTS.md',
          'docs/contracts/p00/RECIPE_RESULTS.md',
          'docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md',
          'docs/contracts/p00/REASON_CODES.md'
        )
        Fragments = @(
          'swarm/crates.toml::package[name=search-domain]',
          'swarm/function-packets.toml::foundation[package=search-domain]',
          'swarm/stages.toml::stage[id=W0]',
          'swarm/launch-state.toml::conditional_packages[search-domain]',
          'swarm/launch-state.toml::conditional_activation.search-domain'
        )
    }
    'search-ports' = [ordered]@{
        Launch = 'CONDITIONAL'
        Precondition = 'ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED'
        IssueStatus = 'BLOCKED_ON_CONTRACTS_HANDOFF_IDENTITY_DIGEST_AND_CONTEXT_FREEZE'
        Scope = 'crates/search-ports/**'
        Soft = 5500
        Context = 'swarm/context-drafts/p00/search-ports.toml'
        Ticket = 'swarm/ticket-drafts/p00/search-ports.toml'
        Handoffs = @('search-contracts::accepted_package_and_api_handoff')
        Sources = @(
          'AGENTS.md',
          'crates/search-ports/AGENTS.md',
          'docs/handoff/AUTHORITY_MAP.md',
          'swarm/ASSIGNMENT_PROTOCOL.md',
          'swarm/assignments/search-ports.md',
          'docs/handoff/P00_BOOTSTRAP.md',
          'docs/contracts/p00/manifest.toml',
          'docs/contracts/p00/CANONICAL_TYPES.md',
          'docs/contracts/p00/TYPE_REGISTRY.md',
          'docs/contracts/p00/SUPPORT_SCHEMAS.md',
          'docs/contracts/p00/CONTRACT_CHALLENGES.md',
          'docs/contracts/p00/PORT_OPERATIONS.md',
          'docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md',
          'docs/contracts/p00/REASON_CODES.md'
        )
        Fragments = @(
          'swarm/crates.toml::package[name=search-ports]',
          'swarm/function-packets.toml::foundation[package=search-ports]',
          'swarm/stages.toml::stage[id=W0]',
          'swarm/launch-state.toml::conditional_packages[search-ports]',
          'swarm/launch-state.toml::conditional_activation.search-ports'
        )
    }
}

# Manifest dispositions.
if ((TStr $text.ticket_manifest 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') { Add-Error 'Ticket draft manifest status is invalid.' }
if ((TInt $text.ticket_manifest 'draft_count') -ne 3) { Add-Error 'Ticket draft count must be 3.' }
foreach ($key in @('issued_ticket_count', 'active_lease_count', 'submission_count', 'accepted_review_count', 'package_handoff_count', 'wave_receipt_count')) {
    if ((TInt $text.ticket_manifest $key) -ne 0) { Add-Error "$key must remain zero." }
}
if ((TStr $text.context_manifest 'status') -cne 'NON_CLAIMABLE_CONTEXT_DRAFTS') { Add-Error 'Context draft manifest status is invalid.' }
if ((TInt $text.context_manifest 'draft_count') -ne 3 -or (TInt $text.context_manifest 'materialized_context_count') -ne 0) {
    Add-Error 'Context draft manifest must declare 3 drafts and zero materialized contexts.'
}
if ((TInt $text.context_manifest 'writer_visible_artifact_count_per_context') -ne 1) { Add-Error 'Each materialized context must remain one writer-visible artifact.' }

# Ticket manifest exact set.
$ticketBlocks = [regex]::Split($text.ticket_manifest, '(?m)^\[\[draft\]\]\s*$')
$ticketManifestPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ticketBlocks.Count; $i++) {
    $block = $ticketBlocks[$i]
    $package = TStr $block 'package'
    if ($package) { $ticketManifestPackages.Add($package) }
    if ((TStr $block 'status') -cne 'DRAFT_ONLY_NOT_ISSUED' -or (TBool $block 'claimable')) { Add-Error "Ticket manifest draft $package is claimable or has wrong status." }
}
if (-not (Same-Set @($ticketManifestPackages) @($expected.Keys))) { Add-Error 'Ticket draft manifest package set is invalid.' }

$contextBlocks = [regex]::Split($text.context_manifest, '(?m)^\[\[draft\]\]\s*$')
$contextManifestPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $contextBlocks.Count; $i++) {
    $block = $contextBlocks[$i]
    $package = TStr $block 'package'
    if ($package) { $contextManifestPackages.Add($package) }
    if ((TStr $block 'status') -cne 'UNMATERIALIZED_DRAFT' -or (TBool $block 'claimable')) { Add-Error "Context manifest draft $package is claimable or has wrong status." }
}
if (-not (Same-Set @($contextManifestPackages) @($expected.Keys))) { Add-Error 'Context draft manifest package set is invalid.' }

# Per-package drafts.
foreach ($entry in $expected.GetEnumerator()) {
    $package = $entry.Key
    $spec = $entry.Value
    $ticketText = Read-Required $spec.Ticket
    $contextText = Read-Required $spec.Context

    if ((TStr $ticketText 'record_kind') -cne 'assignment_ticket_draft') { Add-Error "$package ticket has wrong record_kind." }
    if ((TStr $ticketText 'status') -cne 'DRAFT_ONLY_NOT_ISSUED') { Add-Error "$package ticket has wrong status." }
    foreach ($flag in @('claimable', 'authorizes_implementation', 'creates_lease', 'may_be_writer_acknowledged')) {
        if (TBool $ticketText $flag) { Add-Error "$package ticket illegally enables $flag." }
    }
    if ((TStr $ticketText 'package') -cne $package -or (TStr $ticketText 'stage') -cne 'W0' -or (TStr $ticketText 'phase') -cne 'P00' -or (TInt $ticketText 'wave') -ne 0) {
        Add-Error "$package ticket stage/package identity is invalid."
    }
    if ((TStr $ticketText 'launch_class') -cne $spec.Launch -or (TStr $ticketText 'launch_precondition') -cne $spec.Precondition -or (TStr $ticketText 'issuance_status') -cne $spec.IssueStatus) {
        Add-Error "$package ticket launch/issuance classification is invalid."
    }

    $unresolved = Section $ticketText 'unresolved_identity'
    foreach ($key in @('ticket_id', 'lease_id', 'writer', 'reviewer')) {
        if ((TStr $unresolved $key) -cne 'UNASSIGNED') { Add-Error "$package ticket $key must remain UNASSIGNED." }
    }
    foreach ($key in @('base_commit', 'branch_or_worktree')) {
        if ((TStr $unresolved $key) -cne 'UNSELECTED') { Add-Error "$package ticket $key must remain UNSELECTED." }
    }
    if ((TStr $unresolved 'ticket_canonical_digest') -cne 'UNAVAILABLE') { Add-Error "$package ticket digest must remain UNAVAILABLE." }
    if ((TStr $unresolved 'issued_at') -ne '' -or (TStr $unresolved 'integration_signature_ref') -ne '') { Add-Error "$package ticket contains issuance metadata." }

    $repositoryFence = Section $ticketText 'repository_fence'
    if ((TStr $repositoryFence 'repository') -cne 'UnknownAlienHuman/eliot-search' -or (TStr $repositoryFence 'write_scope') -cne $spec.Scope) {
        Add-Error "$package repository/write-scope fence is invalid."
    }
    foreach ($registry in @('package_registry_path', 'function_registry_path', 'stage_registry_path', 'launch_state_path')) {
        $relative = TStr $repositoryFence $registry
        if (-not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "$package references missing registry $relative." }
    }
    if ((TStr $repositoryFence 'registry_digests') -cne 'UNRESOLVED_AT_ISSUANCE') { Add-Error "$package registry digests must remain unresolved." }

    $ticketContext = Section $ticketText 'context'
    if ((TStr $ticketContext 'context_draft') -cne $spec.Context) { Add-Error "$package ticket context draft path mismatch." }
    foreach ($key in @('context_manifest_ref', 'context_artifact_ref', 'context_artifact_sha256')) {
        if ((TStr $ticketContext $key) -cne 'UNAVAILABLE') { Add-Error "$package ticket $key must remain UNAVAILABLE." }
    }
    if ((TInt $ticketContext 'writer_visible_artifact_count') -ne 1 -or (TStr $ticketContext 'architecture_access') -cne 'exception-only') {
        Add-Error "$package ticket context ceiling/access is invalid."
    }

    $dependencies = Section $ticketText 'dependencies'
    $requiredPackages = @(TArray $dependencies 'required_handoff_packages')
    $acceptedRefs = @(TArray $dependencies 'accepted_handoff_refs')
    if ($package -eq 'search-contracts') {
        if ($requiredPackages.Count -ne 0 -or $acceptedRefs.Count -ne 0 -or (TStr $dependencies 'status') -cne 'NOT_REQUIRED') { Add-Error 'search-contracts draft must require no dependency handoff.' }
    } else {
        if (-not (Same-Set $requiredPackages @('search-contracts')) -or $acceptedRefs.Count -ne 0 -or (TStr $dependencies 'status') -cne 'UNAVAILABLE') { Add-Error "$package dependency handoff must remain unavailable and contracts-only." }
        if ((TStr $dependencies 'required_contract_commit') -cne 'UNSELECTED' -or (TStr $dependencies 'required_contract_api_schema_digest') -cne 'UNAVAILABLE') { Add-Error "$package contracts dependency identity is prematurely selected." }
    }

    $limits = Section $ticketText 'limits'
    if ((TInt $limits 'soft_src_lines') -ne $spec.Soft -or (TInt $limits 'split_review_total_lines') -ne 8500 -or (TInt $limits 'hard_total_lines') -ne 10000 -or -not (TBool $limits 'one_active_writer')) {
        Add-Error "$package line/lease limits are invalid."
    }
    foreach ($arrayName in @('required_outputs', 'required_evidence', 'issuance_requirements')) {
        if ((TArray $ticketText $arrayName).Count -eq 0) { Add-Error "$package ticket lacks $arrayName." }
    }

    if ((TStr $contextText 'record_kind') -cne 'writer_context_draft' -or (TStr $contextText 'status') -cne 'UNMATERIALIZED_DRAFT') { Add-Error "$package context has wrong identity/status." }
    foreach ($flag in @('claimable', 'authorizes_implementation')) { if (TBool $contextText $flag) { Add-Error "$package context illegally enables $flag." } }
    if ((TStr $contextText 'package') -cne $package -or (TStr $contextText 'stage') -cne 'W0' -or (TInt $contextText 'wave') -ne 0) { Add-Error "$package context stage/package identity is invalid." }
    if ((TStr $contextText 'base_commit') -cne 'UNSELECTED' -or (TStr $contextText 'materialized_context_ref') -cne 'UNAVAILABLE' -or (TStr $contextText 'materialized_context_sha256') -cne 'UNAVAILABLE') {
        Add-Error "$package context is prematurely materialized."
    }
    if ((TStr $contextText 'materialization_mode') -cne 'canonical_concatenated_bundle' -or (TInt $contextText 'writer_visible_artifact_count') -ne 1) {
        Add-Error "$package context materialization mode/count is invalid."
    }

    $canonicalization = Section $contextText 'canonicalization'
    if ((TStr $canonicalization 'encoding') -cne 'UTF-8' -or (TStr $canonicalization 'line_endings') -cne 'LF') { Add-Error "$package context canonicalization encoding is invalid." }
    foreach ($flag in @('preserve_declared_order', 'record_source_sha256', 'record_fragment_sha256')) { if (-not (TBool $canonicalization $flag)) { Add-Error "$package context canonicalization disables $flag." } }

    $sources = @(TArray $contextText 'source_files')
    $fragments = @(TArray $contextText 'registry_fragments')
    $handoffs = @(TArray $contextText 'accepted_handoff_slots')
    if ((TInt $contextText 'source_file_count') -ne $sources.Count -or (TInt $contextText 'registry_fragment_count') -ne $fragments.Count -or (TInt $contextText 'accepted_handoff_slot_count') -ne $handoffs.Count) {
        Add-Error "$package context declared counts do not match arrays."
    }
    if (-not (Same-Sequence $sources $spec.Sources)) { Add-Error "$package source-file context differs from the exact draft." }
    if (-not (Same-Sequence $fragments $spec.Fragments)) { Add-Error "$package registry fragments differ from the exact draft." }
    if (-not (Same-Sequence $handoffs $spec.Handoffs)) { Add-Error "$package accepted handoff slots differ from the exact draft." }

    $seenSources = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($source in $sources) {
        if (-not $seenSources.Add($source)) { Add-Error "$package context duplicates source file $source." }
        if (-not (Test-Path (Join-Path $Root $source) -PathType Leaf)) { Add-Error "$package context source file is missing: $source" }
        if ($source.StartsWith('docs/architecture/', [StringComparison]::Ordinal)) { Add-Error "$package context includes architecture master material." }
        if ($source -match '^(crates|bins)/.+/src/') { Add-Error "$package context includes implementation source: $source" }
    }
    foreach ($fragment in $fragments) {
        $registryPath = ($fragment -split '::', 2)[0]
        if (-not (Test-Path (Join-Path $Root $registryPath) -PathType Leaf)) { Add-Error "$package context registry source is missing: $registryPath" }
    }
    $forbidden = @(TArray $contextText 'forbidden_paths')
    if ($forbidden -notcontains 'docs/architecture/**') { Add-Error "$package context lacks architecture-master prohibition." }
    if ((TArray $contextText 'required_unavailable_checks').Count -eq 0) { Add-Error "$package context lacks explicit unavailable checks." }
}

# Orchestration and launch authority.
if ((TInt $text.orchestration 'schema_version') -ne 2) { Add-Error 'Orchestration schema_version must be 2.' }
foreach ($entry in [ordered]@{
    ticket_draft_manifest = $paths.ticket_manifest
    context_draft_manifest = $paths.context_manifest
    writer_lease_template = 'swarm/WRITER_LEASE_TEMPLATE.md'
    context_manifest_template = 'swarm/CONTEXT_MANIFEST_TEMPLATE.md'
    submission_template = 'swarm/SUBMISSION_TEMPLATE.md'
    review_receipt_template = 'swarm/REVIEW_RECEIPT_TEMPLATE.md'
}.GetEnumerator()) {
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

# No issued/claimable records exist yet.
foreach ($directory in @('swarm/tickets', 'swarm/context-manifests', 'swarm/leases', 'swarm/submissions', 'swarm/reviews', 'swarm/handoffs', 'swarm/wave-receipts')) {
    Assert-EmptyControlDirectory $directory
}

# Documentation and workflow policy.
Require-Tokens $paths.control_plane $text.control_plane @('three precise pre-issuance drafts', 'Drafts live under', 'status = DRAFT_ONLY_NOT_ISSUED', 'materialized contexts:   0', 'active writer leases:    0')
Require-Tokens $paths.p00 $text.p00 @('search-contracts', 'search-domain', 'search-ports')
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
    ticket_drafts = $expected.Count
    context_drafts = $expected.Count
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
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search P00 draft ticket/control validation'
    Write-Host "drafts=$($result.ticket_drafts) contexts=$($result.context_drafts) issued=$($result.issued_tickets) leases=$($result.active_leases) launch=$($result.launch_stage)/W$($result.launch_wave)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
